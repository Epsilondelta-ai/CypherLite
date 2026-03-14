// ExpandOp: edge traversal (linked-list walk, O(degree))

use crate::executor::operators::temporal_filter::{is_edge_temporally_valid, TemporalFilter};
use crate::executor::{Record, Value};
use crate::parser::ast::RelDirection;
#[cfg(feature = "subgraph")]
use cypherlite_core::LabelRegistry;
use cypherlite_core::NodeId;
use cypherlite_storage::StorageEngine;

/// Expand from source records along edges.
/// For each source record, find edges matching direction and type,
/// and produce new records with rel_var and target_var bindings.
///
/// When `temporal_filter` is Some, edges that fail temporal validity
/// are skipped (DD-T2).
#[allow(clippy::too_many_arguments)]
pub fn execute_expand(
    source_records: Vec<Record>,
    src_var: &str,
    rel_var: Option<&str>,
    target_var: &str,
    rel_type_id: Option<u32>,
    direction: &RelDirection,
    engine: &StorageEngine,
    temporal_filter: Option<&TemporalFilter>,
) -> Vec<Record> {
    let mut results = Vec::new();

    for record in source_records {
        // Check if source is a Hyperedge -- virtual :INVOLVES expansion.
        #[cfg(feature = "hypergraph")]
        {
            if let Some(Value::Hyperedge(he_id)) = record.get(src_var) {
                if let Some(he_rec) = engine.get_hyperedge(*he_id) {
                    // Filter by rel_type if specified
                    let type_matches = rel_type_id.is_none_or(|tid| he_rec.rel_type_id == tid);
                    if type_matches {
                        // Collect participant target values first for move-last optimisation.
                        let participants: Vec<Value> = he_rec
                            .sources
                            .iter()
                            .chain(he_rec.targets.iter())
                            .map(|entity| match entity {
                                cypherlite_core::GraphEntity::Node(nid) => Value::Node(*nid),
                                #[cfg(feature = "subgraph")]
                                cypherlite_core::GraphEntity::Subgraph(sid) => {
                                    Value::Subgraph(*sid)
                                }
                                cypherlite_core::GraphEntity::HyperEdge(hid) => {
                                    Value::Hyperedge(*hid)
                                }
                                #[cfg(feature = "hypergraph")]
                                cypherlite_core::GraphEntity::TemporalRef(nid, ts) => {
                                    // NN-003: Lazy resolution -- store as TemporalNode,
                                    // resolved when properties are accessed.
                                    Value::TemporalNode(*nid, *ts)
                                }
                            })
                            .collect();

                        if !participants.is_empty() {
                            // Clone for all-but-last, move for last (REQ-Q-003).
                            for target_value in &participants[..participants.len() - 1] {
                                let mut new_record = record.clone();
                                if let Some(rv) = rel_var {
                                    new_record.insert(rv.to_string(), Value::Null);
                                }
                                new_record.insert(target_var.to_string(), target_value.clone());
                                results.push(new_record);
                            }
                            // Last participant: move the record (no clone).
                            let last_value = participants.into_iter().last().unwrap();
                            let mut last_record = record;
                            if let Some(rv) = rel_var {
                                last_record.insert(rv.to_string(), Value::Null);
                            }
                            last_record.insert(target_var.to_string(), last_value);
                            results.push(last_record);
                        }
                    }
                }
                continue;
            }
        }

        // Check if source is a Subgraph and rel_type is "CONTAINS" -- virtual edge expansion.
        #[cfg(feature = "subgraph")]
        {
            if let Some(Value::Subgraph(sg_id)) = record.get(src_var) {
                // Check if the relationship type is "CONTAINS"
                let is_contains = rel_type_id
                    .is_some_and(|tid| engine.catalog().rel_type_name(tid) == Some("CONTAINS"));
                if is_contains {
                    // Virtual :CONTAINS expansion via MembershipIndex.
                    // Collect members first for move-last optimisation (REQ-Q-003).
                    let members: Vec<NodeId> = engine.list_members(*sg_id);
                    if !members.is_empty() {
                        // Clone for all-but-last, move for last.
                        for &node_id in &members[..members.len() - 1] {
                            let mut new_record = record.clone();
                            if let Some(rv) = rel_var {
                                new_record.insert(rv.to_string(), Value::Null);
                            }
                            new_record.insert(target_var.to_string(), Value::Node(node_id));
                            results.push(new_record);
                        }
                        let last_node = *members.last().unwrap();
                        let mut last_record = record;
                        if let Some(rv) = rel_var {
                            last_record.insert(rv.to_string(), Value::Null);
                        }
                        last_record.insert(target_var.to_string(), Value::Node(last_node));
                        results.push(last_record);
                    }
                    continue;
                }
            }
        }

        let src_node_id = match record.get(src_var) {
            Some(Value::Node(nid)) => *nid,
            _ => continue,
        };

        let edges = engine.get_edges_for_node(src_node_id);

        // Collect matching (edge_id, target_node_id) pairs for move-last optimisation
        // (REQ-Q-003). This avoids cloning the source record for the last match.
        let mut matched: Vec<(cypherlite_core::EdgeId, NodeId)> = Vec::new();
        for edge in edges {
            // Filter by relationship type if specified
            if let Some(tid) = rel_type_id {
                if edge.rel_type_id != tid {
                    continue;
                }
            }

            // Temporal filter: skip edges that are not temporally valid
            if let Some(tf) = temporal_filter {
                if !is_edge_temporally_valid(edge.edge_id, tf, engine) {
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
                matched.push((edge.edge_id, target_id));
            }
        }

        if matched.is_empty() {
            continue;
        }

        // Emit: clone for all-but-last, move for last.
        for &(edge_id, target_id) in &matched[..matched.len() - 1] {
            let mut new_record = record.clone();
            if let Some(rv) = rel_var {
                new_record.insert(rv.to_string(), Value::Edge(edge_id));
            }
            new_record.insert(target_var.to_string(), Value::Node(target_id));
            results.push(new_record);
        }
        let (last_edge_id, last_target_id) = *matched.last().unwrap();
        let mut last_record = record; // move ownership, no clone
        if let Some(rv) = rel_var {
            last_record.insert(rv.to_string(), Value::Edge(last_edge_id));
        }
        last_record.insert(target_var.to_string(), Value::Node(last_target_id));
        results.push(last_record);
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
            None,
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
            None,
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
            None,
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
            None,
        );

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].get("b"), Some(&Value::Node(n1)));
    }

    // ── M3-1: Record sharing / clone-last optimization tests ───────────

    /// Star graph: one source node with 20 outgoing edges.
    /// Verifies all 20 results contain correct bindings.
    #[test]
    fn test_expand_large_fanout() {
        let dir = tempdir().expect("tempdir");
        let mut engine = test_engine(dir.path());

        let knows_type = engine.get_or_create_rel_type("KNOWS");
        let center = engine.create_node(vec![], vec![]);
        let mut targets = Vec::new();
        for _ in 0..20 {
            let t = engine.create_node(vec![], vec![]);
            engine
                .create_edge(center, t, knows_type, vec![])
                .expect("edge");
            targets.push(t);
        }

        let mut source = Record::new();
        source.insert("a".to_string(), Value::Node(center));
        // Add extra binding to verify it survives clone/move
        source.insert("extra".to_string(), Value::Int64(42));

        let results = execute_expand(
            vec![source],
            "a",
            Some("r"),
            "b",
            Some(knows_type),
            &RelDirection::Outgoing,
            &engine,
            None,
        );

        assert_eq!(results.len(), 20);
        let mut found_targets: Vec<NodeId> = results
            .iter()
            .filter_map(|r| match r.get("b") {
                Some(Value::Node(nid)) => Some(*nid),
                _ => None,
            })
            .collect();
        found_targets.sort();
        targets.sort();
        assert_eq!(found_targets, targets);

        // Verify extra binding preserved in every result
        for r in &results {
            assert_eq!(r.get("extra"), Some(&Value::Int64(42)));
            assert!(r.contains_key("r"));
            assert!(r.contains_key("a"));
        }
    }

    /// Single edge: the source record should be moved (not cloned).
    /// Validates the single-match path produces a correct record.
    #[test]
    fn test_expand_single_edge_move() {
        let dir = tempdir().expect("tempdir");
        let mut engine = test_engine(dir.path());

        let follows_type = engine.get_or_create_rel_type("FOLLOWS");
        let n1 = engine.create_node(vec![], vec![]);
        let n2 = engine.create_node(vec![], vec![]);
        engine
            .create_edge(n1, n2, follows_type, vec![])
            .expect("edge");

        let mut source = Record::new();
        source.insert("x".to_string(), Value::Node(n1));
        source.insert("ctx".to_string(), Value::Int64(99));

        let results = execute_expand(
            vec![source],
            "x",
            Some("rel"),
            "y",
            Some(follows_type),
            &RelDirection::Outgoing,
            &engine,
            None,
        );

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].get("y"), Some(&Value::Node(n2)));
        assert_eq!(results[0].get("x"), Some(&Value::Node(n1)));
        assert_eq!(results[0].get("ctx"), Some(&Value::Int64(99)));
        assert!(results[0].contains_key("rel"));
    }

    /// Multiple source records, each with different edge counts.
    /// Verifies the optimization works correctly per-source-record.
    #[test]
    fn test_expand_multiple_sources_varied_fanout() {
        let dir = tempdir().expect("tempdir");
        let mut engine = test_engine(dir.path());

        let knows_type = engine.get_or_create_rel_type("KNOWS");

        // Source 1: 3 edges
        let s1 = engine.create_node(vec![], vec![]);
        let t1a = engine.create_node(vec![], vec![]);
        let t1b = engine.create_node(vec![], vec![]);
        let t1c = engine.create_node(vec![], vec![]);
        engine.create_edge(s1, t1a, knows_type, vec![]).unwrap();
        engine.create_edge(s1, t1b, knows_type, vec![]).unwrap();
        engine.create_edge(s1, t1c, knows_type, vec![]).unwrap();

        // Source 2: 0 edges (no matches)
        let s2 = engine.create_node(vec![], vec![]);

        // Source 3: 1 edge
        let s3 = engine.create_node(vec![], vec![]);
        let t3a = engine.create_node(vec![], vec![]);
        engine.create_edge(s3, t3a, knows_type, vec![]).unwrap();

        let mut rec1 = Record::new();
        rec1.insert("n".to_string(), Value::Node(s1));
        rec1.insert("tag".to_string(), Value::Int64(1));

        let mut rec2 = Record::new();
        rec2.insert("n".to_string(), Value::Node(s2));
        rec2.insert("tag".to_string(), Value::Int64(2));

        let mut rec3 = Record::new();
        rec3.insert("n".to_string(), Value::Node(s3));
        rec3.insert("tag".to_string(), Value::Int64(3));

        let results = execute_expand(
            vec![rec1, rec2, rec3],
            "n",
            Some("r"),
            "m",
            Some(knows_type),
            &RelDirection::Outgoing,
            &engine,
            None,
        );

        // 3 from s1 + 0 from s2 + 1 from s3 = 4 results
        assert_eq!(results.len(), 4);

        // First 3 results from s1 have tag=1
        let s1_results: Vec<_> = results
            .iter()
            .filter(|r| r.get("tag") == Some(&Value::Int64(1)))
            .collect();
        assert_eq!(s1_results.len(), 3);

        // No results with tag=2 (s2 had no edges)
        let s2_results: Vec<_> = results
            .iter()
            .filter(|r| r.get("tag") == Some(&Value::Int64(2)))
            .collect();
        assert_eq!(s2_results.len(), 0);

        // 1 result from s3 with tag=3
        let s3_results: Vec<_> = results
            .iter()
            .filter(|r| r.get("tag") == Some(&Value::Int64(3)))
            .collect();
        assert_eq!(s3_results.len(), 1);
        assert_eq!(s3_results[0].get("m"), Some(&Value::Node(t3a)));
    }

    /// Zero matching edges: no results should be produced.
    #[test]
    fn test_expand_zero_matching_edges_no_results() {
        let dir = tempdir().expect("tempdir");
        let mut engine = test_engine(dir.path());

        let knows_type = engine.get_or_create_rel_type("KNOWS");
        let likes_type = engine.get_or_create_rel_type("LIKES");

        let n1 = engine.create_node(vec![], vec![]);
        let n2 = engine.create_node(vec![], vec![]);
        // Edge is LIKES, but we query for KNOWS
        engine.create_edge(n1, n2, likes_type, vec![]).unwrap();

        let mut source = Record::new();
        source.insert("a".to_string(), Value::Node(n1));
        source.insert("data".to_string(), Value::Int64(7));

        let results = execute_expand(
            vec![source],
            "a",
            Some("r"),
            "b",
            Some(knows_type),
            &RelDirection::Outgoing,
            &engine,
            None,
        );

        assert!(results.is_empty());
    }

    // ── Hypergraph :INVOLVES virtual expansion tests ───────────────────
    #[cfg(feature = "hypergraph")]
    mod involves_tests {
        use super::*;

        #[test]
        fn test_involves_expands_to_source_and_target_nodes() {
            let dir = tempdir().expect("tempdir");
            let mut engine = test_engine(dir.path());

            let involves_type = engine.get_or_create_rel_type("INVOLVES");
            let n1 = engine.create_node(vec![], vec![]);
            let n2 = engine.create_node(vec![], vec![]);
            let n3 = engine.create_node(vec![], vec![]);

            use cypherlite_core::GraphEntity;
            engine.create_hyperedge(
                involves_type,
                vec![GraphEntity::Node(n1)],
                vec![GraphEntity::Node(n2), GraphEntity::Node(n3)],
                vec![],
            );

            // Source record: he bound to the hyperedge
            let he_id = cypherlite_core::HyperEdgeId(1);
            let mut source = Record::new();
            source.insert("he".to_string(), Value::Hyperedge(he_id));

            let results = execute_expand(
                vec![source],
                "he",
                Some("r"),
                "n",
                Some(involves_type),
                &RelDirection::Outgoing,
                &engine,
                None,
            );

            // Should return all sources + targets = 3 nodes
            assert_eq!(results.len(), 3);
            for r in &results {
                assert!(r.contains_key("he"));
                assert!(r.contains_key("n"));
                // Virtual edge: rel_var is Null
                assert_eq!(r.get("r"), Some(&Value::Null));
                assert!(matches!(r.get("n"), Some(Value::Node(_))));
            }
        }

        #[test]
        fn test_involves_no_matching_type() {
            let dir = tempdir().expect("tempdir");
            let mut engine = test_engine(dir.path());

            let involves_type = engine.get_or_create_rel_type("INVOLVES");
            let other_type = engine.get_or_create_rel_type("OTHER");
            let n1 = engine.create_node(vec![], vec![]);

            use cypherlite_core::GraphEntity;
            engine.create_hyperedge(involves_type, vec![GraphEntity::Node(n1)], vec![], vec![]);

            let he_id = cypherlite_core::HyperEdgeId(1);
            let mut source = Record::new();
            source.insert("he".to_string(), Value::Hyperedge(he_id));

            // Ask for OTHER type, but hyperedge is INVOLVES -> mismatch
            let results = execute_expand(
                vec![source],
                "he",
                None,
                "n",
                Some(other_type),
                &RelDirection::Outgoing,
                &engine,
                None,
            );

            assert!(results.is_empty());
        }

        #[test]
        fn test_involves_empty_hyperedge() {
            let dir = tempdir().expect("tempdir");
            let mut engine = test_engine(dir.path());

            let involves_type = engine.get_or_create_rel_type("INVOLVES");

            engine.create_hyperedge(involves_type, vec![], vec![], vec![]);

            let he_id = cypherlite_core::HyperEdgeId(1);
            let mut source = Record::new();
            source.insert("he".to_string(), Value::Hyperedge(he_id));

            let results = execute_expand(
                vec![source],
                "he",
                None,
                "n",
                Some(involves_type),
                &RelDirection::Outgoing,
                &engine,
                None,
            );

            assert!(results.is_empty());
        }

        #[test]
        fn test_involves_no_rel_type_filter_matches_all() {
            let dir = tempdir().expect("tempdir");
            let mut engine = test_engine(dir.path());

            let involves_type = engine.get_or_create_rel_type("INVOLVES");
            let n1 = engine.create_node(vec![], vec![]);

            use cypherlite_core::GraphEntity;
            engine.create_hyperedge(involves_type, vec![GraphEntity::Node(n1)], vec![], vec![]);

            let he_id = cypherlite_core::HyperEdgeId(1);
            let mut source = Record::new();
            source.insert("he".to_string(), Value::Hyperedge(he_id));

            // No rel_type filter (None) -> should still expand all participants
            let results = execute_expand(
                vec![source],
                "he",
                None,
                "n",
                None,
                &RelDirection::Outgoing,
                &engine,
                None,
            );

            assert_eq!(results.len(), 1);
            assert_eq!(results[0].get("n"), Some(&Value::Node(n1)));
        }
    }
}
