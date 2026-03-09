// Node/Edge CRUD integration tests

use cypherlite_core::{DatabaseConfig, NodeId, PropertyValue, SyncMode};
use cypherlite_storage::StorageEngine;
use tempfile::tempdir;

fn test_engine() -> (tempfile::TempDir, StorageEngine) {
    let dir = tempdir().expect("tempdir");
    let config = DatabaseConfig {
        path: dir.path().join("test.cyl"),
        wal_sync_mode: SyncMode::Normal,
        ..Default::default()
    };
    let engine = StorageEngine::open(config).expect("open");
    (dir, engine)
}

// REQ-STORE-001: Create node with unique NodeId
#[test]
fn test_create_multiple_nodes_unique_ids() {
    let (_dir, mut engine) = test_engine();
    let ids: Vec<NodeId> = (0..100)
        .map(|i| engine.create_node(vec![1], vec![(1, PropertyValue::Int64(i))]))
        .collect();

    // All IDs should be unique
    let mut sorted = ids.clone();
    sorted.sort();
    sorted.dedup();
    assert_eq!(sorted.len(), 100);
}

// REQ-STORE-002: Lookup by NodeId
#[test]
fn test_node_lookup() {
    let (_dir, mut engine) = test_engine();
    let id = engine.create_node(
        vec![10, 20],
        vec![
            (1, PropertyValue::String("Alice".into())),
            (2, PropertyValue::Int64(30)),
        ],
    );
    let node = engine.get_node(id).expect("found");
    assert_eq!(node.labels, vec![10, 20]);
    assert_eq!(node.properties.len(), 2);
}

// REQ-STORE-003: Update node properties
#[test]
fn test_node_update_properties() {
    let (_dir, mut engine) = test_engine();
    let id = engine.create_node(vec![], vec![(1, PropertyValue::Int64(10))]);
    engine
        .update_node(
            id,
            vec![
                (1, PropertyValue::Int64(20)),
                (2, PropertyValue::Bool(true)),
            ],
        )
        .expect("update");
    let node = engine.get_node(id).expect("found");
    assert_eq!(node.properties.len(), 2);
    assert_eq!(node.properties[0].1, PropertyValue::Int64(20));
}

// REQ-STORE-004: Delete node cascades to edges
#[test]
fn test_delete_node_cascades() {
    let (_dir, mut engine) = test_engine();
    let n1 = engine.create_node(vec![], vec![]);
    let n2 = engine.create_node(vec![], vec![]);
    let n3 = engine.create_node(vec![], vec![]);

    engine.create_edge(n1, n2, 1, vec![]).expect("e1");
    engine.create_edge(n1, n3, 2, vec![]).expect("e2");
    engine.create_edge(n2, n3, 3, vec![]).expect("e3");
    assert_eq!(engine.edge_count(), 3);

    engine.delete_node(n1).expect("delete");
    assert_eq!(engine.edge_count(), 1); // only n2->n3 remains
    assert!(engine.get_node(n1).is_none());
}

// REQ-STORE-005: Create edge with adjacency chain
#[test]
fn test_create_edge_adjacency() {
    let (_dir, mut engine) = test_engine();
    let n1 = engine.create_node(vec![], vec![]);
    let n2 = engine.create_node(vec![], vec![]);

    let e1 = engine.create_edge(n1, n2, 1, vec![]).expect("e");
    let node = engine.get_node(n1).expect("n1");
    assert_eq!(node.next_edge_id, Some(e1));
}

// REQ-STORE-006: Lookup by EdgeId
#[test]
fn test_edge_lookup() {
    let (_dir, mut engine) = test_engine();
    let n1 = engine.create_node(vec![], vec![]);
    let n2 = engine.create_node(vec![], vec![]);
    let e = engine
        .create_edge(n1, n2, 42, vec![(1, PropertyValue::String("KNOWS".into()))])
        .expect("e");
    let edge = engine.get_edge(e).expect("found");
    assert_eq!(edge.start_node, n1);
    assert_eq!(edge.end_node, n2);
    assert_eq!(edge.rel_type_id, 42);
}

// REQ-STORE-007: Get edges for node (Index-Free Adjacency)
#[test]
fn test_index_free_adjacency() {
    let (_dir, mut engine) = test_engine();
    let center = engine.create_node(vec![], vec![]);
    let mut neighbors = Vec::new();
    for _ in 0..10 {
        let n = engine.create_node(vec![], vec![]);
        engine.create_edge(center, n, 1, vec![]).expect("e");
        neighbors.push(n);
    }

    let edges = engine.get_edges_for_node(center);
    assert_eq!(edges.len(), 10);
}

// REQ-STORE-008: Delete edge updates chain
#[test]
fn test_delete_edge() {
    let (_dir, mut engine) = test_engine();
    let n1 = engine.create_node(vec![], vec![]);
    let n2 = engine.create_node(vec![], vec![]);
    let e = engine.create_edge(n1, n2, 1, vec![]).expect("e");
    engine.delete_edge(e).expect("delete");
    assert!(engine.get_edge(e).is_none());
    assert_eq!(engine.edge_count(), 0);
}

// REQ-STORE-011: All property types
#[test]
fn test_all_property_types() {
    let (_dir, mut engine) = test_engine();
    let id = engine.create_node(
        vec![1],
        vec![
            (0, PropertyValue::Null),
            (1, PropertyValue::Bool(true)),
            (2, PropertyValue::Int64(-42)),
            (3, PropertyValue::Float64(3.14)),
            (4, PropertyValue::String("hello".into())),
            (5, PropertyValue::Bytes(vec![0xDE, 0xAD])),
            (
                6,
                PropertyValue::Array(vec![PropertyValue::Int64(1), PropertyValue::Null]),
            ),
        ],
    );
    let node = engine.get_node(id).expect("found");
    assert_eq!(node.properties.len(), 7);
    assert_eq!(node.properties[0].1, PropertyValue::Null);
    assert_eq!(
        node.properties[6].1,
        PropertyValue::Array(vec![PropertyValue::Int64(1), PropertyValue::Null])
    );
}

// Graph pattern: triangle
#[test]
fn test_triangle_pattern() {
    let (_dir, mut engine) = test_engine();
    let a = engine.create_node(vec![1], vec![(1, PropertyValue::String("A".into()))]);
    let b = engine.create_node(vec![1], vec![(1, PropertyValue::String("B".into()))]);
    let c = engine.create_node(vec![1], vec![(1, PropertyValue::String("C".into()))]);

    engine.create_edge(a, b, 1, vec![]).expect("ab");
    engine.create_edge(b, c, 1, vec![]).expect("bc");
    engine.create_edge(c, a, 1, vec![]).expect("ca");

    assert_eq!(engine.node_count(), 3);
    assert_eq!(engine.edge_count(), 3);

    // Each node should have 2 edges
    assert_eq!(engine.get_edges_for_node(a).len(), 2);
    assert_eq!(engine.get_edges_for_node(b).len(), 2);
    assert_eq!(engine.get_edges_for_node(c).len(), 2);
}
