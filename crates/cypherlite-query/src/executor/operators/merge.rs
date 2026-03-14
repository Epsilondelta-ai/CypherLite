// MergeOp: MERGE clause execution (match-or-create with ON MATCH/ON CREATE SET)

use crate::executor::eval::eval;
use crate::executor::operators::create::{
    is_system_property, resolve_properties_mut, validate_no_system_properties,
    SYSTEM_PROP_CREATED_AT, SYSTEM_PROP_UPDATED_AT,
};
use crate::executor::{ExecutionError, Params, Record, ScalarFnLookup, Value};
use crate::parser::ast::*;
use cypherlite_core::{LabelRegistry, NodeId, PropertyValue};
use cypherlite_storage::StorageEngine;

/// Execute a MERGE pattern: for each source record, try to find existing
/// nodes/edges matching the pattern. If found, bind them (matched).
/// If not found, create them (created). Then apply ON MATCH SET or
/// ON CREATE SET accordingly.
pub fn execute_merge(
    source_records: Vec<Record>,
    pattern: &Pattern,
    on_match: &[SetItem],
    on_create: &[SetItem],
    engine: &mut StorageEngine,
    params: &Params,
    scalar_fns: &dyn ScalarFnLookup,
) -> Result<Vec<Record>, ExecutionError> {
    let mut results = Vec::new();

    for record in source_records {
        let mut new_record = record.clone();

        for chain in &pattern.chains {
            let created = merge_chain(chain, &mut new_record, engine, params, scalar_fns)?;

            // Apply ON MATCH SET or ON CREATE SET
            if created {
                apply_set_items(on_create, &new_record, engine, params, scalar_fns)?;
            } else {
                apply_set_items(on_match, &new_record, engine, params, scalar_fns)?;
            }
        }

        results.push(new_record);
    }

    Ok(results)
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

/// Merge a single pattern chain. Returns true if any element was created.
fn merge_chain(
    chain: &PatternChain,
    record: &mut Record,
    engine: &mut StorageEngine,
    params: &Params,
    scalar_fns: &dyn ScalarFnLookup,
) -> Result<bool, ExecutionError> {
    let mut elements = chain.elements.iter();
    let mut prev_var: Option<String> = None;
    let mut any_created = false;
    let temporal_enabled = engine.config().temporal_tracking_enabled;

    while let Some(element) = elements.next() {
        match element {
            PatternElement::Node(np) => {
                let var_name = np.variable.as_deref().unwrap_or("");

                // If variable already bound, skip
                if !var_name.is_empty() && record.contains_key(var_name) {
                    prev_var = Some(var_name.to_string());
                    continue;
                }

                // Try to find an existing node
                let label_ids: Vec<u32> = np
                    .labels
                    .iter()
                    .filter_map(|l| engine.label_id(l))
                    .collect();

                // Only attempt find if we have all labels resolved
                let all_labels_exist = np.labels.len() == label_ids.len();

                let props = resolve_find_properties(&np.properties, record, engine, params, scalar_fns)?;

                let found = if all_labels_exist && !label_ids.is_empty() {
                    find_node_with_index(engine, &label_ids, &props)
                } else if all_labels_exist && label_ids.is_empty() && !props.is_empty() {
                    // No labels but has properties - scan all nodes
                    engine.find_node(&[], &props)
                } else if !all_labels_exist {
                    // Some labels don't exist yet -> can't find
                    None
                } else {
                    None
                };

                match found {
                    Some(node_id) => {
                        if !var_name.is_empty() {
                            record.insert(var_name.to_string(), Value::Node(node_id));
                        }
                    }
                    None => {
                        // Validate no system properties
                        validate_no_system_properties(&np.properties)?;

                        // Create the node
                        let labels: Vec<u32> = np
                            .labels
                            .iter()
                            .map(|l| engine.get_or_create_label(l))
                            .collect();
                        let mut properties =
                            resolve_properties_mut(&np.properties, record, engine, params, scalar_fns)?;

                        if temporal_enabled {
                            inject_create_timestamps(&mut properties, engine, params);
                        }

                        let node_id = engine.create_node(labels, properties);
                        if !var_name.is_empty() {
                            record.insert(var_name.to_string(), Value::Node(node_id));
                        }
                        any_created = true;
                    }
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
                    message: "relationship must be followed by a node in MERGE pattern"
                        .to_string(),
                })?;

                let target_np = match next_node {
                    PatternElement::Node(np) => np,
                    _ => {
                        return Err(ExecutionError {
                            message: "expected node after relationship in MERGE".to_string(),
                        })
                    }
                };

                // Resolve or create target node (recursive merge logic)
                let target_var_name = target_np.variable.as_deref().unwrap_or("");
                let target_node_id = if !target_var_name.is_empty()
                    && record.contains_key(target_var_name)
                {
                    match record.get(target_var_name) {
                        Some(Value::Node(nid)) => *nid,
                        _ => {
                            return Err(ExecutionError {
                                message: format!(
                                    "variable '{}' is not a node",
                                    target_var_name
                                ),
                            })
                        }
                    }
                } else {
                    // Try to find or create target node
                    let target_label_ids: Vec<u32> = target_np
                        .labels
                        .iter()
                        .filter_map(|l| engine.label_id(l))
                        .collect();
                    let all_target_labels_exist =
                        target_np.labels.len() == target_label_ids.len();
                    let target_props =
                        resolve_find_properties(&target_np.properties, record, engine, params, scalar_fns)?;

                    let found_target = if all_target_labels_exist && !target_label_ids.is_empty() {
                        find_node_with_index(engine, &target_label_ids, &target_props)
                    } else {
                        None
                    };

                    match found_target {
                        Some(nid) => {
                            if !target_var_name.is_empty() {
                                record.insert(target_var_name.to_string(), Value::Node(nid));
                            }
                            nid
                        }
                        None => {
                            validate_no_system_properties(&target_np.properties)?;

                            let labels: Vec<u32> = target_np
                                .labels
                                .iter()
                                .map(|l| engine.get_or_create_label(l))
                                .collect();
                            let mut properties = resolve_properties_mut(
                                &target_np.properties,
                                record,
                                engine,
                                params,
                                scalar_fns,
                            )?;

                            if temporal_enabled {
                                inject_create_timestamps(&mut properties, engine, params);
                            }

                            let nid = engine.create_node(labels, properties);
                            if !target_var_name.is_empty() {
                                record.insert(target_var_name.to_string(), Value::Node(nid));
                            }
                            any_created = true;
                            nid
                        }
                    }
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
                            message: "no source node for relationship in MERGE".to_string(),
                        })
                    }
                };

                // Resolve relationship type
                let rel_type_name = rp.rel_types.first().ok_or_else(|| ExecutionError {
                    message: "MERGE relationship requires a type".to_string(),
                })?;

                let (start, end) = match rp.direction {
                    RelDirection::Outgoing | RelDirection::Undirected => {
                        (src_node_id, target_node_id)
                    }
                    RelDirection::Incoming => (target_node_id, src_node_id),
                };

                // Try to find existing edge
                let found_edge = engine
                    .rel_type_id(rel_type_name)
                    .and_then(|tid| engine.find_edge(start, end, tid));

                match found_edge {
                    Some(edge_id) => {
                        if let Some(rv) = &rp.variable {
                            record.insert(rv.clone(), Value::Edge(edge_id));
                        }
                    }
                    None => {
                        validate_no_system_properties(&rp.properties)?;

                        let rel_type_id = engine.get_or_create_rel_type(rel_type_name);
                        let mut rel_props =
                            resolve_properties_mut(&rp.properties, record, engine, params, scalar_fns)?;

                        if temporal_enabled {
                            inject_create_timestamps(&mut rel_props, engine, params);
                        }

                        let edge_id = engine
                            .create_edge(start, end, rel_type_id, rel_props)
                            .map_err(|e| ExecutionError {
                                message: format!("failed to create edge: {}", e),
                            })?;
                        if let Some(rv) = &rp.variable {
                            record.insert(rv.clone(), Value::Edge(edge_id));
                        }
                        any_created = true;
                    }
                }

                prev_var = if target_var_name.is_empty() {
                    None
                } else {
                    Some(target_var_name.to_string())
                };
            }
        }
    }

    Ok(any_created)
}

/// Find a node by labels and properties, using index if available.
///
/// For each property, checks if an index exists on (first_label, prop_key).
/// If found, uses `scan_nodes_by_property` (which leverages the index) to narrow
/// candidates, then verifies remaining labels and properties.
/// Falls back to `find_node` (linear scan) when no index is available.
fn find_node_with_index(
    engine: &StorageEngine,
    label_ids: &[u32],
    properties: &[(u32, PropertyValue)],
) -> Option<NodeId> {
    if let Some(&first_label) = label_ids.first() {
        // Try to use an index for any of the properties
        for (prop_key_id, prop_value) in properties {
            if engine.index_manager().find_index(first_label, *prop_key_id).is_some() {
                // Index exists: use scan_nodes_by_property for fast lookup
                let candidates = engine.scan_nodes_by_property(first_label, *prop_key_id, prop_value);
                // Filter candidates by remaining labels and properties
                for nid in candidates {
                    if let Some(node) = engine.get_node(nid) {
                        let has_all_labels = label_ids.iter().all(|lid| node.labels.contains(lid));
                        if !has_all_labels {
                            continue;
                        }
                        let has_all_props = properties.iter().all(|(key, val)| {
                            node.properties.iter().any(|(k, v)| k == key && v == val)
                        });
                        if has_all_props {
                            return Some(nid);
                        }
                    }
                }
                return None;
            }
        }
    }

    // Fallback to linear scan
    engine.find_node(label_ids, properties)
}

/// Resolve properties for finding (read-only: use existing prop key IDs).
fn resolve_find_properties(
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
                if let Some(key_id) = engine.catalog().prop_key_id(key) {
                    result.push((key_id, pv));
                } else {
                    // Property key doesn't exist, so no node can match
                    return Ok(vec![]);
                }
            }
            Ok(result)
        }
    }
}

/// Apply SET items (used for ON MATCH SET and ON CREATE SET).
fn apply_set_items(
    items: &[SetItem],
    record: &Record,
    engine: &mut StorageEngine,
    params: &Params,
    scalar_fns: &dyn ScalarFnLookup,
) -> Result<(), ExecutionError> {
    for item in items {
        match item {
            SetItem::Property { target, value } => {
                apply_set_property(target, value, record, engine, params, scalar_fns)?;
            }
        }
    }
    Ok(())
}

/// Apply a single SET property (reused from set_props logic).
fn apply_set_property(
    target: &Expression,
    value_expr: &Expression,
    record: &Record,
    engine: &mut StorageEngine,
    params: &Params,
    scalar_fns: &dyn ScalarFnLookup,
) -> Result<(), ExecutionError> {
    match target {
        Expression::Property(var_expr, prop_name) => {
            // V-003: Block user writes to system properties
            if is_system_property(prop_name) {
                return Err(ExecutionError {
                    message: format!("System property is read-only: {}", prop_name),
                });
            }

            let entity = eval(var_expr, record, &*engine, params, scalar_fns)?;
            let new_value = eval(value_expr, record, &*engine, params, scalar_fns)?;
            let pv = PropertyValue::try_from(new_value).map_err(|e| ExecutionError {
                message: format!("invalid property value: {}", e),
            })?;

            let temporal_enabled = engine.config().temporal_tracking_enabled;

            match entity {
                Value::Node(nid) => {
                    let prop_key_id = engine.get_or_create_prop_key(prop_name);
                    let node = engine.get_node(nid).ok_or_else(|| ExecutionError {
                        message: format!("node {} not found", nid.0),
                    })?;
                    let mut props = node.properties.clone();

                    let mut found = false;
                    for (k, v) in &mut props {
                        if *k == prop_key_id {
                            *v = pv.clone();
                            found = true;
                            break;
                        }
                    }
                    if !found {
                        props.push((prop_key_id, pv));
                    }

                    // V-002: Update _updated_at
                    if temporal_enabled {
                        let now = get_query_timestamp(params);
                        let updated_key = engine.get_or_create_prop_key(SYSTEM_PROP_UPDATED_AT);
                        let mut updated_found = false;
                        for (k, v) in props.iter_mut() {
                            if *k == updated_key {
                                *v = PropertyValue::DateTime(now);
                                updated_found = true;
                                break;
                            }
                        }
                        if !updated_found {
                            props.push((updated_key, PropertyValue::DateTime(now)));
                        }
                    }

                    engine.update_node(nid, props).map_err(|e| ExecutionError {
                        message: format!("failed to update node: {}", e),
                    })?;
                }
                Value::Null => {}
                _ => {
                    return Err(ExecutionError {
                        message: "SET target must be a node or edge property".to_string(),
                    });
                }
            }
        }
        _ => {
            return Err(ExecutionError {
                message: "SET target must be a property access expression".to_string(),
            });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::executor::Record;
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

    // TASK-087: Basic MERGE creates node when not found
    #[test]
    fn test_merge_creates_node_when_not_found() {
        let dir = tempdir().expect("tempdir");
        let mut engine = test_engine(dir.path());

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
        let result = execute_merge(
            vec![Record::new()],
            &pattern,
            &[],
            &[],
            &mut engine,
            &params,
            &(),
        );
        let records = result.expect("should succeed");
        assert_eq!(records.len(), 1);
        assert!(records[0].contains_key("n"));
        assert_eq!(engine.node_count(), 1);
    }

    // TASK-089: MERGE idempotency - running same MERGE twice should not duplicate
    #[test]
    fn test_merge_idempotent() {
        let dir = tempdir().expect("tempdir");
        let mut engine = test_engine(dir.path());

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

        // First MERGE: creates node
        execute_merge(
            vec![Record::new()],
            &pattern,
            &[],
            &[],
            &mut engine,
            &params,
            &(),
        )
        .expect("first merge");
        assert_eq!(engine.node_count(), 1);

        // Second MERGE: should find existing node
        let records = execute_merge(
            vec![Record::new()],
            &pattern,
            &[],
            &[],
            &mut engine,
            &params,
            &(),
        )
        .expect("second merge");
        assert_eq!(engine.node_count(), 1); // Still 1, not 2
        assert!(records[0].contains_key("n"));
    }

    // TASK-088: ON CREATE SET is applied when node is created
    #[test]
    fn test_merge_on_create_set() {
        let dir = tempdir().expect("tempdir");
        let mut engine = test_engine(dir.path());

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

        let on_create = vec![SetItem::Property {
            target: Expression::Property(
                Box::new(Expression::Variable("n".to_string())),
                "created".to_string(),
            ),
            value: Expression::Literal(Literal::Bool(true)),
        }];

        let params = Params::new();
        let records = execute_merge(
            vec![Record::new()],
            &pattern,
            &[],
            &on_create,
            &mut engine,
            &params,
            &(),
        )
        .expect("merge");

        // Verify ON CREATE SET was applied
        if let Some(Value::Node(nid)) = records[0].get("n") {
            let node = engine.get_node(*nid).expect("node exists");
            let created_key = engine.catalog().prop_key_id("created").expect("key");
            let created_val = node
                .properties
                .iter()
                .find(|(k, _)| *k == created_key)
                .map(|(_, v)| v);
            assert_eq!(created_val, Some(&PropertyValue::Bool(true)));
        } else {
            panic!("expected node value");
        }
    }

    // TASK-088: ON MATCH SET is applied when node already exists
    #[test]
    fn test_merge_on_match_set() {
        let dir = tempdir().expect("tempdir");
        let mut engine = test_engine(dir.path());

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

        let on_match = vec![SetItem::Property {
            target: Expression::Property(
                Box::new(Expression::Variable("n".to_string())),
                "seen".to_string(),
            ),
            value: Expression::Literal(Literal::Bool(true)),
        }];

        let params = Params::new();

        // First MERGE: creates (ON MATCH should NOT apply)
        execute_merge(
            vec![Record::new()],
            &pattern,
            &on_match,
            &[],
            &mut engine,
            &params,
            &(),
        )
        .expect("first merge");

        // Second MERGE: matches (ON MATCH SHOULD apply)
        let records = execute_merge(
            vec![Record::new()],
            &pattern,
            &on_match,
            &[],
            &mut engine,
            &params,
            &(),
        )
        .expect("second merge");

        if let Some(Value::Node(nid)) = records[0].get("n") {
            let node = engine.get_node(*nid).expect("node exists");
            let seen_key = engine.catalog().prop_key_id("seen").expect("key");
            let seen_val = node
                .properties
                .iter()
                .find(|(k, _)| *k == seen_key)
                .map(|(_, v)| v);
            assert_eq!(seen_val, Some(&PropertyValue::Bool(true)));
        } else {
            panic!("expected node value");
        }
    }

    // TASK-087: MERGE relationship
    #[test]
    fn test_merge_creates_relationship() {
        let dir = tempdir().expect("tempdir");
        let mut engine = test_engine(dir.path());

        // Pre-create two nodes
        let person_label = engine.get_or_create_label("Person");
        let name_key = engine.get_or_create_prop_key("name");
        let n1 = engine.create_node(
            vec![person_label],
            vec![(name_key, PropertyValue::String("Alice".into()))],
        );
        let n2 = engine.create_node(
            vec![person_label],
            vec![(name_key, PropertyValue::String("Bob".into()))],
        );

        // Create initial record with both nodes bound
        let mut initial_record = Record::new();
        initial_record.insert("a".to_string(), Value::Node(n1));
        initial_record.insert("b".to_string(), Value::Node(n2));

        let pattern = Pattern {
            chains: vec![PatternChain {
                elements: vec![
                    PatternElement::Node(NodePattern {
                        variable: Some("a".to_string()),
                        labels: vec![],
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
                        labels: vec![],
                        properties: None,
                    }),
                ],
            }],
        };

        let params = Params::new();

        // First MERGE: creates edge
        let records = execute_merge(
            vec![initial_record.clone()],
            &pattern,
            &[],
            &[],
            &mut engine,
            &params,
            &(),
        )
        .expect("merge");
        assert_eq!(engine.edge_count(), 1);
        assert!(records[0].contains_key("r"));

        // Second MERGE: should find existing edge (idempotent)
        execute_merge(
            vec![initial_record],
            &pattern,
            &[],
            &[],
            &mut engine,
            &params,
            &(),
        )
        .expect("second merge");
        assert_eq!(engine.edge_count(), 1); // Still 1
    }

    // TASK-113: MERGE with index-assisted node lookup
    #[test]
    fn test_merge_uses_index_for_node_lookup() {
        let dir = tempdir().expect("tempdir");
        let mut engine = test_engine(dir.path());

        let person_label = engine.get_or_create_label("Person");
        let name_key = engine.get_or_create_prop_key("name");

        // Create a node
        let n1 = engine.create_node(
            vec![person_label],
            vec![(name_key, PropertyValue::String("Alice".into()))],
        );

        // Create index on Person(name)
        engine
            .index_manager_mut()
            .create_index("idx_person_name".into(), person_label, name_key)
            .expect("create index");
        // Backfill
        if let Some(idx) = engine.index_manager_mut().find_index_mut(person_label, name_key) {
            idx.insert(&PropertyValue::String("Alice".into()), n1);
        }

        // MERGE should find via index
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
        let records = execute_merge(
            vec![Record::new()],
            &pattern,
            &[],
            &[],
            &mut engine,
            &params,
            &(),
        )
        .expect("merge");

        assert_eq!(records.len(), 1);
        assert_eq!(engine.node_count(), 1); // Should find, not create
    }

    // TASK-113: MERGE with index, node not in index
    #[test]
    fn test_merge_creates_when_not_in_index() {
        let dir = tempdir().expect("tempdir");
        let mut engine = test_engine(dir.path());

        let person_label = engine.get_or_create_label("Person");
        let name_key = engine.get_or_create_prop_key("name");

        // Create index but no nodes
        engine
            .index_manager_mut()
            .create_index("idx_person_name".into(), person_label, name_key)
            .expect("create index");

        let pattern = Pattern {
            chains: vec![PatternChain {
                elements: vec![PatternElement::Node(NodePattern {
                    variable: Some("n".to_string()),
                    labels: vec!["Person".to_string()],
                    properties: Some(vec![(
                        "name".to_string(),
                        Expression::Literal(Literal::String("Bob".into())),
                    )]),
                })],
            }],
        };

        let params = Params::new();
        let records = execute_merge(
            vec![Record::new()],
            &pattern,
            &[],
            &[],
            &mut engine,
            &params,
            &(),
        )
        .expect("merge");

        assert_eq!(records.len(), 1);
        assert_eq!(engine.node_count(), 1); // Should create
    }

    // TASK-113: find_node_with_index falls back when no index
    #[test]
    fn test_find_node_with_index_fallback() {
        let dir = tempdir().expect("tempdir");
        let mut engine = test_engine(dir.path());

        let person_label = engine.get_or_create_label("Person");
        let name_key = engine.get_or_create_prop_key("name");

        let n1 = engine.create_node(
            vec![person_label],
            vec![(name_key, PropertyValue::String("Alice".into()))],
        );

        // No index - should still find via fallback
        let found = find_node_with_index(
            &engine,
            &[person_label],
            &[(name_key, PropertyValue::String("Alice".into()))],
        );
        assert_eq!(found, Some(n1));
    }

    // TASK-113: find_node_with_index with empty labels
    #[test]
    fn test_find_node_with_index_no_labels() {
        let dir = tempdir().expect("tempdir");
        let mut engine = test_engine(dir.path());

        let name_key = engine.get_or_create_prop_key("name");
        let _n1 = engine.create_node(
            vec![],
            vec![(name_key, PropertyValue::String("Alice".into()))],
        );

        // No labels -> fallback to find_node
        let found = find_node_with_index(
            &engine,
            &[],
            &[(name_key, PropertyValue::String("Alice".into()))],
        );
        // find_node with empty labels scans all nodes
        assert!(found.is_some());
    }

    // TASK-113: find_node_with_index with index, multi-label check
    #[test]
    fn test_find_node_with_index_multi_label() {
        let dir = tempdir().expect("tempdir");
        let mut engine = test_engine(dir.path());

        let person_label = engine.get_or_create_label("Person");
        let employee_label = engine.get_or_create_label("Employee");
        let name_key = engine.get_or_create_prop_key("name");

        // Create node with both labels
        let n1 = engine.create_node(
            vec![person_label, employee_label],
            vec![(name_key, PropertyValue::String("Alice".into()))],
        );

        // Create index on Person(name)
        engine
            .index_manager_mut()
            .create_index("idx_person_name".into(), person_label, name_key)
            .expect("create index");
        if let Some(idx) = engine.index_manager_mut().find_index_mut(person_label, name_key) {
            idx.insert(&PropertyValue::String("Alice".into()), n1);
        }

        // Should find by both labels
        let found = find_node_with_index(
            &engine,
            &[person_label, employee_label],
            &[(name_key, PropertyValue::String("Alice".into()))],
        );
        assert_eq!(found, Some(n1));
    }
}
