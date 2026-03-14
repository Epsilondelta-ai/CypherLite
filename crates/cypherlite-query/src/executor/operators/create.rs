// CreateOp: node and edge creation via storage engine

use crate::executor::eval::eval;
use crate::executor::{ExecutionError, Params, Record, ScalarFnLookup, TriggerLookup, Value};
use crate::parser::ast::*;
use cypherlite_core::{LabelRegistry, PropertyValue};
use cypherlite_storage::StorageEngine;

/// System property names that are automatically managed.
pub const SYSTEM_PROP_CREATED_AT: &str = "_created_at";
/// System property name for last-updated timestamp.
pub const SYSTEM_PROP_UPDATED_AT: &str = "_updated_at";
/// Temporal edge property: validity start timestamp.
pub const TEMPORAL_PROP_VALID_FROM: &str = "_valid_from";
/// Temporal edge property: validity end timestamp.
pub const TEMPORAL_PROP_VALID_TO: &str = "_valid_to";

/// Check if a property name is a system-managed (read-only) property.
/// Note: _valid_from and _valid_to are temporal but user-settable, so they are NOT system properties.
pub fn is_system_property(name: &str) -> bool {
    name == SYSTEM_PROP_CREATED_AT || name == SYSTEM_PROP_UPDATED_AT
}

/// Check if a property name is a temporal edge property (user-settable).
pub fn is_temporal_edge_property(name: &str) -> bool {
    name == TEMPORAL_PROP_VALID_FROM || name == TEMPORAL_PROP_VALID_TO
}

/// Get the current query timestamp from params.
fn get_query_timestamp(params: &Params) -> i64 {
    match params.get("__query_start_ms__") {
        Some(Value::Int64(ms)) => *ms,
        _ => std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0),
    }
}

/// Inject _created_at and _updated_at into property list.
fn inject_create_timestamps(
    properties: &mut Vec<(u32, PropertyValue)>,
    engine: &mut StorageEngine,
    params: &Params,
) {
    let now = get_query_timestamp(params);
    let created_key = engine.get_or_create_prop_key(SYSTEM_PROP_CREATED_AT);
    let updated_key = engine.get_or_create_prop_key(SYSTEM_PROP_UPDATED_AT);
    properties.push((created_key, PropertyValue::DateTime(now)));
    properties.push((updated_key, PropertyValue::DateTime(now)));
}

/// Inject _valid_from on edge creation if not already provided by user.
fn inject_edge_valid_from(
    properties: &mut Vec<(u32, PropertyValue)>,
    engine: &mut StorageEngine,
    params: &Params,
) {
    let valid_from_key = engine.get_or_create_prop_key(TEMPORAL_PROP_VALID_FROM);
    // Only inject if user didn't already set _valid_from
    let already_set = properties.iter().any(|(k, _)| *k == valid_from_key);
    if !already_set {
        let now = get_query_timestamp(params);
        properties.push((valid_from_key, PropertyValue::DateTime(now)));
    }
}

/// Validate that no system properties are being set by the user in a map literal.
pub fn validate_no_system_properties(props: &Option<MapLiteral>) -> Result<(), ExecutionError> {
    if let Some(map) = props {
        for (key, _) in map {
            if is_system_property(key) {
                return Err(ExecutionError {
                    message: format!("System property is read-only: {}", key),
                });
            }
        }
    }
    Ok(())
}

/// Create nodes and edges from a pattern.
/// Walks each pattern chain, creating nodes and edges as specified.
pub fn execute_create(
    source_records: Vec<Record>,
    pattern: &Pattern,
    engine: &mut StorageEngine,
    params: &Params,
    scalar_fns: &dyn ScalarFnLookup,
    trigger_fns: &dyn TriggerLookup,
) -> Result<Vec<Record>, ExecutionError> {
    let mut results = Vec::new();

    for record in source_records {
        let mut new_record = record.clone();

        for chain in &pattern.chains {
            create_chain(chain, &mut new_record, engine, params, scalar_fns, trigger_fns)?;
        }

        results.push(new_record);
    }

    Ok(results)
}

/// Build a TriggerContext for a node.
fn build_node_trigger_context(
    entity_id: u64,
    label: Option<&str>,
    properties: &[(u32, PropertyValue)],
    engine: &StorageEngine,
    operation: cypherlite_core::TriggerOperation,
) -> cypherlite_core::TriggerContext {
    let props_map = properties
        .iter()
        .map(|(k, v)| {
            let name = engine
                .catalog()
                .prop_key_name(*k)
                .unwrap_or("?")
                .to_string();
            (name, v.clone())
        })
        .collect();
    cypherlite_core::TriggerContext {
        entity_type: cypherlite_core::EntityType::Node,
        entity_id,
        label_or_type: label.map(|s| s.to_string()),
        properties: props_map,
        operation,
    }
}

/// Build a TriggerContext for an edge.
fn build_edge_trigger_context(
    entity_id: u64,
    rel_type: Option<&str>,
    properties: &[(u32, PropertyValue)],
    engine: &StorageEngine,
    operation: cypherlite_core::TriggerOperation,
) -> cypherlite_core::TriggerContext {
    let props_map = properties
        .iter()
        .map(|(k, v)| {
            let name = engine
                .catalog()
                .prop_key_name(*k)
                .unwrap_or("?")
                .to_string();
            (name, v.clone())
        })
        .collect();
    cypherlite_core::TriggerContext {
        entity_type: cypherlite_core::EntityType::Edge,
        entity_id,
        label_or_type: rel_type.map(|s| s.to_string()),
        properties: props_map,
        operation,
    }
}

/// Create nodes and edges from a single pattern chain.
fn create_chain(
    chain: &PatternChain,
    record: &mut Record,
    engine: &mut StorageEngine,
    params: &Params,
    scalar_fns: &dyn ScalarFnLookup,
    trigger_fns: &dyn TriggerLookup,
) -> Result<(), ExecutionError> {
    let mut elements = chain.elements.iter();
    let mut prev_var: Option<String> = None;
    let temporal_enabled = engine.config().temporal_tracking_enabled;

    while let Some(element) = elements.next() {
        match element {
            PatternElement::Node(np) => {
                let var_name = np.variable.as_deref().unwrap_or("");

                // If variable already bound in record, skip node creation
                if !var_name.is_empty() && record.contains_key(var_name) {
                    prev_var = Some(var_name.to_string());
                    continue;
                }

                // Validate no system properties in user-specified properties
                validate_no_system_properties(&np.properties)?;

                // Resolve labels
                let labels: Vec<u32> = np
                    .labels
                    .iter()
                    .map(|l| engine.get_or_create_label(l))
                    .collect();

                // Resolve properties
                let mut properties = resolve_properties(&np.properties, record, engine, params, scalar_fns)?;

                // Inject timestamps if temporal tracking is enabled
                if temporal_enabled {
                    inject_create_timestamps(&mut properties, engine, params);
                }

                // Fire before_create trigger
                let first_label = np.labels.first().map(|s| s.as_str());
                let before_ctx = build_node_trigger_context(
                    0,
                    first_label,
                    &properties,
                    engine,
                    cypherlite_core::TriggerOperation::Create,
                );
                trigger_fns.fire_before_create(&before_ctx)?;

                let node_id = engine.create_node(labels, properties.clone());

                // Fire after_create trigger with actual node_id
                let after_ctx = build_node_trigger_context(
                    node_id.0,
                    first_label,
                    &properties,
                    engine,
                    cypherlite_core::TriggerOperation::Create,
                );
                trigger_fns.fire_after_create(&after_ctx)?;

                if !var_name.is_empty() {
                    record.insert(var_name.to_string(), Value::Node(node_id));
                }
                prev_var = if var_name.is_empty() {
                    None
                } else {
                    Some(var_name.to_string())
                };
            }
            PatternElement::Relationship(rp) => {
                // Next element should be a node
                let next_node = elements.next().ok_or_else(|| ExecutionError {
                    message: "relationship must be followed by a node in CREATE pattern"
                        .to_string(),
                })?;

                let target_np = match next_node {
                    PatternElement::Node(np) => np,
                    _ => {
                        return Err(ExecutionError {
                            message: "expected node after relationship in CREATE".to_string(),
                        })
                    }
                };

                // Create or resolve target node
                let target_var_name = target_np.variable.as_deref().unwrap_or("");
                let target_node_id = if !target_var_name.is_empty()
                    && record.contains_key(target_var_name)
                {
                    match record.get(target_var_name) {
                        Some(Value::Node(nid)) => *nid,
                        _ => {
                            return Err(ExecutionError {
                                message: format!("variable '{}' is not a node", target_var_name),
                            })
                        }
                    }
                } else {
                    // Validate no system properties in target node
                    validate_no_system_properties(&target_np.properties)?;

                    let labels: Vec<u32> = target_np
                        .labels
                        .iter()
                        .map(|l| engine.get_or_create_label(l))
                        .collect();
                    let mut properties =
                        resolve_properties(&target_np.properties, record, engine, params, scalar_fns)?;

                    if temporal_enabled {
                        inject_create_timestamps(&mut properties, engine, params);
                    }

                    // Fire before_create trigger for target node
                    let first_label = target_np.labels.first().map(|s| s.as_str());
                    let before_ctx = build_node_trigger_context(
                        0,
                        first_label,
                        &properties,
                        engine,
                        cypherlite_core::TriggerOperation::Create,
                    );
                    trigger_fns.fire_before_create(&before_ctx)?;

                    let nid = engine.create_node(labels, properties.clone());

                    // Fire after_create trigger for target node
                    let after_ctx = build_node_trigger_context(
                        nid.0,
                        first_label,
                        &properties,
                        engine,
                        cypherlite_core::TriggerOperation::Create,
                    );
                    trigger_fns.fire_after_create(&after_ctx)?;

                    if !target_var_name.is_empty() {
                        record.insert(target_var_name.to_string(), Value::Node(nid));
                    }
                    nid
                };

                // Resolve source node
                let src_node_id = match &prev_var {
                    Some(pv) => match record.get(pv) {
                        Some(Value::Node(nid)) => *nid,
                        _ => {
                            return Err(ExecutionError {
                                message: format!("variable '{}' is not a node", pv),
                            })
                        }
                    },
                    None => {
                        return Err(ExecutionError {
                            message: "no source node for relationship in CREATE".to_string(),
                        })
                    }
                };

                // Resolve relationship type
                let rel_type_name = rp
                    .rel_types
                    .first()
                    .ok_or_else(|| ExecutionError {
                        message: "CREATE relationship requires a type".to_string(),
                    })?;
                let rel_type_id = engine.get_or_create_rel_type(rel_type_name);

                // Validate no system properties in relationship
                validate_no_system_properties(&rp.properties)?;

                // Resolve relationship properties
                let mut rel_props = resolve_properties(&rp.properties, record, engine, params, scalar_fns)?;

                if temporal_enabled {
                    inject_create_timestamps(&mut rel_props, engine, params);
                    // BB-T3: Auto-inject _valid_from on edge CREATE
                    inject_edge_valid_from(&mut rel_props, engine, params);
                }

                // Create edge based on direction
                let (start, end) = match rp.direction {
                    RelDirection::Outgoing | RelDirection::Undirected => {
                        (src_node_id, target_node_id)
                    }
                    RelDirection::Incoming => (target_node_id, src_node_id),
                };

                // Fire before_create trigger for edge
                let before_edge_ctx = build_edge_trigger_context(
                    0,
                    Some(rel_type_name),
                    &rel_props,
                    engine,
                    cypherlite_core::TriggerOperation::Create,
                );
                trigger_fns.fire_before_create(&before_edge_ctx)?;

                let edge_id = engine
                    .create_edge(start, end, rel_type_id, rel_props.clone())
                    .map_err(|e| ExecutionError {
                        message: format!("failed to create edge: {}", e),
                    })?;

                // Fire after_create trigger for edge
                let after_edge_ctx = build_edge_trigger_context(
                    edge_id.0,
                    Some(rel_type_name),
                    &rel_props,
                    engine,
                    cypherlite_core::TriggerOperation::Create,
                );
                trigger_fns.fire_after_create(&after_edge_ctx)?;

                if let Some(rv) = &rp.variable {
                    record.insert(rv.clone(), Value::Edge(edge_id));
                }

                prev_var = if target_var_name.is_empty() {
                    None
                } else {
                    Some(target_var_name.to_string())
                };
            }
        }
    }

    Ok(())
}

/// Resolve properties from a MapLiteral, evaluating expressions.
fn resolve_properties(
    props: &Option<MapLiteral>,
    record: &Record,
    engine: &StorageEngine,
    params: &Params,
    scalar_fns: &dyn ScalarFnLookup,
) -> Result<Vec<(u32, PropertyValue)>, ExecutionError> {
    match props {
        None => Ok(vec![]),
        Some(map) => {
            let mut result = Vec::new();
            for (key, expr) in map {
                let value = eval(expr, record, engine, params, scalar_fns)?;
                let pv = PropertyValue::try_from(value).map_err(|e| ExecutionError {
                    message: format!("invalid property value for '{}': {}", key, e),
                })?;
                let key_id = engine.catalog().prop_key_id(key).unwrap_or(0);
                result.push((key_id, pv));
            }
            Ok(result)
        }
    }
}

/// Resolve properties from a MapLiteral using mutable engine access.
/// This is the preferred version that can register new property keys.
pub fn resolve_properties_mut(
    props: &Option<MapLiteral>,
    record: &Record,
    engine: &mut StorageEngine,
    params: &Params,
    scalar_fns: &dyn ScalarFnLookup,
) -> Result<Vec<(u32, PropertyValue)>, ExecutionError> {
    match props {
        None => Ok(vec![]),
        Some(map) => {
            let mut result = Vec::new();
            for (key, expr) in map {
                let value = eval(expr, record, &*engine, params, scalar_fns)?;
                let pv = PropertyValue::try_from(value).map_err(|e| ExecutionError {
                    message: format!("invalid property value for '{}': {}", key, e),
                })?;
                let key_id = engine.get_or_create_prop_key(key);
                result.push((key_id, pv));
            }
            Ok(result)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cypherlite_core::{DatabaseConfig, SyncMode};
    use tempfile::tempdir;

    fn test_engine(dir: &std::path::Path) -> StorageEngine {
        let config = DatabaseConfig {
            path: dir.join("test.cyl"),
            wal_sync_mode: SyncMode::Normal,
            ..Default::default()
        };
        StorageEngine::open(config).expect("open")
    }

    // EXEC-T005: CreateOp node creation
    #[test]
    fn test_create_single_node() {
        let dir = tempdir().expect("tempdir");
        let mut engine = test_engine(dir.path());

        let pattern = Pattern {
            chains: vec![PatternChain {
                elements: vec![PatternElement::Node(NodePattern {
                    variable: Some("n".to_string()),
                    labels: vec!["Person".to_string()],
                    properties: None,
                })],
            }],
        };

        let params = Params::new();
        let result = execute_create(vec![Record::new()], &pattern, &mut engine, &params, &(), &());
        let records = result.expect("should succeed");
        assert_eq!(records.len(), 1);
        assert!(records[0].contains_key("n"));
        assert!(matches!(records[0].get("n"), Some(Value::Node(_))));

        // Verify node was created in engine
        assert_eq!(engine.node_count(), 1);
    }

    #[test]
    fn test_create_node_with_properties() {
        let dir = tempdir().expect("tempdir");
        let mut engine = test_engine(dir.path());

        // Pre-register property key
        let _name_key = engine.get_or_create_prop_key("name");

        let pattern = Pattern {
            chains: vec![PatternChain {
                elements: vec![PatternElement::Node(NodePattern {
                    variable: Some("n".to_string()),
                    labels: vec!["Person".to_string()],
                    properties: Some(vec![(
                        "name".to_string(),
                        Expression::Literal(Literal::String("Alice".into())),
                    )]),
                })],
            }],
        };

        let params = Params::new();
        let result = execute_create(vec![Record::new()], &pattern, &mut engine, &params, &(), &());
        let records = result.expect("should succeed");
        assert_eq!(records.len(), 1);

        // Verify node properties
        if let Some(Value::Node(nid)) = records[0].get("n") {
            let node = engine.get_node(*nid).expect("node exists");
            assert!(!node.properties.is_empty());
        } else {
            panic!("expected node value");
        }
    }

    #[test]
    fn test_create_node_and_relationship() {
        let dir = tempdir().expect("tempdir");
        let mut engine = test_engine(dir.path());

        let pattern = Pattern {
            chains: vec![PatternChain {
                elements: vec![
                    PatternElement::Node(NodePattern {
                        variable: Some("a".to_string()),
                        labels: vec!["Person".to_string()],
                        properties: None,
                    }),
                    PatternElement::Relationship(RelationshipPattern {
                        variable: Some("r".to_string()),
                        rel_types: vec!["KNOWS".to_string()],
                        direction: RelDirection::Outgoing,
                        properties: None,
                        min_hops: None,
                        max_hops: None,
                    }),
                    PatternElement::Node(NodePattern {
                        variable: Some("b".to_string()),
                        labels: vec!["Person".to_string()],
                        properties: None,
                    }),
                ],
            }],
        };

        let params = Params::new();
        let result = execute_create(vec![Record::new()], &pattern, &mut engine, &params, &(), &());
        let records = result.expect("should succeed");
        assert_eq!(records.len(), 1);
        assert!(records[0].contains_key("a"));
        assert!(records[0].contains_key("r"));
        assert!(records[0].contains_key("b"));
        assert_eq!(engine.node_count(), 2);
        assert_eq!(engine.edge_count(), 1);
    }

    // BB-T1: is_system_property does NOT include _valid_from/_valid_to
    #[test]
    fn test_valid_from_is_not_system_property() {
        assert!(!is_system_property("_valid_from"));
        assert!(!is_system_property("_valid_to"));
    }

    // BB-T1: is_temporal_edge_property recognizes _valid_from/_valid_to
    #[test]
    fn test_temporal_edge_property_detection() {
        assert!(is_temporal_edge_property("_valid_from"));
        assert!(is_temporal_edge_property("_valid_to"));
        assert!(!is_temporal_edge_property("_created_at"));
        assert!(!is_temporal_edge_property("name"));
    }

    // BB-T3: Edge CREATE injects _valid_from
    #[test]
    fn test_create_edge_injects_valid_from() {
        let dir = tempdir().expect("tempdir");
        let mut engine = test_engine(dir.path());

        let pattern = Pattern {
            chains: vec![PatternChain {
                elements: vec![
                    PatternElement::Node(NodePattern {
                        variable: Some("a".to_string()),
                        labels: vec!["Person".to_string()],
                        properties: None,
                    }),
                    PatternElement::Relationship(RelationshipPattern {
                        variable: Some("r".to_string()),
                        rel_types: vec!["KNOWS".to_string()],
                        direction: RelDirection::Outgoing,
                        properties: None,
                        min_hops: None,
                        max_hops: None,
                    }),
                    PatternElement::Node(NodePattern {
                        variable: Some("b".to_string()),
                        labels: vec!["Person".to_string()],
                        properties: None,
                    }),
                ],
            }],
        };

        let mut params = Params::new();
        params.insert(
            "__query_start_ms__".to_string(),
            Value::Int64(1_700_000_000_000),
        );
        let result = execute_create(vec![Record::new()], &pattern, &mut engine, &params, &(), &());
        let records = result.expect("should succeed");

        // Get the edge and verify it has _valid_from
        if let Some(Value::Edge(eid)) = records[0].get("r") {
            let edge = engine.get_edge(*eid).expect("edge exists");
            let valid_from_key = engine
                .catalog()
                .prop_key_id("_valid_from")
                .expect("_valid_from key");
            let has_valid_from = edge.properties.iter().any(|(k, _)| *k == valid_from_key);
            assert!(has_valid_from, "edge should have _valid_from property");

            // Also verify _created_at and _updated_at
            let created_key = engine
                .catalog()
                .prop_key_id("_created_at")
                .expect("_created_at key");
            let updated_key = engine
                .catalog()
                .prop_key_id("_updated_at")
                .expect("_updated_at key");
            assert!(edge.properties.iter().any(|(k, _)| *k == created_key));
            assert!(edge.properties.iter().any(|(k, _)| *k == updated_key));
        } else {
            panic!("expected edge value for 'r'");
        }
    }

    #[test]
    fn test_create_reuses_existing_variable() {
        let dir = tempdir().expect("tempdir");
        let mut engine = test_engine(dir.path());

        // Pre-create a node bound to "a"
        let existing_id = engine.create_node(vec![], vec![]);
        let mut initial_record = Record::new();
        initial_record.insert("a".to_string(), Value::Node(existing_id));

        let pattern = Pattern {
            chains: vec![PatternChain {
                elements: vec![
                    PatternElement::Node(NodePattern {
                        variable: Some("a".to_string()),
                        labels: vec![],
                        properties: None,
                    }),
                    PatternElement::Relationship(RelationshipPattern {
                        variable: None,
                        rel_types: vec!["KNOWS".to_string()],
                        direction: RelDirection::Outgoing,
                        properties: None,
                        min_hops: None,
                        max_hops: None,
                    }),
                    PatternElement::Node(NodePattern {
                        variable: Some("b".to_string()),
                        labels: vec![],
                        properties: None,
                    }),
                ],
            }],
        };

        let params = Params::new();
        let result = execute_create(vec![initial_record], &pattern, &mut engine, &params, &(), &());
        let records = result.expect("should succeed");

        // Should reuse existing node "a" and create only "b"
        assert_eq!(engine.node_count(), 2); // existing + new b
        assert_eq!(records[0].get("a"), Some(&Value::Node(existing_id)));
    }
}
