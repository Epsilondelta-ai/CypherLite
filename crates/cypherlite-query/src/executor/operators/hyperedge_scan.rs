// HyperEdgeScan operator: scans all hyperedge entities from HyperEdgeStore.
// Each hyperedge produces a Record with the variable bound to Value::Hyperedge(id).

use crate::executor::{Record, Value};
use cypherlite_core::HyperEdgeId;
use cypherlite_storage::StorageEngine;

/// Scan all hyperedges from the engine.
/// Each hyperedge produces a Record with the variable bound to Value::Hyperedge(id).
pub fn execute_hyperedge_scan(variable: &str, engine: &StorageEngine) -> Vec<Record> {
    engine
        .scan_hyperedges()
        .into_iter()
        .map(|he| {
            let mut record = Record::new();
            record.insert(variable.to_string(), Value::Hyperedge(HyperEdgeId(he.id.0)));
            record
        })
        .collect()
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

    #[test]
    fn test_hyperedge_scan_empty() {
        let dir = tempdir().expect("tempdir");
        let engine = test_engine(dir.path());
        let records = execute_hyperedge_scan("he", &engine);
        assert!(records.is_empty());
    }

    #[test]
    fn test_hyperedge_scan_multiple() {
        let dir = tempdir().expect("tempdir");
        let mut engine = test_engine(dir.path());

        engine.create_hyperedge(1, vec![], vec![], vec![]);
        engine.create_hyperedge(2, vec![], vec![], vec![]);
        engine.create_hyperedge(3, vec![], vec![], vec![]);

        let records = execute_hyperedge_scan("he", &engine);
        assert_eq!(records.len(), 3);

        for record in &records {
            assert!(record.contains_key("he"));
            assert!(matches!(record.get("he"), Some(Value::Hyperedge(_))));
        }
    }

    #[test]
    fn test_hyperedge_scan_variable_binding() {
        let dir = tempdir().expect("tempdir");
        let mut engine = test_engine(dir.path());

        let he_id = engine.create_hyperedge(1, vec![], vec![], vec![]);
        let records = execute_hyperedge_scan("h", &engine);

        assert_eq!(records.len(), 1);
        assert_eq!(records[0].get("h"), Some(&Value::Hyperedge(he_id)));
    }
}
