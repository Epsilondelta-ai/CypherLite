// OptionalExpandOp: edge traversal with left join semantics (OPTIONAL MATCH)
//
// For each source record, try to expand (find matching edges/nodes).
// If expansion finds N results: emit N records (same as regular expand).
// If expansion finds 0 results: emit 1 record with NULL for rel_var and target_var.

use crate::executor::{Record, Value};
use crate::parser::ast::RelDirection;
use cypherlite_core::NodeId;
use cypherlite_storage::StorageEngine;

/// Execute an optional expand (left join). For each source record, find matching
/// edges. If matches exist, emit expanded records. If no match, emit one record
/// with NULL-padded new variables.
pub fn execute_optional_expand(
    source_records: Vec<Record>,
    src_var: &str,
    rel_var: Option<&str>,
    target_var: &str,
    rel_type_id: Option<u32>,
    direction: &RelDirection,
    engine: &StorageEngine,
) -> Vec<Record> {
    let mut results = Vec::new();

    for record in source_records {
        let src_node_id = match record.get(src_var) {
            Some(Value::Node(nid)) => *nid,
            _ => {
                // Source variable is not a node (or missing) -- emit NULL-padded record.
                let mut null_record = record;
                if let Some(rv) = rel_var {
                    null_record.insert(rv.to_string(), Value::Null);
                }
                null_record.insert(target_var.to_string(), Value::Null);
                results.push(null_record);
                continue;
            }
        };

        let edges = engine.get_edges_for_node(src_node_id);
        let mut matched = false;

        for edge in edges {
            // Filter by relationship type if specified
            if let Some(tid) = rel_type_id {
                if edge.rel_type_id != tid {
                    continue;
                }
            }

            // Direction filtering and determine target node
            let target_node_id: Option<NodeId> = match direction {
                RelDirection::Outgoing => {
                    if edge.start_node == src_node_id {
                        Some(edge.end_node)
                    } else {
                        None
                    }
                }
                RelDirection::Incoming => {
                    if edge.end_node == src_node_id {
                        Some(edge.start_node)
                    } else {
                        None
                    }
                }
                RelDirection::Undirected => {
                    if edge.start_node == src_node_id {
                        Some(edge.end_node)
                    } else if edge.end_node == src_node_id {
                        Some(edge.start_node)
                    } else {
                        None
                    }
                }
            };

            if let Some(target_id) = target_node_id {
                matched = true;
                let mut new_record = record.clone();
                if let Some(rv) = rel_var {
                    new_record.insert(rv.to_string(), Value::Edge(edge.edge_id));
                }
                new_record.insert(target_var.to_string(), Value::Node(target_id));
                results.push(new_record);
            }
        }

        // Left join semantics: if no match found, emit one NULL-padded record.
        if !matched {
            let mut null_record = record;
            if let Some(rv) = rel_var {
                null_record.insert(rv.to_string(), Value::Null);
            }
            null_record.insert(target_var.to_string(), Value::Null);
            results.push(null_record);
        }
    }

    results
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::executor::Record;
    use cypherlite_core::{DatabaseConfig, LabelRegistry, SyncMode};
    use cypherlite_storage::StorageEngine;
    use tempfile::tempdir;

    fn test_engine(dir: &std::path::Path) -> StorageEngine {
        let config = DatabaseConfig {
            path: dir.join("test.cyl"),
            wal_sync_mode: SyncMode::Normal,
            ..Default::default()
        };
        StorageEngine::open(config).expect("open")
    }

    // TASK-076: OptionalExpand with matching edges (same as regular expand)
    #[test]
    fn test_optional_expand_with_matches() {
        let dir = tempdir().expect("tempdir");
        let mut engine = test_engine(dir.path());

        let knows_type = engine.get_or_create_rel_type("KNOWS");
        let n1 = engine.create_node(vec![], vec![]);
        let n2 = engine.create_node(vec![], vec![]);
        let n3 = engine.create_node(vec![], vec![]);

        engine
            .create_edge(n1, n2, knows_type, vec![])
            .expect("edge");
        engine
            .create_edge(n1, n3, knows_type, vec![])
            .expect("edge");

        let mut source = Record::new();
        source.insert("a".to_string(), Value::Node(n1));

        let results = execute_optional_expand(
            vec![source],
            "a",
            Some("r"),
            "b",
            Some(knows_type),
            &RelDirection::Outgoing,
            &engine,
        );

        // Should find 2 matches (same as regular expand)
        assert_eq!(results.len(), 2);
        for r in &results {
            assert!(r.contains_key("a"));
            assert!(r.contains_key("r"));
            assert!(r.contains_key("b"));
            assert_ne!(r.get("b"), Some(&Value::Null));
        }
    }

    // TASK-076: OptionalExpand with NO matching edges (left join: NULL padding)
    #[test]
    fn test_optional_expand_no_matches_produces_null() {
        let dir = tempdir().expect("tempdir");
        let mut engine = test_engine(dir.path());

        let knows_type = engine.get_or_create_rel_type("KNOWS");
        // Node with no outgoing KNOWS edges
        let n1 = engine.create_node(vec![], vec![]);

        let mut source = Record::new();
        source.insert("a".to_string(), Value::Node(n1));

        let results = execute_optional_expand(
            vec![source],
            "a",
            Some("r"),
            "b",
            Some(knows_type),
            &RelDirection::Outgoing,
            &engine,
        );

        // Should produce exactly 1 record with NULL for 'b' and 'r'
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].get("a"), Some(&Value::Node(n1)));
        assert_eq!(results[0].get("b"), Some(&Value::Null));
        assert_eq!(results[0].get("r"), Some(&Value::Null));
    }

    // TASK-076: OptionalExpand with mixed matches (some source nodes match, some don't)
    #[test]
    fn test_optional_expand_mixed_matches() {
        let dir = tempdir().expect("tempdir");
        let mut engine = test_engine(dir.path());

        let knows_type = engine.get_or_create_rel_type("KNOWS");
        let n1 = engine.create_node(vec![], vec![]);
        let n2 = engine.create_node(vec![], vec![]);
        let n3 = engine.create_node(vec![], vec![]);

        // n1 knows n3, but n2 knows nobody
        engine
            .create_edge(n1, n3, knows_type, vec![])
            .expect("edge");

        let mut source1 = Record::new();
        source1.insert("a".to_string(), Value::Node(n1));
        let mut source2 = Record::new();
        source2.insert("a".to_string(), Value::Node(n2));

        let results = execute_optional_expand(
            vec![source1, source2],
            "a",
            None,
            "b",
            Some(knows_type),
            &RelDirection::Outgoing,
            &engine,
        );

        // n1 -> 1 match, n2 -> 1 null-padded = 2 records total
        assert_eq!(results.len(), 2);

        // First record: n1 matched n3
        assert_eq!(results[0].get("a"), Some(&Value::Node(n1)));
        assert_eq!(results[0].get("b"), Some(&Value::Node(n3)));

        // Second record: n2 has no match, b is Null
        assert_eq!(results[1].get("a"), Some(&Value::Node(n2)));
        assert_eq!(results[1].get("b"), Some(&Value::Null));
    }

    // TASK-076: OptionalExpand without rel_var
    #[test]
    fn test_optional_expand_no_rel_var() {
        let dir = tempdir().expect("tempdir");
        let mut engine = test_engine(dir.path());

        let knows_type = engine.get_or_create_rel_type("KNOWS");
        let n1 = engine.create_node(vec![], vec![]);

        let mut source = Record::new();
        source.insert("a".to_string(), Value::Node(n1));

        let results = execute_optional_expand(
            vec![source],
            "a",
            None, // no rel_var
            "b",
            Some(knows_type),
            &RelDirection::Outgoing,
            &engine,
        );

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].get("b"), Some(&Value::Null));
        // No 'r' key should be present
        assert!(!results[0].contains_key("r"));
    }
}
