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
