// VarLengthExpandOp: variable-length path traversal using DFS with backtracking

use crate::executor::{Record, Value};
use crate::parser::ast::RelDirection;
use cypherlite_core::NodeId;
use cypherlite_storage::StorageEngine;
use std::collections::HashSet;

/// Execute variable-length expansion from source records.
/// Uses DFS with backtracking to enumerate all paths within [min_hops, max_hops].
/// Cycle detection: a node cannot appear twice in the same path.
pub fn execute_var_length_expand(
    source_records: Vec<Record>,
    src_var: &str,
    rel_var: Option<&str>,
    target_var: &str,
    rel_type_id: Option<u32>,
    direction: &RelDirection,
    min_hops: u32,
    max_hops: u32,
    engine: &StorageEngine,
) -> Vec<Record> {
    let mut results = Vec::new();

    for record in &source_records {
        let src_node_id = match record.get(src_var) {
            Some(Value::Node(nid)) => *nid,
            _ => continue,
        };

        // Special case: min_hops == 0 means source itself matches
        if min_hops == 0 {
            let mut new_record = record.clone();
            if let Some(rv) = rel_var {
                new_record.insert(rv.to_string(), Value::Null);
            }
            new_record.insert(target_var.to_string(), Value::Node(src_node_id));
            results.push(new_record);
        }

        // DFS with backtracking
        let mut visited = HashSet::new();
        visited.insert(src_node_id);

        dfs(
            src_node_id,
            1, // starting at depth 1 (first hop)
            min_hops,
            max_hops,
            &mut visited,
            rel_type_id,
            direction,
            record,
            rel_var,
            target_var,
            engine,
            &mut results,
        );
    }

    results
}

fn dfs(
    current_node: NodeId,
    depth: u32,
    min_hops: u32,
    max_hops: u32,
    visited: &mut HashSet<NodeId>,
    rel_type_id: Option<u32>,
    direction: &RelDirection,
    base_record: &Record,
    rel_var: Option<&str>,
    target_var: &str,
    engine: &StorageEngine,
    results: &mut Vec<Record>,
) {
    if depth > max_hops {
        return;
    }

    let edges = engine.get_edges_for_node(current_node);

    for edge in &edges {
        // Filter by relationship type if specified
        if let Some(tid) = rel_type_id {
            if edge.rel_type_id != tid {
                continue;
            }
        }

        // Direction filtering and determine target node
        let target_node_id: Option<NodeId> = match direction {
            RelDirection::Outgoing => {
                if edge.start_node == current_node {
                    Some(edge.end_node)
                } else {
                    None
                }
            }
            RelDirection::Incoming => {
                if edge.end_node == current_node {
                    Some(edge.start_node)
                } else {
                    None
                }
            }
            RelDirection::Undirected => {
                if edge.start_node == current_node {
                    Some(edge.end_node)
                } else if edge.end_node == current_node {
                    Some(edge.start_node)
                } else {
                    None
                }
            }
        };

        if let Some(target_id) = target_node_id {
            // Cycle detection: skip if already in current path
            if visited.contains(&target_id) {
                continue;
            }

            // Emit record if within bounds
            if depth >= min_hops {
                let mut new_record = base_record.clone();
                if let Some(rv) = rel_var {
                    new_record.insert(rv.to_string(), Value::Edge(edge.edge_id));
                }
                new_record.insert(target_var.to_string(), Value::Node(target_id));
                results.push(new_record);
            }

            // Continue DFS if we can go deeper
            if depth < max_hops {
                visited.insert(target_id);
                dfs(
                    target_id,
                    depth + 1,
                    min_hops,
                    max_hops,
                    visited,
                    rel_type_id,
                    direction,
                    base_record,
                    rel_var,
                    target_var,
                    engine,
                    results,
                );
                visited.remove(&target_id);
            }
        }
    }
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

    // Helper: create a linear chain: n0 -> n1 -> n2 -> ... -> n(count-1)
    fn create_linear_chain(
        engine: &mut StorageEngine,
        rel_type: u32,
        count: usize,
    ) -> Vec<NodeId> {
        let nodes: Vec<NodeId> = (0..count).map(|_| engine.create_node(vec![], vec![])).collect();
        for i in 0..count - 1 {
            engine
                .create_edge(nodes[i], nodes[i + 1], rel_type, vec![])
                .expect("edge");
        }
        nodes
    }

    // -- TASK-106: Basic VarLengthExpand tests --

    #[test]
    fn test_var_expand_1_hop() {
        let dir = tempdir().expect("tempdir");
        let mut engine = test_engine(dir.path());
        let knows = engine.get_or_create_rel_type("KNOWS");
        let nodes = create_linear_chain(&mut engine, knows, 3);

        let mut source = Record::new();
        source.insert("a".to_string(), Value::Node(nodes[0]));

        let results = execute_var_length_expand(
            vec![source],
            "a",
            Some("r"),
            "b",
            Some(knows),
            &RelDirection::Outgoing,
            1,
            1,
            &engine,
        );

        // Only 1-hop: n0 -> n1
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].get("b"), Some(&Value::Node(nodes[1])));
    }

    #[test]
    fn test_var_expand_1_to_2_hops() {
        let dir = tempdir().expect("tempdir");
        let mut engine = test_engine(dir.path());
        let knows = engine.get_or_create_rel_type("KNOWS");
        let nodes = create_linear_chain(&mut engine, knows, 4);

        let mut source = Record::new();
        source.insert("a".to_string(), Value::Node(nodes[0]));

        let results = execute_var_length_expand(
            vec![source],
            "a",
            None,
            "b",
            Some(knows),
            &RelDirection::Outgoing,
            1,
            2,
            &engine,
        );

        // 1-hop: n1, 2-hop: n2
        assert_eq!(results.len(), 2);
        let targets: Vec<&Value> = results.iter().filter_map(|r| r.get("b")).collect();
        assert!(targets.contains(&&Value::Node(nodes[1])));
        assert!(targets.contains(&&Value::Node(nodes[2])));
    }

    #[test]
    fn test_var_expand_1_to_3_hops_linear() {
        let dir = tempdir().expect("tempdir");
        let mut engine = test_engine(dir.path());
        let knows = engine.get_or_create_rel_type("KNOWS");
        let nodes = create_linear_chain(&mut engine, knows, 5);

        let mut source = Record::new();
        source.insert("a".to_string(), Value::Node(nodes[0]));

        let results = execute_var_length_expand(
            vec![source],
            "a",
            None,
            "b",
            Some(knows),
            &RelDirection::Outgoing,
            1,
            3,
            &engine,
        );

        // 1-hop: n1, 2-hop: n2, 3-hop: n3
        assert_eq!(results.len(), 3);
    }

    #[test]
    fn test_var_expand_no_type_filter() {
        let dir = tempdir().expect("tempdir");
        let mut engine = test_engine(dir.path());
        let knows = engine.get_or_create_rel_type("KNOWS");
        let likes = engine.get_or_create_rel_type("LIKES");
        let n0 = engine.create_node(vec![], vec![]);
        let n1 = engine.create_node(vec![], vec![]);
        let n2 = engine.create_node(vec![], vec![]);
        engine.create_edge(n0, n1, knows, vec![]).expect("edge");
        engine.create_edge(n1, n2, likes, vec![]).expect("edge");

        let mut source = Record::new();
        source.insert("a".to_string(), Value::Node(n0));

        // No type filter: should traverse both KNOWS and LIKES
        let results = execute_var_length_expand(
            vec![source],
            "a",
            None,
            "b",
            None, // no type filter
            &RelDirection::Outgoing,
            1,
            2,
            &engine,
        );

        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_var_expand_with_type_filter() {
        let dir = tempdir().expect("tempdir");
        let mut engine = test_engine(dir.path());
        let knows = engine.get_or_create_rel_type("KNOWS");
        let likes = engine.get_or_create_rel_type("LIKES");
        let n0 = engine.create_node(vec![], vec![]);
        let n1 = engine.create_node(vec![], vec![]);
        let n2 = engine.create_node(vec![], vec![]);
        engine.create_edge(n0, n1, knows, vec![]).expect("edge");
        engine.create_edge(n1, n2, likes, vec![]).expect("edge");

        let mut source = Record::new();
        source.insert("a".to_string(), Value::Node(n0));

        // Only KNOWS: should stop at n1
        let results = execute_var_length_expand(
            vec![source],
            "a",
            None,
            "b",
            Some(knows),
            &RelDirection::Outgoing,
            1,
            3,
            &engine,
        );

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].get("b"), Some(&Value::Node(n1)));
    }

    #[test]
    fn test_var_expand_incoming() {
        let dir = tempdir().expect("tempdir");
        let mut engine = test_engine(dir.path());
        let knows = engine.get_or_create_rel_type("KNOWS");
        let nodes = create_linear_chain(&mut engine, knows, 4);

        let mut source = Record::new();
        source.insert("a".to_string(), Value::Node(nodes[3]));

        let results = execute_var_length_expand(
            vec![source],
            "a",
            None,
            "b",
            Some(knows),
            &RelDirection::Incoming,
            1,
            2,
            &engine,
        );

        // Incoming from n3: 1-hop: n2, 2-hop: n1
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_var_expand_empty_source() {
        let dir = tempdir().expect("tempdir");
        let engine = test_engine(dir.path());

        let results = execute_var_length_expand(
            vec![],
            "a",
            None,
            "b",
            None,
            &RelDirection::Outgoing,
            1,
            3,
            &engine,
        );

        assert!(results.is_empty());
    }

    #[test]
    fn test_var_expand_no_edges() {
        let dir = tempdir().expect("tempdir");
        let mut engine = test_engine(dir.path());
        let n0 = engine.create_node(vec![], vec![]);

        let mut source = Record::new();
        source.insert("a".to_string(), Value::Node(n0));

        let results = execute_var_length_expand(
            vec![source],
            "a",
            None,
            "b",
            None,
            &RelDirection::Outgoing,
            1,
            3,
            &engine,
        );

        assert!(results.is_empty());
    }

    #[test]
    fn test_var_expand_rel_var_binding() {
        let dir = tempdir().expect("tempdir");
        let mut engine = test_engine(dir.path());
        let knows = engine.get_or_create_rel_type("KNOWS");
        let nodes = create_linear_chain(&mut engine, knows, 3);

        let mut source = Record::new();
        source.insert("a".to_string(), Value::Node(nodes[0]));

        let results = execute_var_length_expand(
            vec![source],
            "a",
            Some("r"),
            "b",
            Some(knows),
            &RelDirection::Outgoing,
            1,
            2,
            &engine,
        );

        // Both results should have r bound to an edge
        for r in &results {
            assert!(r.contains_key("r"));
            assert!(matches!(r.get("r"), Some(Value::Edge(_))));
        }
    }

    #[test]
    fn test_var_expand_branching() {
        let dir = tempdir().expect("tempdir");
        let mut engine = test_engine(dir.path());
        let knows = engine.get_or_create_rel_type("KNOWS");
        // n0 -> n1, n0 -> n2, n1 -> n3, n2 -> n3
        let n0 = engine.create_node(vec![], vec![]);
        let n1 = engine.create_node(vec![], vec![]);
        let n2 = engine.create_node(vec![], vec![]);
        let n3 = engine.create_node(vec![], vec![]);
        engine.create_edge(n0, n1, knows, vec![]).expect("edge");
        engine.create_edge(n0, n2, knows, vec![]).expect("edge");
        engine.create_edge(n1, n3, knows, vec![]).expect("edge");
        engine.create_edge(n2, n3, knows, vec![]).expect("edge");

        let mut source = Record::new();
        source.insert("a".to_string(), Value::Node(n0));

        let results = execute_var_length_expand(
            vec![source],
            "a",
            None,
            "b",
            Some(knows),
            &RelDirection::Outgoing,
            1,
            2,
            &engine,
        );

        // 1-hop: n1, n2. 2-hop from n1: n3, 2-hop from n2: n3
        // Total: n1, n2, n3, n3 = 4 records
        assert_eq!(results.len(), 4);
    }

    // -- TASK-107: Cycle detection tests --

    #[test]
    fn test_var_expand_cycle_detection_simple() {
        let dir = tempdir().expect("tempdir");
        let mut engine = test_engine(dir.path());
        let knows = engine.get_or_create_rel_type("KNOWS");
        // Cycle: n0 -> n1 -> n2 -> n0
        let n0 = engine.create_node(vec![], vec![]);
        let n1 = engine.create_node(vec![], vec![]);
        let n2 = engine.create_node(vec![], vec![]);
        engine.create_edge(n0, n1, knows, vec![]).expect("edge");
        engine.create_edge(n1, n2, knows, vec![]).expect("edge");
        engine.create_edge(n2, n0, knows, vec![]).expect("edge");

        let mut source = Record::new();
        source.insert("a".to_string(), Value::Node(n0));

        let results = execute_var_length_expand(
            vec![source],
            "a",
            None,
            "b",
            Some(knows),
            &RelDirection::Outgoing,
            1,
            10,
            &engine,
        );

        // With cycle detection: n0->n1 (1), n0->n1->n2 (2)
        // n2->n0 is blocked because n0 is already in the path
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_var_expand_cycle_detection_self_loop() {
        let dir = tempdir().expect("tempdir");
        let mut engine = test_engine(dir.path());
        let knows = engine.get_or_create_rel_type("KNOWS");
        let n0 = engine.create_node(vec![], vec![]);
        let n1 = engine.create_node(vec![], vec![]);
        engine.create_edge(n0, n1, knows, vec![]).expect("edge");
        engine.create_edge(n1, n1, knows, vec![]).expect("edge"); // self loop

        let mut source = Record::new();
        source.insert("a".to_string(), Value::Node(n0));

        let results = execute_var_length_expand(
            vec![source],
            "a",
            None,
            "b",
            Some(knows),
            &RelDirection::Outgoing,
            1,
            5,
            &engine,
        );

        // n0->n1 (1 hop). n1->n1 is blocked (self loop, already visited)
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_var_expand_cycle_different_paths_allowed() {
        let dir = tempdir().expect("tempdir");
        let mut engine = test_engine(dir.path());
        let knows = engine.get_or_create_rel_type("KNOWS");
        // Diamond: n0 -> n1, n0 -> n2, n1 -> n3, n2 -> n3
        let n0 = engine.create_node(vec![], vec![]);
        let n1 = engine.create_node(vec![], vec![]);
        let n2 = engine.create_node(vec![], vec![]);
        let n3 = engine.create_node(vec![], vec![]);
        engine.create_edge(n0, n1, knows, vec![]).expect("edge");
        engine.create_edge(n0, n2, knows, vec![]).expect("edge");
        engine.create_edge(n1, n3, knows, vec![]).expect("edge");
        engine.create_edge(n2, n3, knows, vec![]).expect("edge");

        let mut source = Record::new();
        source.insert("a".to_string(), Value::Node(n0));

        let results = execute_var_length_expand(
            vec![source],
            "a",
            None,
            "b",
            Some(knows),
            &RelDirection::Outgoing,
            2,
            2,
            &engine,
        );

        // 2-hop paths: n0->n1->n3 and n0->n2->n3
        // n3 appears via different paths - both should be emitted
        assert_eq!(results.len(), 2);
        for r in &results {
            assert_eq!(r.get("b"), Some(&Value::Node(n3)));
        }
    }

    #[test]
    fn test_var_expand_undirected_cycle() {
        let dir = tempdir().expect("tempdir");
        let mut engine = test_engine(dir.path());
        let knows = engine.get_or_create_rel_type("KNOWS");
        let n0 = engine.create_node(vec![], vec![]);
        let n1 = engine.create_node(vec![], vec![]);
        engine.create_edge(n0, n1, knows, vec![]).expect("edge");

        let mut source = Record::new();
        source.insert("a".to_string(), Value::Node(n0));

        // Undirected: n0-n1 exists. Without cycle detection, undirected would
        // bounce back and forth. With it, n0->n1 only.
        let results = execute_var_length_expand(
            vec![source],
            "a",
            None,
            "b",
            Some(knows),
            &RelDirection::Undirected,
            1,
            5,
            &engine,
        );

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].get("b"), Some(&Value::Node(n1)));
    }

    // -- TASK-108: Special cases --

    #[test]
    fn test_var_expand_exact_2_hop() {
        let dir = tempdir().expect("tempdir");
        let mut engine = test_engine(dir.path());
        let knows = engine.get_or_create_rel_type("KNOWS");
        let nodes = create_linear_chain(&mut engine, knows, 4);

        let mut source = Record::new();
        source.insert("a".to_string(), Value::Node(nodes[0]));

        let results = execute_var_length_expand(
            vec![source],
            "a",
            None,
            "b",
            Some(knows),
            &RelDirection::Outgoing,
            2,
            2,
            &engine,
        );

        // Exact 2-hop: only n2
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].get("b"), Some(&Value::Node(nodes[2])));
    }

    #[test]
    fn test_var_expand_zero_min_hops() {
        let dir = tempdir().expect("tempdir");
        let mut engine = test_engine(dir.path());
        let knows = engine.get_or_create_rel_type("KNOWS");
        let n0 = engine.create_node(vec![], vec![]);
        let n1 = engine.create_node(vec![], vec![]);
        engine.create_edge(n0, n1, knows, vec![]).expect("edge");

        let mut source = Record::new();
        source.insert("a".to_string(), Value::Node(n0));

        let results = execute_var_length_expand(
            vec![source],
            "a",
            Some("r"),
            "b",
            Some(knows),
            &RelDirection::Outgoing,
            0,
            1,
            &engine,
        );

        // 0-hop: n0 itself (with r=Null), 1-hop: n1
        assert_eq!(results.len(), 2);

        // Check zero-hop record
        let zero_hop: Vec<_> = results
            .iter()
            .filter(|r| r.get("b") == Some(&Value::Node(n0)))
            .collect();
        assert_eq!(zero_hop.len(), 1);
        assert_eq!(zero_hop[0].get("r"), Some(&Value::Null));

        // Check one-hop record
        let one_hop: Vec<_> = results
            .iter()
            .filter(|r| r.get("b") == Some(&Value::Node(n1)))
            .collect();
        assert_eq!(one_hop.len(), 1);
    }

    #[test]
    fn test_var_expand_zero_min_no_edges() {
        let dir = tempdir().expect("tempdir");
        let mut engine = test_engine(dir.path());
        let n0 = engine.create_node(vec![], vec![]);

        let mut source = Record::new();
        source.insert("a".to_string(), Value::Node(n0));

        let results = execute_var_length_expand(
            vec![source],
            "a",
            None,
            "b",
            None,
            &RelDirection::Outgoing,
            0,
            1,
            &engine,
        );

        // Only zero-hop match
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].get("b"), Some(&Value::Node(n0)));
    }

    #[test]
    fn test_var_expand_multiple_sources() {
        let dir = tempdir().expect("tempdir");
        let mut engine = test_engine(dir.path());
        let knows = engine.get_or_create_rel_type("KNOWS");
        let n0 = engine.create_node(vec![], vec![]);
        let n1 = engine.create_node(vec![], vec![]);
        let n2 = engine.create_node(vec![], vec![]);
        engine.create_edge(n0, n1, knows, vec![]).expect("edge");
        engine.create_edge(n2, n1, knows, vec![]).expect("edge");

        let mut s0 = Record::new();
        s0.insert("a".to_string(), Value::Node(n0));
        let mut s1 = Record::new();
        s1.insert("a".to_string(), Value::Node(n2));

        let results = execute_var_length_expand(
            vec![s0, s1],
            "a",
            None,
            "b",
            Some(knows),
            &RelDirection::Outgoing,
            1,
            1,
            &engine,
        );

        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_var_expand_preserves_source_vars() {
        let dir = tempdir().expect("tempdir");
        let mut engine = test_engine(dir.path());
        let knows = engine.get_or_create_rel_type("KNOWS");
        let n0 = engine.create_node(vec![], vec![]);
        let n1 = engine.create_node(vec![], vec![]);
        engine.create_edge(n0, n1, knows, vec![]).expect("edge");

        let mut source = Record::new();
        source.insert("a".to_string(), Value::Node(n0));
        source.insert("extra".to_string(), Value::Int64(42));

        let results = execute_var_length_expand(
            vec![source],
            "a",
            None,
            "b",
            Some(knows),
            &RelDirection::Outgoing,
            1,
            1,
            &engine,
        );

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].get("extra"), Some(&Value::Int64(42)));
    }
}
