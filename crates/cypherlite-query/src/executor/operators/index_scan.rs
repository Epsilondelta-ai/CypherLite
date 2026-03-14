// IndexScanOp: looks up nodes via property index, falls back to label scan + filter

use crate::executor::eval::eval;
use crate::executor::{ExecutionError, Params, Record, ScalarFnLookup, Value};
use crate::parser::ast::Expression;
use cypherlite_core::{LabelRegistry, PropertyValue};
use cypherlite_storage::StorageEngine;

/// Execute an index scan: look up nodes by label + property value.
///
/// If an index exists for (label_id, prop_key), uses fast index lookup.
/// Otherwise falls back to label scan + property filter (same result, slower).
pub fn execute_index_scan(
    variable: &str,
    label_id: u32,
    prop_key: &str,
    lookup_value: &Expression,
    engine: &StorageEngine,
    params: &Params,
    scalar_fns: &dyn ScalarFnLookup,
) -> Result<Vec<Record>, ExecutionError> {
    // Evaluate the lookup value expression
    let empty_record = Record::new();
    let val = eval(lookup_value, &empty_record, engine, params, scalar_fns)?;

    // Convert Value to PropertyValue for index lookup
    let pv = PropertyValue::try_from(val).map_err(|e| ExecutionError {
        message: format!("invalid index lookup value: {}", e),
    })?;

    // Resolve prop_key name to ID
    let prop_key_id = match engine.prop_key_id(prop_key) {
        Some(id) => id,
        None => {
            // Property key does not exist, so no nodes can match
            return Ok(vec![]);
        }
    };

    // Use scan_nodes_by_property which automatically uses index if available
    let node_ids = engine.scan_nodes_by_property(label_id, prop_key_id, &pv);

    let records = node_ids
        .into_iter()
        .map(|nid| {
            let mut record = Record::new();
            record.insert(variable.to_string(), Value::Node(nid));
            record
        })
        .collect();

    Ok(records)
}

#[cfg(test)]
mod tests {
    use super::*;
    use cypherlite_core::{DatabaseConfig, SyncMode};
    use cypherlite_storage::StorageEngine;
    use tempfile::tempdir;

    use crate::parser::ast::*;

    fn test_engine(dir: &std::path::Path) -> StorageEngine {
        let config = DatabaseConfig {
            path: dir.join("test.cyl"),
            wal_sync_mode: SyncMode::Normal,
            ..Default::default()
        };
        StorageEngine::open(config).expect("open")
    }

    // TASK-111: IndexScan with index present uses fast lookup
    #[test]
    fn test_index_scan_with_index() {
        let dir = tempdir().expect("tempdir");
        let mut engine = test_engine(dir.path());

        let person_label = engine.get_or_create_label("Person");
        let name_key = engine.get_or_create_prop_key("name");

        // Create nodes
        let _n1 = engine.create_node(
            vec![person_label],
            vec![(name_key, PropertyValue::String("Alice".into()))],
        );
        let _n2 = engine.create_node(
            vec![person_label],
            vec![(name_key, PropertyValue::String("Bob".into()))],
        );

        // Create index
        engine
            .index_manager_mut()
            .create_index("idx_person_name".into(), person_label, name_key)
            .expect("create index");

        // Backfill index
        let nodes: Vec<(cypherlite_core::NodeId, Vec<(u32, PropertyValue)>)> = engine
            .scan_nodes_by_label(person_label)
            .iter()
            .map(|n| (n.node_id, n.properties.clone()))
            .collect();
        for (nid, props) in &nodes {
            for (pk, v) in props {
                if *pk == name_key {
                    if let Some(idx) = engine.index_manager_mut().find_index_mut(person_label, name_key) {
                        idx.insert(v, *nid);
                    }
                }
            }
        }

        let params = Params::new();
        let lookup = Expression::Literal(Literal::String("Alice".into()));
        let records = execute_index_scan("n", person_label, "name", &lookup, &engine, &params, &()).expect("should succeed");

        assert_eq!(records.len(), 1);
        assert!(matches!(records[0].get("n"), Some(Value::Node(_))));
    }

    // TASK-111: IndexScan without index falls back to linear scan
    #[test]
    fn test_index_scan_without_index_falls_back() {
        let dir = tempdir().expect("tempdir");
        let mut engine = test_engine(dir.path());

        let person_label = engine.get_or_create_label("Person");
        let name_key = engine.get_or_create_prop_key("name");

        engine.create_node(
            vec![person_label],
            vec![(name_key, PropertyValue::String("Alice".into()))],
        );
        engine.create_node(
            vec![person_label],
            vec![(name_key, PropertyValue::String("Bob".into()))],
        );

        // No index created - should still work via fallback
        let params = Params::new();
        let lookup = Expression::Literal(Literal::String("Alice".into()));
        let records = execute_index_scan("n", person_label, "name", &lookup, &engine, &params, &()).expect("should succeed");

        assert_eq!(records.len(), 1);
    }

    // TASK-111: IndexScan with non-existent property key returns empty
    #[test]
    fn test_index_scan_unknown_property_key() {
        let dir = tempdir().expect("tempdir");
        let mut engine = test_engine(dir.path());

        let person_label = engine.get_or_create_label("Person");

        let params = Params::new();
        let lookup = Expression::Literal(Literal::String("Alice".into()));
        let records = execute_index_scan("n", person_label, "nonexistent", &lookup, &engine, &params, &()).expect("should succeed");

        assert!(records.is_empty());
    }
}
