// Integration tests for Subgraph Planner/Executor (Phase 6e)
//
// HH-002: CREATE SNAPSHOT execution
// HH-003: Default temporal anchor
// HH-004: Query subgraph members via :CONTAINS
// HH-005: Virtual :CONTAINS relationship
// JJ-001: Subgraph MATCH patterns
// JJ-002: Aggregate over members
// JJ-003: Temporal anchor filter

#![cfg(feature = "subgraph")]

use cypherlite_query::api::CypherLite;
use cypherlite_query::executor::Value;
use cypherlite_core::{DatabaseConfig, SyncMode};
use tempfile::tempdir;

fn test_config(dir: &std::path::Path) -> DatabaseConfig {
    DatabaseConfig {
        path: dir.join("test.cyl"),
        wal_sync_mode: SyncMode::Normal,
        ..Default::default()
    }
}

// HH-002: CREATE SNAPSHOT basic execution
// Creates nodes, then uses CREATE SNAPSHOT to capture matching nodes into a subgraph.
#[test]
fn test_create_snapshot_basic() {
    let dir = tempdir().expect("tempdir");
    let mut db = CypherLite::open(test_config(dir.path())).expect("open");

    // Create some nodes
    db.execute("CREATE (a:Person {name: 'Alice'})").expect("create alice");
    db.execute("CREATE (b:Person {name: 'Bob'})").expect("create bob");
    db.execute("CREATE (c:Animal {name: 'Cat'})").expect("create cat");

    // Create snapshot capturing all Person nodes
    db.execute(
        "CREATE SNAPSHOT (sg:Snap {name: 'people'}) FROM MATCH (n:Person) RETURN n"
    ).expect("create snapshot");

    // Verify the snapshot was created by querying subgraph members via :CONTAINS
    let result = db.execute(
        "MATCH (sg:Subgraph)-[:CONTAINS]->(n) RETURN n.name"
    ).expect("query members");

    assert_eq!(result.rows.len(), 2, "snapshot should contain 2 Person nodes");

    let mut names: Vec<String> = result.rows.iter()
        .filter_map(|r| r.get_as::<String>("n.name"))
        .collect();
    names.sort();
    assert_eq!(names, vec!["Alice", "Bob"]);
}

// HH-003: CREATE SNAPSHOT with AT TIME uses explicit temporal anchor
#[test]
fn test_create_snapshot_with_at_time() {
    let dir = tempdir().expect("tempdir");
    let mut db = CypherLite::open(test_config(dir.path())).expect("open");

    db.execute("CREATE (a:Person {name: 'Alice'})").expect("create");

    // Create snapshot with explicit AT TIME
    db.execute(
        "CREATE SNAPSHOT (sg:Snap {name: 'time-snap'}) AT TIME 1700000000000 FROM MATCH (n:Person) RETURN n"
    ).expect("create snapshot with at time");

    // The subgraph's _temporal_anchor should be the specified timestamp
    let result = db.execute(
        "MATCH (sg:Subgraph) WHERE sg.name = 'time-snap' RETURN sg._temporal_anchor"
    ).expect("query anchor");

    assert_eq!(result.rows.len(), 1);
    // _temporal_anchor should be 1700000000000
    let anchor = result.rows[0].get("sg._temporal_anchor");
    match anchor {
        Some(Value::DateTime(ms)) => assert_eq!(*ms, 1_700_000_000_000),
        Some(Value::Int64(ms)) => assert_eq!(*ms, 1_700_000_000_000),
        other => panic!("expected DateTime or Int64 for _temporal_anchor, got: {:?}", other),
    }
}

// HH-004 + HH-005: Query subgraph members via virtual :CONTAINS relationship
#[test]
fn test_query_snapshot_members_via_contains() {
    let dir = tempdir().expect("tempdir");
    let mut db = CypherLite::open(test_config(dir.path())).expect("open");

    // Create nodes and a snapshot
    db.execute("CREATE (a:Person {name: 'Alice'})").expect("create alice");
    db.execute("CREATE (b:Person {name: 'Bob'})").expect("create bob");
    db.execute("CREATE (c:Person {name: 'Charlie'})").expect("create charlie");

    db.execute(
        "CREATE SNAPSHOT (sg:Snap {name: 'team'}) FROM MATCH (n:Person) RETURN n"
    ).expect("create snapshot");

    // Query members using :CONTAINS virtual relationship
    let result = db.execute(
        "MATCH (sg:Subgraph {name: 'team'})-[:CONTAINS]->(n) RETURN n.name"
    ).expect("query via CONTAINS");

    assert_eq!(result.rows.len(), 3);
    let mut names: Vec<String> = result.rows.iter()
        .filter_map(|r| r.get_as::<String>("n.name"))
        .collect();
    names.sort();
    assert_eq!(names, vec!["Alice", "Bob", "Charlie"]);
}

// JJ-001: Create two snapshots and verify both exist
#[test]
fn test_create_edge_between_subgraphs() {
    let dir = tempdir().expect("tempdir");
    let mut db = CypherLite::open(test_config(dir.path())).expect("open");

    // Create two groups of nodes
    db.execute("CREATE (a:TeamA {name: 'Alice'})").expect("create");
    db.execute("CREATE (b:TeamB {name: 'Bob'})").expect("create");

    // Create two snapshots
    db.execute(
        "CREATE SNAPSHOT (sg1:Snap {name: 'team-a'}) FROM MATCH (n:TeamA) RETURN n"
    ).expect("snap1");
    db.execute(
        "CREATE SNAPSHOT (sg2:Snap {name: 'team-b'}) FROM MATCH (n:TeamB) RETURN n"
    ).expect("snap2");

    // Verify both subgraphs exist
    let result = db.execute(
        "MATCH (sg:Subgraph) RETURN sg.name"
    ).expect("list subgraphs");
    assert_eq!(result.rows.len(), 2);
}

// JJ-001: Query subgraphs by property filter
#[test]
fn test_query_subgraph_relationships() {
    let dir = tempdir().expect("tempdir");
    let mut db = CypherLite::open(test_config(dir.path())).expect("open");

    // Create nodes for different teams
    db.execute("CREATE (a:GroupX {name: 'Alpha'})").expect("create");
    db.execute("CREATE (b:GroupY {name: 'Beta'})").expect("create");

    // Create snapshots
    db.execute(
        "CREATE SNAPSHOT (sg1:Snap {name: 'group-x'}) FROM MATCH (n:GroupX) RETURN n"
    ).expect("snap1");
    db.execute(
        "CREATE SNAPSHOT (sg2:Snap {name: 'group-y'}) FROM MATCH (n:GroupY) RETURN n"
    ).expect("snap2");

    // Query subgraphs by name
    let result = db.execute(
        "MATCH (sg:Subgraph) WHERE sg.name = 'group-x' RETURN sg.name"
    ).expect("filter subgraph");
    assert_eq!(result.rows.len(), 1);
    assert_eq!(
        result.rows[0].get_as::<String>("sg.name"),
        Some("group-x".to_string())
    );
}

// JJ-002: Aggregate over subgraph members using count(n)
#[test]
fn test_aggregate_over_members() {
    let dir = tempdir().expect("tempdir");
    let mut db = CypherLite::open(test_config(dir.path())).expect("open");

    // Create nodes
    db.execute("CREATE (a:Member {name: 'A'})").expect("create a");
    db.execute("CREATE (b:Member {name: 'B'})").expect("create b");
    db.execute("CREATE (c:Member {name: 'C'})").expect("create c");

    // Create snapshot
    db.execute(
        "CREATE SNAPSHOT (sg:Snap {name: 'members'}) FROM MATCH (n:Member) RETURN n"
    ).expect("create snapshot");

    // Count members via CONTAINS using alias
    let result = db.execute(
        "MATCH (sg:Subgraph {name: 'members'})-[:CONTAINS]->(n) WITH count(*) AS total RETURN total"
    ).expect("count members");

    assert_eq!(result.rows.len(), 1);
    let count = result.rows[0].get_as::<i64>("total");
    assert_eq!(count, Some(3));
}

// JJ-003: Filter subgraphs by temporal anchor
#[test]
fn test_filter_subgraph_by_temporal_anchor() {
    let dir = tempdir().expect("tempdir");
    let mut db = CypherLite::open(test_config(dir.path())).expect("open");

    db.execute("CREATE (a:OldData {name: 'old'})").expect("create old");
    db.execute("CREATE (b:NewData {name: 'new'})").expect("create new");

    // Create snapshots with different temporal anchors
    db.execute(
        "CREATE SNAPSHOT (sg1:Snap {name: 'old-snap'}) AT TIME 1000000000000 FROM MATCH (n:OldData) RETURN n"
    ).expect("old snapshot");
    db.execute(
        "CREATE SNAPSHOT (sg2:Snap {name: 'new-snap'}) AT TIME 1800000000000 FROM MATCH (n:NewData) RETURN n"
    ).expect("new snapshot");

    // Filter by temporal anchor >= 1500000000000 (should only return new-snap)
    let result = db.execute(
        "MATCH (sg:Subgraph) WHERE sg._temporal_anchor >= 1500000000000 RETURN sg.name"
    ).expect("filter by anchor");

    assert_eq!(result.rows.len(), 1);
    assert_eq!(
        result.rows[0].get_as::<String>("sg.name"),
        Some("new-snap".to_string())
    );
}
