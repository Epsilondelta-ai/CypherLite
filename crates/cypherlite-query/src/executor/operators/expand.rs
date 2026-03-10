// ExpandOp: edge traversal (linked-list walk, O(degree))

use crate::executor::{Record, Value};
use crate::parser::ast::RelDirection;
use cypherlite_core::NodeId;
use cypherlite_storage::StorageEngine;

/// Expand from source records along edges.
/// For each source record, find edges matching direction and type,
/// and produce new records with rel_var and target_var bindings.
pub fn execute_expand(
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
            _ => continue,
        };

        let edges = engine.get_edges_for_node(src_node_id);

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
                let mut new_record = record.clone();
                if let Some(rv) = rel_var {
                    new_record.insert(rv.to_string(), Value::Edge(edge.edge_id));
                }
                new_record.insert(target_var.to_string(), Value::Node(target_id));
                results.push(new_record);
            }
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

    // EXEC-T002: ExpandOp directed traversal
    #[test]
    fn test_expand_outgoing() {
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

        let results = execute_expand(
            vec![source],
            "a",
            Some("r"),
            "b",
            Some(knows_type),
            &RelDirection::Outgoing,
            &engine,
        );

        assert_eq!(results.len(), 2);
        for r in &results {
            assert!(r.contains_key("a"));
            assert!(r.contains_key("r"));
            assert!(r.contains_key("b"));
        }
    }

    #[test]
    fn test_expand_incoming() {
        let dir = tempdir().expect("tempdir");
        let mut engine = test_engine(dir.path());

        let knows_type = engine.get_or_create_rel_type("KNOWS");
        let n1 = engine.create_node(vec![], vec![]);
        let n2 = engine.create_node(vec![], vec![]);

        engine
            .create_edge(n1, n2, knows_type, vec![])
            .expect("edge");

        let mut source = Record::new();
        source.insert("b".to_string(), Value::Node(n2));

        let results = execute_expand(
            vec![source],
            "b",
            None,
            "a",
            Some(knows_type),
            &RelDirection::Incoming,
            &engine,
        );

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].get("a"), Some(&Value::Node(n1)));
    }

    #[test]
    fn test_expand_no_matching_type() {
        let dir = tempdir().expect("tempdir");
        let mut engine = test_engine(dir.path());

        let knows_type = engine.get_or_create_rel_type("KNOWS");
        let likes_type = engine.get_or_create_rel_type("LIKES");
        let n1 = engine.create_node(vec![], vec![]);
        let n2 = engine.create_node(vec![], vec![]);

        engine
            .create_edge(n1, n2, knows_type, vec![])
            .expect("edge");

        let mut source = Record::new();
        source.insert("a".to_string(), Value::Node(n1));

        let results = execute_expand(
            vec![source],
            "a",
            None,
            "b",
            Some(likes_type),
            &RelDirection::Outgoing,
            &engine,
        );

        assert!(results.is_empty());
    }

    #[test]
    fn test_expand_undirected() {
        let dir = tempdir().expect("tempdir");
        let mut engine = test_engine(dir.path());

        let knows_type = engine.get_or_create_rel_type("KNOWS");
        let n1 = engine.create_node(vec![], vec![]);
        let n2 = engine.create_node(vec![], vec![]);

        engine
            .create_edge(n1, n2, knows_type, vec![])
            .expect("edge");

        // Starting from n2, undirected should find n1
        let mut source = Record::new();
        source.insert("a".to_string(), Value::Node(n2));

        let results = execute_expand(
            vec![source],
            "a",
            None,
            "b",
            None,
            &RelDirection::Undirected,
            &engine,
        );

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].get("b"), Some(&Value::Node(n1)));
    }
}
