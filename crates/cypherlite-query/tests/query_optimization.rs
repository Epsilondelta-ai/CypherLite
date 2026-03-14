// Integration tests for Query Optimization Rules (Group S: TASK-111 to TASK-117)
//
// Tests verify that optimization rules produce correct results end-to-end
// through the CypherLite API.

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
// Index scan selection (TASK-111, TASK-112)
// ======================================================================

#[test]
fn opt_index_scan_basic_property_eq() {
    let dir = tempdir().expect("tempdir");
    let mut db = test_db(dir.path());

    // Setup: create nodes and index
    db.execute("CREATE (:Person {name: 'Alice', age: 30})")
        .expect("create");
    db.execute("CREATE (:Person {name: 'Bob', age: 25})")
        .expect("create");
    db.execute("CREATE (:Person {name: 'Charlie', age: 35})")
        .expect("create");
    db.execute("CREATE INDEX idx_person_name ON :Person(name)")
        .expect("create index");

    // Query with property equality -> should use IndexScan internally
    let result = db
        .execute("MATCH (n:Person) WHERE n.name = 'Alice' RETURN n.name")
        .expect("query");
    assert_eq!(result.rows.len(), 1);
}

#[test]
fn opt_index_scan_integer_property() {
    let dir = tempdir().expect("tempdir");
    let mut db = test_db(dir.path());

    db.execute("CREATE (:Person {name: 'Alice', age: 30})")
        .expect("create");
    db.execute("CREATE (:Person {name: 'Bob', age: 25})")
        .expect("create");
    db.execute("CREATE INDEX idx_person_age ON :Person(age)")
        .expect("create index");

    let result = db
        .execute("MATCH (n:Person) WHERE n.age = 30 RETURN n.name")
        .expect("query");
    assert_eq!(result.rows.len(), 1);
}

#[test]
fn opt_index_scan_no_match() {
    let dir = tempdir().expect("tempdir");
    let mut db = test_db(dir.path());

    db.execute("CREATE (:Person {name: 'Alice'})")
        .expect("create");
    db.execute("CREATE INDEX idx_person_name ON :Person(name)")
        .expect("create index");

    let result = db
        .execute("MATCH (n:Person) WHERE n.name = 'Unknown' RETURN n")
        .expect("query");
    assert!(result.rows.is_empty());
}

#[test]
fn opt_index_scan_without_index_still_correct() {
    let dir = tempdir().expect("tempdir");
    let mut db = test_db(dir.path());

    db.execute("CREATE (:Person {name: 'Alice'})")
        .expect("create");
    db.execute("CREATE (:Person {name: 'Bob'})")
        .expect("create");
    // No index created

    let result = db
        .execute("MATCH (n:Person) WHERE n.name = 'Bob' RETURN n.name")
        .expect("query");
    assert_eq!(result.rows.len(), 1);
}

#[test]
fn opt_index_scan_with_additional_filter() {
    let dir = tempdir().expect("tempdir");
    let mut db = test_db(dir.path());

    db.execute("CREATE (:Person {name: 'Alice', age: 30})")
        .expect("create");
    db.execute("CREATE (:Person {name: 'Alice', age: 25})")
        .expect("create");
    db.execute("CREATE (:Person {name: 'Bob', age: 30})")
        .expect("create");
    db.execute("CREATE INDEX idx_person_name ON :Person(name)")
        .expect("create index");

    // Filter: n.name = 'Alice' AND n.age > 28
    // IndexScan on name, then filter on age
    let result = db
        .execute("MATCH (n:Person) WHERE n.name = 'Alice' AND n.age > 28 RETURN n.age")
        .expect("query");
    assert_eq!(result.rows.len(), 1);
}

// ======================================================================
// LIMIT pushdown (TASK-114)
// ======================================================================

#[test]
fn opt_limit_basic() {
    let dir = tempdir().expect("tempdir");
    let mut db = test_db(dir.path());

    for i in 0..10 {
        db.execute(&format!("CREATE (:Item {{num: {}}})", i))
            .expect("create");
    }

    let result = db
        .execute("MATCH (n:Item) RETURN n LIMIT 3")
        .expect("query");
    assert_eq!(result.rows.len(), 3);
}

#[test]
fn opt_limit_zero() {
    let dir = tempdir().expect("tempdir");
    let mut db = test_db(dir.path());

    db.execute("CREATE (:Item {num: 1})").expect("create");
    db.execute("CREATE (:Item {num: 2})").expect("create");

    let result = db
        .execute("MATCH (n:Item) RETURN n LIMIT 0")
        .expect("query");
    assert!(result.rows.is_empty());
}

#[test]
fn opt_limit_larger_than_data() {
    let dir = tempdir().expect("tempdir");
    let mut db = test_db(dir.path());

    db.execute("CREATE (:Item {num: 1})").expect("create");
    db.execute("CREATE (:Item {num: 2})").expect("create");

    let result = db
        .execute("MATCH (n:Item) RETURN n LIMIT 100")
        .expect("query");
    assert_eq!(result.rows.len(), 2);
}

// ======================================================================
// Constant folding (TASK-115)
// ======================================================================

#[test]
fn opt_constant_fold_arithmetic_in_return() {
    let dir = tempdir().expect("tempdir");
    let mut db = test_db(dir.path());

    db.execute("CREATE (:Num {val: 10})").expect("create");

    // 1 + 2 should be folded to 3 at optimization time
    let result = db.execute("MATCH (n:Num) RETURN 1 + 2").expect("query");
    assert_eq!(result.rows.len(), 1);
}

#[test]
fn opt_constant_fold_where_true() {
    let dir = tempdir().expect("tempdir");
    let mut db = test_db(dir.path());

    db.execute("CREATE (:X {val: 1})").expect("create");
    db.execute("CREATE (:X {val: 2})").expect("create");

    // WHERE true should be eliminated
    let result = db
        .execute("MATCH (n:X) WHERE true RETURN n")
        .expect("query");
    assert_eq!(result.rows.len(), 2);
}

#[test]
fn opt_constant_fold_where_false() {
    let dir = tempdir().expect("tempdir");
    let mut db = test_db(dir.path());

    db.execute("CREATE (:X {val: 1})").expect("create");

    // WHERE false should eliminate all rows
    let result = db
        .execute("MATCH (n:X) WHERE false RETURN n")
        .expect("query");
    assert!(result.rows.is_empty());
}

// ======================================================================
// Combined optimization tests
// ======================================================================

#[test]
fn opt_combined_index_and_limit() {
    let dir = tempdir().expect("tempdir");
    let mut db = test_db(dir.path());

    for i in 0..5 {
        db.execute(&format!("CREATE (:Person {{name: 'Alice', idx: {}}})", i))
            .expect("create");
    }
    db.execute("CREATE INDEX idx_person_name ON :Person(name)")
        .expect("create index");

    let result = db
        .execute("MATCH (n:Person) WHERE n.name = 'Alice' RETURN n LIMIT 2")
        .expect("query");
    assert_eq!(result.rows.len(), 2);
}

#[test]
fn opt_combined_filter_and_limit() {
    let dir = tempdir().expect("tempdir");
    let mut db = test_db(dir.path());

    for i in 0..10 {
        db.execute(&format!("CREATE (:Item {{val: {}}})", i))
            .expect("create");
    }

    let result = db
        .execute("MATCH (n:Item) WHERE n.val > 5 RETURN n LIMIT 2")
        .expect("query");
    assert_eq!(result.rows.len(), 2);
}

#[test]
fn opt_combined_constant_fold_and_filter() {
    let dir = tempdir().expect("tempdir");
    let mut db = test_db(dir.path());

    db.execute("CREATE (:X {val: 3})").expect("create");
    db.execute("CREATE (:X {val: 5})").expect("create");

    // 1 + 2 folds to 3, then n.val = 3 matches one node
    let result = db
        .execute("MATCH (n:X) WHERE n.val = 1 + 2 RETURN n.val")
        .expect("query");
    assert_eq!(result.rows.len(), 1);
}

// ======================================================================
// MERGE with index (TASK-113)
// ======================================================================

#[test]
fn opt_merge_with_index_finds_existing() {
    let dir = tempdir().expect("tempdir");
    let mut db = test_db(dir.path());

    db.execute("CREATE (:Person {name: 'Alice'})")
        .expect("create");
    db.execute("CREATE INDEX idx_person_name ON :Person(name)")
        .expect("create index");

    // MERGE should find existing node via index
    db.execute("MERGE (:Person {name: 'Alice'})")
        .expect("merge");
    assert_eq!(db.engine().node_count(), 1); // Still 1
}

#[test]
fn opt_merge_with_index_creates_when_missing() {
    let dir = tempdir().expect("tempdir");
    let mut db = test_db(dir.path());

    db.execute("CREATE (:Person {name: 'Alice'})")
        .expect("create");
    db.execute("CREATE INDEX idx_person_name ON :Person(name)")
        .expect("create index");

    // MERGE Bob -> should create
    db.execute("MERGE (:Person {name: 'Bob'})").expect("merge");
    assert_eq!(db.engine().node_count(), 2);
}

#[test]
fn opt_merge_without_index_same_result() {
    let dir = tempdir().expect("tempdir");
    let mut db = test_db(dir.path());

    db.execute("CREATE (:Person {name: 'Alice'})")
        .expect("create");
    // No index

    db.execute("MERGE (:Person {name: 'Alice'})")
        .expect("merge");
    assert_eq!(db.engine().node_count(), 1); // Found without index
}
