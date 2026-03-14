// Integration tests for UNWIND clause (Group M, SPEC-DB-003)
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
// TASK-073: UNWIND integration tests
// ======================================================================

/// UNWIND literal list produces one row per element.
#[test]
fn test_unwind_literal_list() {
    let dir = tempdir().expect("tempdir");
    let mut db = open_db(dir.path());

    let result = db
        .execute("UNWIND [1, 2, 3] AS x RETURN x")
        .expect("unwind literal");
    assert_eq!(result.rows.len(), 3);

    let values: Vec<i64> = result
        .rows
        .iter()
        .filter_map(|r| r.get_as::<i64>("x"))
        .collect();
    assert_eq!(values, vec![1, 2, 3]);
}

/// UNWIND empty list produces zero rows.
#[test]
fn test_unwind_empty_list() {
    let dir = tempdir().expect("tempdir");
    let mut db = open_db(dir.path());

    let result = db.execute("UNWIND [] AS x RETURN x").expect("unwind empty");
    assert_eq!(result.rows.len(), 0);
}

/// UNWIND with MATCH: expand list after match.
#[test]
fn test_unwind_after_match() {
    let dir = tempdir().expect("tempdir");
    let mut db = open_db(dir.path());

    db.execute("CREATE (n:Person {name: 'Alice'})")
        .expect("create");

    // UNWIND a literal list after MATCH
    let result = db
        .execute("MATCH (n:Person) UNWIND [10, 20] AS x RETURN n.name, x")
        .expect("unwind after match");
    assert_eq!(result.rows.len(), 2);
}

/// UNWIND combined with WITH pipeline.
#[test]
fn test_unwind_with_pipeline() {
    let dir = tempdir().expect("tempdir");
    let mut db = open_db(dir.path());

    db.execute("CREATE (n:Person {name: 'Alice'})")
        .expect("create");
    db.execute("CREATE (n:Person {name: 'Bob'})")
        .expect("create");

    let result = db
        .execute("MATCH (n:Person) WITH n.name AS name UNWIND [1, 2] AS x RETURN name, x")
        .expect("with + unwind");
    // 2 persons x 2 elements = 4 rows
    assert_eq!(result.rows.len(), 4);
}
