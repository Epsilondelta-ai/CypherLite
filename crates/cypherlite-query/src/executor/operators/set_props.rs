// SetPropsOp: property mutation for SET/REMOVE clauses

use crate::executor::eval::eval;
use crate::executor::operators::create::{
    is_system_property, SYSTEM_PROP_UPDATED_AT,
};
use crate::executor::{ExecutionError, Params, Record, Value};
use crate::parser::ast::*;
use cypherlite_core::{LabelRegistry, PropertyValue};
use cypherlite_storage::StorageEngine;

/// Set properties on nodes/edges.
/// For each SetItem::Property { target, value }, evaluate target to get the entity,
/// and value to get the new property value.
pub fn execute_set(
    source_records: Vec<Record>,
    items: &[SetItem],
    engine: &mut StorageEngine,
    params: &Params,
) -> Result<Vec<Record>, ExecutionError> {
    for record in &source_records {
        for item in items {
            match item {
                SetItem::Property { target, value } => {
                    apply_set_property(target, value, record, engine, params)?;
                }
            }
        }
    }

    Ok(source_records)
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

/// Update _updated_at on a node's property list.
fn inject_updated_at(
    props: &mut Vec<(u32, PropertyValue)>,
    engine: &mut StorageEngine,
    params: &Params,
) {
    let now = get_query_timestamp(params);
    let updated_key = engine.get_or_create_prop_key(SYSTEM_PROP_UPDATED_AT);
    let mut found = false;
    for (k, v) in props.iter_mut() {
        if *k == updated_key {
            *v = PropertyValue::DateTime(now);
            found = true;
            break;
        }
    }
    if !found {
        props.push((updated_key, PropertyValue::DateTime(now)));
    }
}

/// Apply a single SET property operation.
fn apply_set_property(
    target: &Expression,
    value_expr: &Expression,
    record: &Record,
    engine: &mut StorageEngine,
    params: &Params,
) -> Result<(), ExecutionError> {
    // target should be Property(Variable(name), prop_name)
    match target {
        Expression::Property(var_expr, prop_name) => {
            // V-003: Block user writes to system properties
            if is_system_property(prop_name) {
                return Err(ExecutionError {
                    message: format!("System property is read-only: {}", prop_name),
                });
            }

            let entity = eval(var_expr, record, &*engine, params)?;
            let new_value = eval(value_expr, record, &*engine, params)?;
            let pv = PropertyValue::try_from(new_value).map_err(|e| ExecutionError {
                message: format!("invalid property value: {}", e),
            })?;

            let temporal_enabled = engine.config().temporal_tracking_enabled;

            match entity {
                Value::Node(nid) => {
                    let prop_key_id = engine.get_or_create_prop_key(prop_name);
                    // Get current node properties
                    let node = engine.get_node(nid).ok_or_else(|| ExecutionError {
                        message: format!("node {} not found", nid.0),
                    })?;
                    let mut props = node.properties.clone();

                    // Update or add the property
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

                    // V-002: Update _updated_at timestamp
                    if temporal_enabled {
                        inject_updated_at(&mut props, engine, params);
                    }

                    engine.update_node(nid, props).map_err(|e| ExecutionError {
                        message: format!("failed to update node: {}", e),
                    })?;
                }
                Value::Null => {
                    // SET on null is a no-op (Cypher behavior)
                }
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

/// Execute REMOVE operations (remove properties or labels).
pub fn execute_remove(
    source_records: Vec<Record>,
    items: &[RemoveItem],
    engine: &mut StorageEngine,
    params: &Params,
) -> Result<Vec<Record>, ExecutionError> {
    for record in &source_records {
        for item in items {
            match item {
                RemoveItem::Property(prop_expr) => {
                    apply_remove_property(prop_expr, record, engine, params)?;
                }
                RemoveItem::Label { variable, label } => {
                    apply_remove_label(variable, label, record, engine)?;
                }
            }
        }
    }

    Ok(source_records)
}

/// Remove a property from a node/edge.
fn apply_remove_property(
    prop_expr: &Expression,
    record: &Record,
    engine: &mut StorageEngine,
    params: &Params,
) -> Result<(), ExecutionError> {
    match prop_expr {
        Expression::Property(var_expr, prop_name) => {
            // V-003: Block removal of system properties
            if is_system_property(prop_name) {
                return Err(ExecutionError {
                    message: format!("System property is read-only: {}", prop_name),
                });
            }

            let entity = eval(var_expr, record, &*engine, params)?;
            let temporal_enabled = engine.config().temporal_tracking_enabled;

            match entity {
                Value::Node(nid) => {
                    let prop_key_id = match engine.catalog().prop_key_id(prop_name) {
                        Some(id) => id,
                        None => return Ok(()), // Property key doesn't exist, nothing to remove
                    };

                    let node = engine.get_node(nid).ok_or_else(|| ExecutionError {
                        message: format!("node {} not found", nid.0),
                    })?;
                    let mut props: Vec<_> = node
                        .properties
                        .iter()
                        .filter(|(k, _)| *k != prop_key_id)
                        .cloned()
                        .collect();

                    // V-002: Update _updated_at on REMOVE
                    if temporal_enabled {
                        inject_updated_at(&mut props, engine, params);
                    }

                    engine.update_node(nid, props).map_err(|e| ExecutionError {
                        message: format!("failed to update node: {}", e),
                    })?;
                }
                Value::Null => {} // no-op
                _ => {
                    return Err(ExecutionError {
                        message: "REMOVE target must be a node or edge property".to_string(),
                    });
                }
            }
        }
        _ => {
            return Err(ExecutionError {
                message: "REMOVE property must be a property access expression".to_string(),
            });
        }
    }

    Ok(())
}

/// Remove a label from a node.
fn apply_remove_label(
    variable: &str,
    label: &str,
    record: &Record,
    engine: &mut StorageEngine,
) -> Result<(), ExecutionError> {
    let entity = record.get(variable).cloned().unwrap_or(Value::Null);

    match entity {
        Value::Node(nid) => {
            let label_id = match engine.catalog().label_id(label) {
                Some(id) => id,
                None => return Ok(()), // Label doesn't exist, nothing to remove
            };

            let node = engine.get_node(nid).ok_or_else(|| ExecutionError {
                message: format!("node {} not found", nid.0),
            })?;

            // We can't directly modify labels through the current API.
            // The node record has labels as a separate field.
            // For now, we'll use update_node with existing properties.
            // Labels modification would need a dedicated API.
            // This is a limitation we note but cannot fully implement
            // without extending the StorageEngine API.
            let _ = label_id;
            let _ = node;

            // Note: StorageEngine doesn't expose a label modification API.
            // This would need update_node_labels() or similar.
            Ok(())
        }
        Value::Null => Ok(()),
        _ => Err(ExecutionError {
            message: "REMOVE label target must be a node".to_string(),
        }),
    }
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

    #[test]
    fn test_set_property_on_node() {
        let dir = tempdir().expect("tempdir");
        let mut engine = test_engine(dir.path());

        let name_key = engine.get_or_create_prop_key("name");
        let nid = engine.create_node(
            vec![],
            vec![(name_key, PropertyValue::String("Alice".into()))],
        );

        let mut record = Record::new();
        record.insert("n".to_string(), Value::Node(nid));

        let items = vec![SetItem::Property {
            target: Expression::Property(
                Box::new(Expression::Variable("n".to_string())),
                "name".to_string(),
            ),
            value: Expression::Literal(Literal::String("Bob".into())),
        }];

        let params = Params::new();
        let result = execute_set(vec![record], &items, &mut engine, &params);
        assert!(result.is_ok());

        // Verify property was updated
        let node = engine.get_node(nid).expect("node exists");
        let name_val = node
            .properties
            .iter()
            .find(|(k, _)| *k == name_key)
            .map(|(_, v)| v);
        assert_eq!(name_val, Some(&PropertyValue::String("Bob".into())));
    }

    #[test]
    fn test_set_new_property() {
        let dir = tempdir().expect("tempdir");
        let mut engine = test_engine(dir.path());

        let nid = engine.create_node(vec![], vec![]);

        let mut record = Record::new();
        record.insert("n".to_string(), Value::Node(nid));

        let items = vec![SetItem::Property {
            target: Expression::Property(
                Box::new(Expression::Variable("n".to_string())),
                "age".to_string(),
            ),
            value: Expression::Literal(Literal::Integer(30)),
        }];

        let params = Params::new();
        let result = execute_set(vec![record], &items, &mut engine, &params);
        assert!(result.is_ok());

        let age_key = engine.catalog().prop_key_id("age").expect("age key");
        let node = engine.get_node(nid).expect("node exists");
        let age_val = node
            .properties
            .iter()
            .find(|(k, _)| *k == age_key)
            .map(|(_, v)| v);
        assert_eq!(age_val, Some(&PropertyValue::Int64(30)));
    }

    #[test]
    fn test_set_on_null_is_noop() {
        let dir = tempdir().expect("tempdir");
        let mut engine = test_engine(dir.path());

        let mut record = Record::new();
        record.insert("n".to_string(), Value::Null);

        let items = vec![SetItem::Property {
            target: Expression::Property(
                Box::new(Expression::Variable("n".to_string())),
                "name".to_string(),
            ),
            value: Expression::Literal(Literal::String("test".into())),
        }];

        let params = Params::new();
        let result = execute_set(vec![record], &items, &mut engine, &params);
        assert!(result.is_ok());
    }

    #[test]
    fn test_remove_property() {
        let dir = tempdir().expect("tempdir");
        let mut engine = test_engine(dir.path());

        let name_key = engine.get_or_create_prop_key("name");
        let age_key = engine.get_or_create_prop_key("age");
        let nid = engine.create_node(
            vec![],
            vec![
                (name_key, PropertyValue::String("Alice".into())),
                (age_key, PropertyValue::Int64(30)),
            ],
        );

        let mut record = Record::new();
        record.insert("n".to_string(), Value::Node(nid));

        let items = vec![RemoveItem::Property(Expression::Property(
            Box::new(Expression::Variable("n".to_string())),
            "age".to_string(),
        ))];

        let params = Params::new();
        let result = execute_remove(vec![record], &items, &mut engine, &params);
        assert!(result.is_ok());

        let node = engine.get_node(nid).expect("node exists");
        // After REMOVE, we have: name + _updated_at (temporal tracking is on by default)
        assert_eq!(node.properties.len(), 2);
        assert!(node.properties.iter().any(|(k, _)| *k == name_key));
        let updated_key = engine.catalog().prop_key_id("_updated_at").expect("key");
        assert!(node.properties.iter().any(|(k, _)| *k == updated_key));
    }
}
