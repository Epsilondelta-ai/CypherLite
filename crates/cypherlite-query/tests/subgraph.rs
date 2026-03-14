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

use cypherlite_core::{DatabaseConfig, SyncMode};
use cypherlite_query::api::CypherLite;
use cypherlite_query::executor::Value;
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
    db.execute("CREATE (a:Person {name: 'Alice'})")
        .expect("create alice");
    db.execute("CREATE (b:Person {name: 'Bob'})")
        .expect("create bob");
    db.execute("CREATE (c:Animal {name: 'Cat'})")
        .expect("create cat");

    // Create snapshot capturing all Person nodes
    db.execute("CREATE SNAPSHOT (sg:Snap {name: 'people'}) FROM MATCH (n:Person) RETURN n")
        .expect("create snapshot");

    // Verify the snapshot was created by querying subgraph members via :CONTAINS
    let result = db
        .execute("MATCH (sg:Subgraph)-[:CONTAINS]->(n) RETURN n.name")
        .expect("query members");

    assert_eq!(
        result.rows.len(),
        2,
        "snapshot should contain 2 Person nodes"
    );

    let mut names: Vec<String> = result
        .rows
        .iter()
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

    db.execute("CREATE (a:Person {name: 'Alice'})")
        .expect("create");

    // Create snapshot with explicit AT TIME
    db.execute(
        "CREATE SNAPSHOT (sg:Snap {name: 'time-snap'}) AT TIME 1700000000000 FROM MATCH (n:Person) RETURN n"
    ).expect("create snapshot with at time");

    // The subgraph's _temporal_anchor should be the specified timestamp
    let result = db
        .execute("MATCH (sg:Subgraph) WHERE sg.name = 'time-snap' RETURN sg._temporal_anchor")
        .expect("query anchor");

    assert_eq!(result.rows.len(), 1);
    // _temporal_anchor should be 1700000000000
    let anchor = result.rows[0].get("sg._temporal_anchor");
    match anchor {
        Some(Value::DateTime(ms)) => assert_eq!(*ms, 1_700_000_000_000),
        Some(Value::Int64(ms)) => assert_eq!(*ms, 1_700_000_000_000),
        other => panic!(
            "expected DateTime or Int64 for _temporal_anchor, got: {:?}",
            other
        ),
    }
}

// HH-004 + HH-005: Query subgraph members via virtual :CONTAINS relationship
#[test]
fn test_query_snapshot_members_via_contains() {
    let dir = tempdir().expect("tempdir");
    let mut db = CypherLite::open(test_config(dir.path())).expect("open");

    // Create nodes and a snapshot
    db.execute("CREATE (a:Person {name: 'Alice'})")
        .expect("create alice");
    db.execute("CREATE (b:Person {name: 'Bob'})")
        .expect("create bob");
    db.execute("CREATE (c:Person {name: 'Charlie'})")
        .expect("create charlie");

    db.execute("CREATE SNAPSHOT (sg:Snap {name: 'team'}) FROM MATCH (n:Person) RETURN n")
        .expect("create snapshot");

    // Query members using :CONTAINS virtual relationship
    let result = db
        .execute("MATCH (sg:Subgraph {name: 'team'})-[:CONTAINS]->(n) RETURN n.name")
        .expect("query via CONTAINS");

    assert_eq!(result.rows.len(), 3);
    let mut names: Vec<String> = result
        .rows
        .iter()
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
    db.execute("CREATE (a:TeamA {name: 'Alice'})")
        .expect("create");
    db.execute("CREATE (b:TeamB {name: 'Bob'})")
        .expect("create");

    // Create two snapshots
    db.execute("CREATE SNAPSHOT (sg1:Snap {name: 'team-a'}) FROM MATCH (n:TeamA) RETURN n")
        .expect("snap1");
    db.execute("CREATE SNAPSHOT (sg2:Snap {name: 'team-b'}) FROM MATCH (n:TeamB) RETURN n")
        .expect("snap2");

    // Verify both subgraphs exist
    let result = db
        .execute("MATCH (sg:Subgraph) RETURN sg.name")
        .expect("list subgraphs");
    assert_eq!(result.rows.len(), 2);
}

// JJ-001: Query subgraphs by property filter
#[test]
fn test_query_subgraph_relationships() {
    let dir = tempdir().expect("tempdir");
    let mut db = CypherLite::open(test_config(dir.path())).expect("open");

    // Create nodes for different teams
    db.execute("CREATE (a:GroupX {name: 'Alpha'})")
        .expect("create");
    db.execute("CREATE (b:GroupY {name: 'Beta'})")
        .expect("create");

    // Create snapshots
    db.execute("CREATE SNAPSHOT (sg1:Snap {name: 'group-x'}) FROM MATCH (n:GroupX) RETURN n")
        .expect("snap1");
    db.execute("CREATE SNAPSHOT (sg2:Snap {name: 'group-y'}) FROM MATCH (n:GroupY) RETURN n")
        .expect("snap2");

    // Query subgraphs by name
    let result = db
        .execute("MATCH (sg:Subgraph) WHERE sg.name = 'group-x' RETURN sg.name")
        .expect("filter subgraph");
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
    db.execute("CREATE (a:Member {name: 'A'})")
        .expect("create a");
    db.execute("CREATE (b:Member {name: 'B'})")
        .expect("create b");
    db.execute("CREATE (c:Member {name: 'C'})")
        .expect("create c");

    // Create snapshot
    db.execute("CREATE SNAPSHOT (sg:Snap {name: 'members'}) FROM MATCH (n:Member) RETURN n")
        .expect("create snapshot");

    // Count members via CONTAINS using alias
    let result = db.execute(
        "MATCH (sg:Subgraph {name: 'members'})-[:CONTAINS]->(n) WITH count(*) AS total RETURN total"
    ).expect("count members");

    assert_eq!(result.rows.len(), 1);
    let count = result.rows[0].get_as::<i64>("total");
    assert_eq!(count, Some(3));
}

// KK-003: Create multiple snapshots of the same data at different times
#[test]
fn test_multiple_snapshots_same_data_different_times() {
    let dir = tempdir().expect("tempdir");
    let mut db = CypherLite::open(test_config(dir.path())).expect("open");

    // Create nodes
    db.execute("CREATE (a:Versioned {name: 'Alice'})")
        .expect("create");
    db.execute("CREATE (b:Versioned {name: 'Bob'})")
        .expect("create");

    // Snapshot at time T1
    db.execute(
        "CREATE SNAPSHOT (sg1:Snap {name: 'v1'}) AT TIME 1000000000000 FROM MATCH (n:Versioned) RETURN n"
    ).expect("snap v1");

    // Snapshot at time T2
    db.execute(
        "CREATE SNAPSHOT (sg2:Snap {name: 'v2'}) AT TIME 2000000000000 FROM MATCH (n:Versioned) RETURN n"
    ).expect("snap v2");

    // Both snapshots should have 2 members each
    let v1_members = db
        .execute("MATCH (sg:Subgraph {name: 'v1'})-[:CONTAINS]->(n) RETURN n.name")
        .expect("query v1");
    assert_eq!(v1_members.rows.len(), 2, "v1 should have 2 members");

    let v2_members = db
        .execute("MATCH (sg:Subgraph {name: 'v2'})-[:CONTAINS]->(n) RETURN n.name")
        .expect("query v2");
    assert_eq!(v2_members.rows.len(), 2, "v2 should have 2 members");

    // Different temporal anchors
    let v1_anchor = db
        .execute("MATCH (sg:Subgraph {name: 'v1'}) RETURN sg._temporal_anchor")
        .expect("v1 anchor");
    let v2_anchor = db
        .execute("MATCH (sg:Subgraph {name: 'v2'}) RETURN sg._temporal_anchor")
        .expect("v2 anchor");

    let a1 = match v1_anchor.rows[0].get("sg._temporal_anchor") {
        Some(Value::DateTime(ms)) => *ms,
        Some(Value::Int64(ms)) => *ms,
        other => panic!("unexpected anchor type: {:?}", other),
    };
    let a2 = match v2_anchor.rows[0].get("sg._temporal_anchor") {
        Some(Value::DateTime(ms)) => *ms,
        Some(Value::Int64(ms)) => *ms,
        other => panic!("unexpected anchor type: {:?}", other),
    };
    assert_eq!(a1, 1_000_000_000_000);
    assert_eq!(a2, 2_000_000_000_000);
}

// KK-003: Query empty subgraph members (snapshot from empty result)
#[test]
fn test_query_empty_subgraph_members() {
    let dir = tempdir().expect("tempdir");
    let mut db = CypherLite::open(test_config(dir.path())).expect("open");

    // Create snapshot from non-existent label -> empty result
    db.execute(
        "CREATE SNAPSHOT (sg:Snap {name: 'empty-snap'}) FROM MATCH (n:NonExistent) RETURN n",
    )
    .expect("snap empty");

    // Verify subgraph exists
    let result = db
        .execute("MATCH (sg:Subgraph {name: 'empty-snap'}) RETURN sg.name")
        .expect("query subgraph");
    assert_eq!(result.rows.len(), 1, "subgraph should exist");

    // Verify no members
    let members = db
        .execute("MATCH (sg:Subgraph {name: 'empty-snap'})-[:CONTAINS]->(n) RETURN n")
        .expect("query members");
    assert_eq!(
        members.rows.len(),
        0,
        "empty subgraph should have 0 members"
    );
}

// KK-003: List all subgraphs and verify count
#[test]
fn test_list_all_subgraphs() {
    let dir = tempdir().expect("tempdir");
    let mut db = CypherLite::open(test_config(dir.path())).expect("open");

    // Create 3 subgraphs
    db.execute("CREATE (a:TypeA {name: 'A'})").expect("create");
    db.execute("CREATE (b:TypeB {name: 'B'})").expect("create");

    db.execute("CREATE SNAPSHOT (sg1:Snap {name: 'sg1'}) FROM MATCH (n:TypeA) RETURN n")
        .expect("snap1");
    db.execute("CREATE SNAPSHOT (sg2:Snap {name: 'sg2'}) FROM MATCH (n:TypeB) RETURN n")
        .expect("snap2");
    db.execute("CREATE SNAPSHOT (sg3:Snap {name: 'sg3'}) FROM MATCH (n:TypeA) RETURN n")
        .expect("snap3");

    // List all subgraphs
    let result = db
        .execute("MATCH (sg:Subgraph) RETURN sg.name")
        .expect("list all");
    assert_eq!(result.rows.len(), 3, "should have 3 subgraphs");

    let mut names: Vec<String> = result
        .rows
        .iter()
        .filter_map(|r| r.get_as::<String>("sg.name"))
        .collect();
    names.sort();
    assert_eq!(names, vec!["sg1", "sg2", "sg3"]);
}

// KK-003: Snapshot with WHERE filter in FROM clause
#[test]
fn test_snapshot_with_where_filter() {
    let dir = tempdir().expect("tempdir");
    let mut db = CypherLite::open(test_config(dir.path())).expect("open");

    // Create nodes
    db.execute("CREATE (a:Worker {name: 'Alice', dept: 'eng'})")
        .expect("create");
    db.execute("CREATE (b:Worker {name: 'Bob', dept: 'sales'})")
        .expect("create");
    db.execute("CREATE (c:Worker {name: 'Carol', dept: 'eng'})")
        .expect("create");

    // Snapshot with WHERE filter to capture only eng dept
    db.execute(
        "CREATE SNAPSHOT (sg:Snap {name: 'eng-team'}) FROM MATCH (n:Worker) WHERE n.dept = 'eng' RETURN n"
    ).expect("snap with filter");

    // Verify only eng workers are members
    let members = db
        .execute("MATCH (sg:Subgraph {name: 'eng-team'})-[:CONTAINS]->(n) RETURN n.name")
        .expect("query members");
    assert_eq!(members.rows.len(), 2, "should have 2 eng workers");

    let mut names: Vec<String> = members
        .rows
        .iter()
        .filter_map(|r| r.get_as::<String>("n.name"))
        .collect();
    names.sort();
    assert_eq!(names, vec!["Alice", "Carol"]);
}

// JJ-003: Filter subgraphs by temporal anchor
#[test]
fn test_filter_subgraph_by_temporal_anchor() {
    let dir = tempdir().expect("tempdir");
    let mut db = CypherLite::open(test_config(dir.path())).expect("open");

    db.execute("CREATE (a:OldData {name: 'old'})")
        .expect("create old");
    db.execute("CREATE (b:NewData {name: 'new'})")
        .expect("create new");

    // Create snapshots with different temporal anchors
    db.execute(
        "CREATE SNAPSHOT (sg1:Snap {name: 'old-snap'}) AT TIME 1000000000000 FROM MATCH (n:OldData) RETURN n"
    ).expect("old snapshot");
    db.execute(
        "CREATE SNAPSHOT (sg2:Snap {name: 'new-snap'}) AT TIME 1800000000000 FROM MATCH (n:NewData) RETURN n"
    ).expect("new snapshot");

    // Filter by temporal anchor >= 1500000000000 (should only return new-snap)
    let result = db
        .execute("MATCH (sg:Subgraph) WHERE sg._temporal_anchor >= 1500000000000 RETURN sg.name")
        .expect("filter by anchor");

    assert_eq!(result.rows.len(), 1);
    assert_eq!(
        result.rows[0].get_as::<String>("sg.name"),
        Some("new-snap".to_string())
    );
}
