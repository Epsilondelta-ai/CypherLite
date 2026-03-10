// NodeScanOp: scans all nodes or nodes by label

use crate::executor::{Record, Value};
use cypherlite_storage::StorageEngine;

/// Scan nodes from the engine, optionally filtered by label.
/// Each node produces a Record with the variable bound to Value::Node(node_id).
pub fn execute_node_scan(
    variable: &str,
    label_id: Option<u32>,
    engine: &StorageEngine,
) -> Vec<Record> {
    let nodes = match label_id {
        Some(lid) => engine.scan_nodes_by_label(lid),
        None => engine.scan_nodes(),
    };

    nodes
        .into_iter()
        .map(|node| {
            let mut record = Record::new();
            record.insert(variable.to_string(), Value::Node(node.node_id));
            record
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use cypherlite_core::{DatabaseConfig, LabelRegistry, SyncMode};
    use tempfile::tempdir;

    fn test_engine(dir: &std::path::Path) -> StorageEngine {
        let config = DatabaseConfig {
            path: dir.join("test.cyl"),
            wal_sync_mode: SyncMode::Normal,
            ..Default::default()
        };
        StorageEngine::open(config).expect("open")
    }

    // EXEC-T001: NodeScan with label filter
    #[test]
    fn test_node_scan_with_label_filter() {
        let dir = tempdir().expect("tempdir");
        let mut engine = test_engine(dir.path());

        let person_label = engine.get_or_create_label("Person");
        let company_label = engine.get_or_create_label("Company");

        engine.create_node(vec![person_label], vec![]);
        engine.create_node(vec![person_label], vec![]);
        engine.create_node(vec![company_label], vec![]);

        let records = execute_node_scan("n", Some(person_label), &engine);
        assert_eq!(records.len(), 2);

        for record in &records {
            assert!(record.contains_key("n"));
            assert!(matches!(record.get("n"), Some(Value::Node(_))));
        }
    }

    #[test]
    fn test_node_scan_all_nodes() {
        let dir = tempdir().expect("tempdir");
        let mut engine = test_engine(dir.path());

        engine.create_node(vec![0], vec![]);
        engine.create_node(vec![1], vec![]);
        engine.create_node(vec![2], vec![]);

        let records = execute_node_scan("n", None, &engine);
        assert_eq!(records.len(), 3);
    }

    #[test]
    fn test_node_scan_empty_database() {
        let dir = tempdir().expect("tempdir");
        let engine = test_engine(dir.path());

        let records = execute_node_scan("n", Some(0), &engine);
        assert!(records.is_empty());
    }
}
