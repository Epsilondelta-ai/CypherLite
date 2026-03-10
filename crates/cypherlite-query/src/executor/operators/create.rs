// CreateOp: node and edge creation via storage engine

use crate::executor::eval::eval;
use crate::executor::{ExecutionError, Params, Record, Value};
use crate::parser::ast::*;
use cypherlite_core::{LabelRegistry, PropertyValue};
use cypherlite_storage::StorageEngine;

/// Create nodes and edges from a pattern.
/// Walks each pattern chain, creating nodes and edges as specified.
pub fn execute_create(
    source_records: Vec<Record>,
    pattern: &Pattern,
    engine: &mut StorageEngine,
    params: &Params,
) -> Result<Vec<Record>, ExecutionError> {
    let mut results = Vec::new();

    for record in source_records {
        let mut new_record = record.clone();

        for chain in &pattern.chains {
            create_chain(chain, &mut new_record, engine, params)?;
        }

        results.push(new_record);
    }

    Ok(results)
}

/// Create nodes and edges from a single pattern chain.
fn create_chain(
    chain: &PatternChain,
    record: &mut Record,
    engine: &mut StorageEngine,
    params: &Params,
) -> Result<(), ExecutionError> {
    let mut elements = chain.elements.iter();
    let mut prev_var: Option<String> = None;

    while let Some(element) = elements.next() {
        match element {
            PatternElement::Node(np) => {
                let var_name = np.variable.as_deref().unwrap_or("");

                // If variable already bound in record, skip node creation
                if !var_name.is_empty() && record.contains_key(var_name) {
                    prev_var = Some(var_name.to_string());
                    continue;
                }

                // Resolve labels
                let labels: Vec<u32> = np
                    .labels
                    .iter()
                    .map(|l| engine.get_or_create_label(l))
                    .collect();

                // Resolve properties
                let properties = resolve_properties(&np.properties, record, engine, params)?;

                let node_id = engine.create_node(labels, properties);

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
                    let labels: Vec<u32> = target_np
                        .labels
                        .iter()
                        .map(|l| engine.get_or_create_label(l))
                        .collect();
                    let properties =
                        resolve_properties(&target_np.properties, record, engine, params)?;
                    let nid = engine.create_node(labels, properties);
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
                let rel_type_id = rp
                    .rel_types
                    .first()
                    .map(|t| engine.get_or_create_rel_type(t))
                    .ok_or_else(|| ExecutionError {
                        message: "CREATE relationship requires a type".to_string(),
                    })?;

                // Resolve relationship properties
                let rel_props = resolve_properties(&rp.properties, record, engine, params)?;

                // Create edge based on direction
                let (start, end) = match rp.direction {
                    RelDirection::Outgoing | RelDirection::Undirected => {
                        (src_node_id, target_node_id)
                    }
                    RelDirection::Incoming => (target_node_id, src_node_id),
                };

                let edge_id = engine
                    .create_edge(start, end, rel_type_id, rel_props)
                    .map_err(|e| ExecutionError {
                        message: format!("failed to create edge: {}", e),
                    })?;

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
) -> Result<Vec<(u32, PropertyValue)>, ExecutionError> {
    match props {
        None => Ok(vec![]),
        Some(map) => {
            let mut result = Vec::new();
            for (key, expr) in map {
                let value = eval(expr, record, engine, params)?;
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
) -> Result<Vec<(u32, PropertyValue)>, ExecutionError> {
    match props {
        None => Ok(vec![]),
        Some(map) => {
            let mut result = Vec::new();
            for (key, expr) in map {
                let value = eval(expr, record, &*engine, params)?;
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
        let result = execute_create(vec![Record::new()], &pattern, &mut engine, &params);
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
        let result = execute_create(vec![Record::new()], &pattern, &mut engine, &params);
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
        let result = execute_create(vec![Record::new()], &pattern, &mut engine, &params);
        let records = result.expect("should succeed");
        assert_eq!(records.len(), 1);
        assert!(records[0].contains_key("a"));
        assert!(records[0].contains_key("r"));
        assert!(records[0].contains_key("b"));
        assert_eq!(engine.node_count(), 2);
        assert_eq!(engine.edge_count(), 1);
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
        let result = execute_create(vec![initial_record], &pattern, &mut engine, &params);
        let records = result.expect("should succeed");

        // Should reuse existing node "a" and create only "b"
        assert_eq!(engine.node_count(), 2); // existing + new b
        assert_eq!(records[0].get("a"), Some(&Value::Node(existing_id)));
    }
}
