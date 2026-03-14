// SubgraphScan operator: scans all subgraph entities from SubgraphStore.
// Each subgraph produces a Record with the variable bound to Value::Subgraph(id).

use crate::executor::{Record, Value};
use cypherlite_core::SubgraphId;
use cypherlite_storage::StorageEngine;

/// Scan all subgraphs from the engine.
/// Each subgraph produces a Record with the variable bound to Value::Subgraph(subgraph_id).
pub fn execute_subgraph_scan(variable: &str, engine: &StorageEngine) -> Vec<Record> {
    engine
        .scan_subgraphs()
        .into_iter()
        .map(|sg| {
            let mut record = Record::new();
            record.insert(
                variable.to_string(),
                Value::Subgraph(SubgraphId(sg.subgraph_id.0)),
            );
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
    fn test_subgraph_scan_empty() {
        let dir = tempdir().expect("tempdir");
        let engine = test_engine(dir.path());
        let records = execute_subgraph_scan("sg", &engine);
        assert!(records.is_empty());
    }

    #[test]
    fn test_subgraph_scan_multiple() {
        let dir = tempdir().expect("tempdir");
        let mut engine = test_engine(dir.path());

        engine.create_subgraph(vec![], None);
        engine.create_subgraph(vec![], Some(1000));
        engine.create_subgraph(vec![], None);

        let records = execute_subgraph_scan("sg", &engine);
        assert_eq!(records.len(), 3);

        for record in &records {
            assert!(record.contains_key("sg"));
            assert!(matches!(record.get("sg"), Some(Value::Subgraph(_))));
        }
    }
}
