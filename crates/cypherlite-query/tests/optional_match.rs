// Integration tests for OPTIONAL MATCH (Group N, SPEC-DB-003)
//
// Tests the full pipeline: parse -> semantic -> plan -> execute

use cypherlite_core::{DatabaseConfig, SyncMode};
use cypherlite_query::api::CypherLite;
use cypherlite_query::executor::Value;
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
// TASK-078: OPTIONAL MATCH integration tests
// ======================================================================

/// OPTIONAL MATCH with no matching edges produces NULL for optional variables.
#[test]
fn test_optional_match_null_when_no_edges() {
    let dir = tempdir().expect("tempdir");
    let mut db = open_db(dir.path());

    // Create Alice->Bob edge in a single CREATE pattern
    db.execute("CREATE (a:Person {name: 'Alice'})-[:KNOWS]->(b:Person {name: 'Bob'})")
        .expect("create edge");
    // Carol has no outgoing KNOWS edges
    db.execute("CREATE (c:Person {name: 'Carol'})")
        .expect("create carol");

    // OPTIONAL MATCH: Alice has outgoing KNOWS (->Bob), Bob and Carol do not
    let result = db
        .execute("MATCH (a:Person) OPTIONAL MATCH (a)-[:KNOWS]->(b) RETURN a.name, b.name")
        .expect("optional match");

    // Alice -> Bob (1 match), Bob -> NULL, Carol -> NULL
    assert_eq!(result.rows.len(), 3);

    let mut found_alice_bob = false;
    let mut found_null_count = 0;

    for row in &result.rows {
        let a_name = row.get_as::<String>("a.name");
        let b_name_val = row.get("b.name");

        match (a_name.as_deref(), b_name_val) {
            (Some("Alice"), Some(Value::String(b))) if b == "Bob" => {
                found_alice_bob = true;
            }
            (_, Some(Value::Null)) => {
                found_null_count += 1;
            }
            _ => {}
        }
    }

    assert!(found_alice_bob, "Should find Alice->Bob match. Rows: {:?}",
        result.rows.iter().map(|r| format!("a.name={:?} b.name={:?}", r.get("a.name"), r.get("b.name"))).collect::<Vec<_>>());
    assert_eq!(found_null_count, 2, "Bob and Carol should have NULL b.name");
}

/// OPTIONAL MATCH with multiple matches produces multiple rows.
#[test]
fn test_optional_match_multiple_matches() {
    let dir = tempdir().expect("tempdir");
    let mut db = open_db(dir.path());

    // Create Alice with two KNOWS edges
    db.execute("CREATE (a:Person {name: 'Alice'})-[:KNOWS]->(b:Person {name: 'Bob'})")
        .expect("edge1");
    // Add second edge from the same Alice node
    db.execute("MATCH (a:Person {name: 'Alice'}) CREATE (a)-[:KNOWS]->(c:Person {name: 'Carol'})")
        .expect("edge2");

    // Alice should have 2 outgoing KNOWS edges (Bob and Carol)
    let result = db
        .execute(
            "MATCH (a:Person {name: 'Alice'}) OPTIONAL MATCH (a)-[:KNOWS]->(b) RETURN b.name",
        )
        .expect("optional match multi");

    // Filter to only Alice results: should find exactly 2 matches
    assert!(result.rows.len() >= 2, "Expected at least 2 rows, got {}", result.rows.len());
    let b_names: Vec<Option<String>> = result
        .rows
        .iter()
        .map(|r| r.get_as::<String>("b.name"))
        .collect();
    assert!(b_names.contains(&Some("Bob".to_string())), "Should contain Bob");
    assert!(b_names.contains(&Some("Carol".to_string())), "Should contain Carol");
}

/// OPTIONAL MATCH on non-existent relationship type produces NULL.
#[test]
fn test_optional_match_nonexistent_rel_type() {
    let dir = tempdir().expect("tempdir");
    let mut db = open_db(dir.path());

    db.execute("CREATE (a:Person {name: 'Alice'})")
        .expect("create");

    let result = db
        .execute("MATCH (a:Person) OPTIONAL MATCH (a)-[:WORKS_AT]->(c) RETURN a.name, c")
        .expect("optional match no rel type");

    assert_eq!(result.rows.len(), 1);
    let c_val = result.rows[0].get("c");
    assert!(
        matches!(c_val, Some(Value::Null)),
        "c should be NULL when no WORKS_AT relationship exists, got {:?}",
        c_val
    );
}
