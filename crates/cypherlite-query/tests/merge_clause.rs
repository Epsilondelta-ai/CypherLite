// Integration tests for MERGE clause (Group P, SPEC-DB-003)
//
// Tests the full pipeline: parse -> semantic -> plan -> execute

use cypherlite_core::{DatabaseConfig, SyncMode};
use cypherlite_query::api::CypherLite;
use tempfile::tempdir;

fn open_db(dir: &std::path::Path) -> CypherLite {
    let config = DatabaseConfig {
        path: dir.join("test.cyl"),
        wal_sync_mode: SyncMode::Normal,
        ..Default::default()
    };
    CypherLite::open(config).expect("open db")
}

// ======================================================================
// TASK-090: MERGE integration tests
// ======================================================================

/// MERGE on empty DB creates a new node.
#[test]
fn test_merge_creates_node_on_empty_db() {
    let dir = tempdir().expect("tempdir");
    let mut db = open_db(dir.path());

    db.execute("MERGE (n:Person {name: 'Alice'})")
        .expect("merge");

    let result = db
        .execute("MATCH (n:Person) RETURN n.name")
        .expect("match");
    assert_eq!(result.rows.len(), 1);
    assert_eq!(
        result.rows[0].get_as::<String>("n.name"),
        Some("Alice".to_string())
    );
}

/// MERGE is idempotent: running same MERGE twice should not create duplicates.
#[test]
fn test_merge_idempotent() {
    let dir = tempdir().expect("tempdir");
    let mut db = open_db(dir.path());

    db.execute("MERGE (n:Person {name: 'Alice'})")
        .expect("first merge");
    db.execute("MERGE (n:Person {name: 'Alice'})")
        .expect("second merge");

    let result = db
        .execute("MATCH (n:Person) RETURN n.name")
        .expect("match");
    assert_eq!(result.rows.len(), 1, "should still be 1 node, not 2");
}

/// MERGE with ON CREATE SET applies SET only when creating.
#[test]
fn test_merge_on_create_set() {
    let dir = tempdir().expect("tempdir");
    let mut db = open_db(dir.path());

    db.execute("MERGE (n:Person {name: 'Alice'}) ON CREATE SET n.created = true")
        .expect("merge with on create");

    let result = db
        .execute("MATCH (n:Person) RETURN n.name, n.created")
        .expect("match");
    assert_eq!(result.rows.len(), 1);
    assert_eq!(result.rows[0].get_as::<bool>("n.created"), Some(true));
}

/// MERGE with ON MATCH SET applies SET only when matching existing node.
#[test]
fn test_merge_on_match_set() {
    let dir = tempdir().expect("tempdir");
    let mut db = open_db(dir.path());

    // First create the node
    db.execute("CREATE (n:Person {name: 'Alice'})")
        .expect("create");

    // MERGE should match existing node and apply ON MATCH SET
    db.execute("MERGE (n:Person {name: 'Alice'}) ON MATCH SET n.seen = true")
        .expect("merge with on match");

    let result = db
        .execute("MATCH (n:Person) RETURN n.name, n.seen")
        .expect("match");
    assert_eq!(result.rows.len(), 1);
    assert_eq!(result.rows[0].get_as::<bool>("n.seen"), Some(true));
}

/// MERGE relationship between existing nodes.
#[test]
fn test_merge_relationship() {
    let dir = tempdir().expect("tempdir");
    let mut db = open_db(dir.path());

    // Create the full relationship via MERGE: nodes + edge in one pattern
    db.execute("MERGE (a:Person {name: 'Alice'})-[:KNOWS]->(b:Person {name: 'Bob'})")
        .expect("merge relationship");

    // Verify nodes and relationship were created
    let result = db
        .execute("MATCH (a:Person)-[:KNOWS]->(b:Person) RETURN a.name, b.name")
        .expect("match relationship");
    assert_eq!(result.rows.len(), 1);

    // Run same MERGE again - should be idempotent
    db.execute("MERGE (a:Person {name: 'Alice'})-[:KNOWS]->(b:Person {name: 'Bob'})")
        .expect("second merge");

    let result2 = db
        .execute("MATCH (a:Person)-[:KNOWS]->(b:Person) RETURN a.name, b.name")
        .expect("match after second merge");
    assert_eq!(result2.rows.len(), 1, "should still be 1 relationship");
}

/// MERGE with ON CREATE SET does NOT apply SET when node already exists.
#[test]
fn test_merge_on_create_set_not_applied_when_exists() {
    let dir = tempdir().expect("tempdir");
    let mut db = open_db(dir.path());

    // Create node first
    db.execute("CREATE (n:Person {name: 'Alice'})")
        .expect("create");

    // MERGE should match, so ON CREATE SET should NOT apply
    db.execute("MERGE (n:Person {name: 'Alice'}) ON CREATE SET n.new_prop = true")
        .expect("merge");

    let result = db
        .execute("MATCH (n:Person) RETURN n.name, n.new_prop")
        .expect("match");
    assert_eq!(result.rows.len(), 1);
    // new_prop should be null since ON CREATE was not triggered
    assert_eq!(result.rows[0].get_as::<bool>("n.new_prop"), None);
}

/// MERGE with ON MATCH SET does NOT apply SET when creating new node.
#[test]
fn test_merge_on_match_set_not_applied_when_creating() {
    let dir = tempdir().expect("tempdir");
    let mut db = open_db(dir.path());

    // MERGE on empty DB creates node; ON MATCH should NOT apply
    db.execute("MERGE (n:Person {name: 'Alice'}) ON MATCH SET n.seen = true")
        .expect("merge");

    let result = db
        .execute("MATCH (n:Person) RETURN n.name, n.seen")
        .expect("match");
    assert_eq!(result.rows.len(), 1);
    // seen should be null since ON MATCH was not triggered
    assert_eq!(result.rows[0].get_as::<bool>("n.seen"), None);
}
