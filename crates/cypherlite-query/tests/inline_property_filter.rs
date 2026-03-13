// Integration tests for inline property filters in MATCH patterns (Phase 8a, SPEC-DB-008)
//
// Tests verify that `MATCH (n:Label {key: value})` correctly filters nodes
// through the full query pipeline: parse -> plan -> execute.

use cypherlite_core::{DatabaseConfig, SyncMode};
use cypherlite_query::api::CypherLite;
use tempfile::tempdir;

fn test_db(dir: &std::path::Path) -> CypherLite {
    let config = DatabaseConfig {
        path: dir.join("test.cyl"),
        wal_sync_mode: SyncMode::Normal,
        ..Default::default()
    };
    CypherLite::open(config).expect("open")
}

// ======================================================================
// Task 8a-3: Node inline property filter tests
// ======================================================================

/// Single property filter: MATCH (n:Person {name: 'Alice'}) should return only Alice.
#[test]
fn inline_filter_single_property() {
    let dir = tempdir().expect("tempdir");
    let mut db = test_db(dir.path());

    db.execute("CREATE (:Person {name: 'Alice', age: 30})")
        .expect("create alice");
    db.execute("CREATE (:Person {name: 'Bob', age: 25})")
        .expect("create bob");
    db.execute("CREATE (:Person {name: 'Charlie', age: 35})")
        .expect("create charlie");

    let result = db
        .execute("MATCH (n:Person {name: 'Alice'}) RETURN n.name, n.age")
        .expect("query");
    assert_eq!(result.rows.len(), 1, "should return only Alice");
    assert_eq!(
        result.rows[0].get_as::<String>("n.name"),
        Some("Alice".to_string())
    );
}

/// Multiple property filter: MATCH (n:Person {name: 'Alice', age: 30}) should match exactly.
#[test]
fn inline_filter_multiple_properties() {
    let dir = tempdir().expect("tempdir");
    let mut db = test_db(dir.path());

    db.execute("CREATE (:Person {name: 'Alice', age: 30})")
        .expect("create alice");
    db.execute("CREATE (:Person {name: 'Alice', age: 25})")
        .expect("create alice-younger");
    db.execute("CREATE (:Person {name: 'Bob', age: 30})")
        .expect("create bob");

    let result = db
        .execute("MATCH (n:Person {name: 'Alice', age: 30}) RETURN n.name, n.age")
        .expect("query");
    assert_eq!(
        result.rows.len(),
        1,
        "should return only Alice with age 30"
    );
    assert_eq!(
        result.rows[0].get_as::<String>("n.name"),
        Some("Alice".to_string())
    );
    assert_eq!(result.rows[0].get_as::<i64>("n.age"), Some(30));
}

/// Null property filter: MATCH (n:Person {email: null}) should return no nodes.
/// In Cypher semantics, `null = null` evaluates to `null` (not true), so
/// inline `{prop: null}` matches nothing -- this is the correct behavior.
#[test]
fn inline_filter_null_property() {
    let dir = tempdir().expect("tempdir");
    let mut db = test_db(dir.path());

    db.execute("CREATE (:Person {name: 'Alice', email: 'alice@example.com'})")
        .expect("create alice");
    db.execute("CREATE (:Person {name: 'Bob'})").expect("create bob");

    let result = db
        .execute("MATCH (n:Person {email: null}) RETURN n.name")
        .expect("query");
    assert!(
        result.rows.is_empty(),
        "null = null is null in Cypher, so no node matches {{email: null}}"
    );
}

/// WHERE combination: inline filter + WHERE clause should both apply.
#[test]
fn inline_filter_combined_with_where() {
    let dir = tempdir().expect("tempdir");
    let mut db = test_db(dir.path());

    db.execute("CREATE (:Person {name: 'Alice', age: 30})")
        .expect("create alice");
    db.execute("CREATE (:Person {name: 'Alice', age: 20})")
        .expect("create alice-young");
    db.execute("CREATE (:Person {name: 'Bob', age: 40})")
        .expect("create bob");

    let result = db
        .execute("MATCH (n:Person {name: 'Alice'}) WHERE n.age > 25 RETURN n.name, n.age")
        .expect("query");
    assert_eq!(
        result.rows.len(),
        1,
        "should return only Alice with age > 25"
    );
    assert_eq!(
        result.rows[0].get_as::<String>("n.name"),
        Some("Alice".to_string())
    );
    assert_eq!(result.rows[0].get_as::<i64>("n.age"), Some(30));
}

/// Empty map: MATCH (n:Person {}) should return all Person nodes (no filter).
#[test]
fn inline_filter_empty_map() {
    let dir = tempdir().expect("tempdir");
    let mut db = test_db(dir.path());

    db.execute("CREATE (:Person {name: 'Alice'})").expect("create");
    db.execute("CREATE (:Person {name: 'Bob'})").expect("create");

    let result = db
        .execute("MATCH (n:Person {}) RETURN n.name")
        .expect("query");
    assert_eq!(result.rows.len(), 2, "empty map should return all Person nodes");
}

/// No match: MATCH (n:Person {name: 'NonExistent'}) should return empty result.
#[test]
fn inline_filter_no_match() {
    let dir = tempdir().expect("tempdir");
    let mut db = test_db(dir.path());

    db.execute("CREATE (:Person {name: 'Alice'})").expect("create");
    db.execute("CREATE (:Person {name: 'Bob'})").expect("create");

    let result = db
        .execute("MATCH (n:Person {name: 'NonExistent'}) RETURN n.name")
        .expect("query");
    assert!(
        result.rows.is_empty(),
        "non-existent property value should yield empty result"
    );
}

// ======================================================================
// Task 8b-5: Relationship and target node inline property filter tests
//
// Note: Each graph is created in a single CREATE chain so that
// property keys share the same catalog generation.  This avoids the
// known cross-execute property resolution issue.
// ======================================================================

/// Relationship single property filter:
/// MATCH (a)-[r:KNOWS {since: 2020}]->(b) should filter edges by property.
#[test]
fn inline_filter_rel_single_property() {
    let dir = tempdir().expect("tempdir");
    let mut db = test_db(dir.path());

    // Two edges with different 'since' values (variables required in CREATE chains).
    db.execute(
        "CREATE (a:Src1 {name: 'Alice'})-[:KNOWS {since: 2020}]->(b:Dst1 {name: 'Bob'})",
    )
    .expect("create chain1");
    db.execute(
        "CREATE (a:Src2 {name: 'Alice'})-[:KNOWS {since: 2015}]->(b:Dst2 {name: 'Charlie'})",
    )
    .expect("create chain2");

    // Without filter: both edges from any source
    let all = db
        .execute("MATCH (a)-[r:KNOWS]->(b) RETURN r.since")
        .expect("all");
    assert_eq!(all.rows.len(), 2, "unfiltered should return 2 edges");

    // Filter by since: 2020 -- should only return the first edge
    let result = db
        .execute("MATCH (a)-[r:KNOWS {since: 2020}]->(b) RETURN r.since, b.name")
        .expect("query");
    assert_eq!(result.rows.len(), 1, "should return only the edge with since=2020");
    assert_eq!(result.rows[0].get_as::<i64>("r.since"), Some(2020));
    assert_eq!(
        result.rows[0].get_as::<String>("b.name"),
        Some("Bob".to_string())
    );
}

/// Relationship multiple property filter:
/// MATCH (a)-[r:KNOWS {since: 2020, strength: 5}]->(b) should match exact edge.
#[test]
fn inline_filter_rel_multiple_properties() {
    let dir = tempdir().expect("tempdir");
    let mut db = test_db(dir.path());

    db.execute(
        "CREATE (a:M1 {name: 'Alice'})-[:KNOWS {since: 2020, strength: 5}]->(b:M2 {name: 'Bob'})",
    )
    .expect("create chain1");
    db.execute(
        "CREATE (a:M3 {name: 'Alice'})-[:KNOWS {since: 2020, strength: 3}]->(b:M4 {name: 'Charlie'})",
    )
    .expect("create chain2");

    // Filter by both since=2020 AND strength=5 -- should only return one edge
    let result = db
        .execute("MATCH (a)-[r:KNOWS {since: 2020, strength: 5}]->(b) RETURN r.since, r.strength")
        .expect("query");
    assert_eq!(
        result.rows.len(),
        1,
        "should match only the edge with since=2020 AND strength=5"
    );
    assert_eq!(result.rows[0].get_as::<i64>("r.since"), Some(2020));
    assert_eq!(result.rows[0].get_as::<i64>("r.strength"), Some(5));
}

/// Target node property filter:
/// MATCH (a)-[:KNOWS]->(b:Dst {name: 'Bob'}) should filter target node.
#[test]
fn inline_filter_target_node_property() {
    let dir = tempdir().expect("tempdir");
    let mut db = test_db(dir.path());

    db.execute("CREATE (x:Src {name: 'Alice'})-[:KNOWS]->(y:Dst {name: 'Bob'})")
        .expect("create chain1");
    db.execute("CREATE (x:Src {name: 'Alice'})-[:KNOWS]->(y:Dst {name: 'Charlie'})")
        .expect("create chain2");

    // Without filter: two edges
    let all = db
        .execute("MATCH (a:Src)-[:KNOWS]->(b:Dst) RETURN b.name")
        .expect("all");
    assert_eq!(all.rows.len(), 2, "unfiltered should return 2 targets");

    // Filter target node by name -- should only return Bob
    let result = db
        .execute("MATCH (a:Src)-[:KNOWS]->(b:Dst {name: 'Bob'}) RETURN b.name")
        .expect("query");
    assert_eq!(result.rows.len(), 1, "should return only Bob as target");
    assert_eq!(
        result.rows[0].get_as::<String>("b.name"),
        Some("Bob".to_string())
    );
}

/// Combined source + relationship property filter:
/// MATCH (a:S1 {name: 'Alice'})-[r:KNOWS {since: 2020}]->(b) RETURN r.since
#[test]
fn inline_filter_source_and_rel_combined() {
    let dir = tempdir().expect("tempdir");
    let mut db = test_db(dir.path());

    // Alice -> Bob with since=2020
    db.execute(
        "CREATE (a:S1 {name: 'Alice'})-[:KNOWS {since: 2020}]->(b:T1 {name: 'Bob'})",
    )
    .expect("create chain1");
    // Dave -> Bob with since=2020
    db.execute(
        "CREATE (a:S1 {name: 'Dave'})-[:KNOWS {since: 2020}]->(b:T2 {name: 'Bob'})",
    )
    .expect("create chain2");

    // Filter source by name=Alice AND rel by since=2020 -- only Alice path
    let result = db
        .execute(
            "MATCH (a:S1 {name: 'Alice'})-[r:KNOWS {since: 2020}]->(b) RETURN r.since, b.name",
        )
        .expect("query");
    assert_eq!(
        result.rows.len(),
        1,
        "should return only path from Alice with since=2020"
    );
    assert_eq!(result.rows[0].get_as::<i64>("r.since"), Some(2020));
    assert_eq!(
        result.rows[0].get_as::<String>("b.name"),
        Some("Bob".to_string())
    );
}

/// Anonymous relationship with property filter:
/// MATCH (a)-[:KNOWS {since: 2020}]->(b) without explicit variable name.
/// The planner should assign an internal variable so edges are filtered.
#[test]
fn inline_filter_anonymous_rel_with_property() {
    let dir = tempdir().expect("tempdir");
    let mut db = test_db(dir.path());

    db.execute(
        "CREATE (a:A1 {name: 'Alice'})-[:KNOWS {since: 2020}]->(b:B1 {name: 'Bob'})",
    )
    .expect("create chain1");
    db.execute(
        "CREATE (a:A2 {name: 'Alice'})-[:KNOWS {since: 2015}]->(b:B2 {name: 'Charlie'})",
    )
    .expect("create chain2");

    // No relationship variable -- but inline props should still filter
    let result = db
        .execute("MATCH (a)-[:KNOWS {since: 2020}]->(b) RETURN b.name")
        .expect("query");
    assert_eq!(
        result.rows.len(),
        1,
        "anonymous rel with inline props should still filter"
    );
    assert_eq!(
        result.rows[0].get_as::<String>("b.name"),
        Some("Bob".to_string())
    );
}
