// Integration tests for Group V: Timestamp Tracking
//
// V-001: Automatic _created_at on CREATE
// V-002: Automatic _updated_at on SET
// V-003: System property convention (read-only)
// V-004: Timestamp opt-out

use cypherlite_core::{DatabaseConfig, SyncMode};
use cypherlite_query::api::CypherLite;
use cypherlite_query::executor::Value;
use tempfile::tempdir;

fn test_db(dir: &std::path::Path) -> CypherLite {
    let config = DatabaseConfig {
        path: dir.join("test.cyl"),
        wal_sync_mode: SyncMode::Normal,
        ..Default::default()
    };
    CypherLite::open(config).expect("open")
}

fn test_db_no_temporal(dir: &std::path::Path) -> CypherLite {
    let config = DatabaseConfig {
        path: dir.join("test.cyl"),
        wal_sync_mode: SyncMode::Normal,
        temporal_tracking_enabled: false,
        ..Default::default()
    };
    CypherLite::open(config).expect("open")
}

// ======================================================================
// V-001: Automatic _created_at on CREATE
// ======================================================================

#[test]
fn v001_create_node_sets_created_at() {
    let dir = tempdir().expect("tempdir");
    let mut db = test_db(dir.path());

    db.execute("CREATE (n:Person {name: 'Alice'})").expect("create");

    let result = db
        .execute("MATCH (n:Person) RETURN n._created_at")
        .expect("match");
    assert_eq!(result.rows.len(), 1);

    // _created_at should be a DateTime value (not null)
    let val = result.rows[0].get("n._created_at").expect("has _created_at");
    assert!(
        matches!(val, Value::DateTime(ms) if *ms > 0),
        "expected DateTime, got: {:?}",
        val
    );
}

#[test]
fn v001_create_relationship_sets_created_at() {
    let dir = tempdir().expect("tempdir");
    let mut db = test_db(dir.path());

    db.execute("CREATE (a:Person {name: 'Alice'})-[:KNOWS]->(b:Person {name: 'Bob'})")
        .expect("create");

    let result = db
        .execute("MATCH (a:Person)-[r:KNOWS]->(b:Person) RETURN r._created_at")
        .expect("match");
    assert_eq!(result.rows.len(), 1);

    let val = result.rows[0].get("r._created_at").expect("has _created_at");
    assert!(
        matches!(val, Value::DateTime(ms) if *ms > 0),
        "expected DateTime, got: {:?}",
        val
    );
}

#[test]
fn v001_create_sets_updated_at_equal_to_created_at() {
    let dir = tempdir().expect("tempdir");
    let mut db = test_db(dir.path());

    db.execute("CREATE (n:Person {name: 'Alice'})").expect("create");

    let result = db
        .execute("MATCH (n:Person) RETURN n._created_at, n._updated_at")
        .expect("match");
    assert_eq!(result.rows.len(), 1);

    let created = result.rows[0].get("n._created_at").expect("created_at");
    let updated = result.rows[0].get("n._updated_at").expect("updated_at");
    assert_eq!(created, updated, "_created_at should equal _updated_at on CREATE");
}

#[test]
fn v001_merge_on_create_sets_timestamps() {
    let dir = tempdir().expect("tempdir");
    let mut db = test_db(dir.path());

    db.execute("MERGE (n:Person {name: 'Alice'})").expect("merge");

    let result = db
        .execute("MATCH (n:Person) RETURN n._created_at, n._updated_at")
        .expect("match");
    assert_eq!(result.rows.len(), 1);

    let created = result.rows[0].get("n._created_at").expect("created_at");
    assert!(
        matches!(created, Value::DateTime(ms) if *ms > 0),
        "expected DateTime"
    );
}

// ======================================================================
// V-002: Automatic _updated_at on SET
// ======================================================================

#[test]
fn v002_set_updates_updated_at() {
    let dir = tempdir().expect("tempdir");
    let mut db = test_db(dir.path());

    db.execute("CREATE (n:Person {name: 'Alice'})").expect("create");

    // Get initial _updated_at
    let result1 = db
        .execute("MATCH (n:Person) RETURN n._updated_at")
        .expect("match1");
    let initial_updated = result1.rows[0].get("n._updated_at").cloned().expect("updated_at");

    // SET a property
    db.execute("MATCH (n:Person) SET n.age = 30").expect("set");

    let result2 = db
        .execute("MATCH (n:Person) RETURN n._updated_at")
        .expect("match2");
    let new_updated = result2.rows[0].get("n._updated_at").cloned().expect("updated_at");

    // _updated_at should be >= initial (same query start time may equal)
    match (&initial_updated, &new_updated) {
        (Value::DateTime(a), Value::DateTime(b)) => {
            assert!(*b >= *a, "updated_at should not go backwards");
        }
        _ => panic!("expected DateTime values"),
    }
}

#[test]
fn v002_set_does_not_change_created_at() {
    let dir = tempdir().expect("tempdir");
    let mut db = test_db(dir.path());

    db.execute("CREATE (n:Person {name: 'Alice'})").expect("create");

    let result1 = db
        .execute("MATCH (n:Person) RETURN n._created_at")
        .expect("match1");
    let initial_created = result1.rows[0].get("n._created_at").cloned().expect("created_at");

    db.execute("MATCH (n:Person) SET n.age = 30").expect("set");

    let result2 = db
        .execute("MATCH (n:Person) RETURN n._created_at")
        .expect("match2");
    let after_created = result2.rows[0].get("n._created_at").cloned().expect("created_at");

    assert_eq!(initial_created, after_created, "_created_at should not change on SET");
}

#[test]
fn v002_remove_property_updates_updated_at() {
    let dir = tempdir().expect("tempdir");
    let mut db = test_db(dir.path());

    db.execute("CREATE (n:Person {name: 'Alice', age: 30})").expect("create");

    let result1 = db
        .execute("MATCH (n:Person) RETURN n._updated_at")
        .expect("match1");
    let initial_updated = result1.rows[0].get("n._updated_at").cloned().expect("updated_at");

    db.execute("MATCH (n:Person) REMOVE n.age").expect("remove");

    let result2 = db
        .execute("MATCH (n:Person) RETURN n._updated_at")
        .expect("match2");
    let new_updated = result2.rows[0].get("n._updated_at").cloned().expect("updated_at");

    match (&initial_updated, &new_updated) {
        (Value::DateTime(a), Value::DateTime(b)) => {
            assert!(*b >= *a, "updated_at should not go backwards after REMOVE");
        }
        _ => panic!("expected DateTime values"),
    }
}

// ======================================================================
// V-003: System property convention (read-only)
// ======================================================================

#[test]
fn v003_user_set_created_at_fails() {
    let dir = tempdir().expect("tempdir");
    let mut db = test_db(dir.path());

    db.execute("CREATE (n:Person {name: 'Alice'})").expect("create");

    let result = db.execute("MATCH (n:Person) SET n._created_at = 0");
    assert!(result.is_err(), "SET _created_at should fail");

    let err = result.unwrap_err();
    let msg = format!("{}", err);
    assert!(
        msg.contains("read-only") || msg.contains("System property"),
        "error should mention read-only, got: {}",
        msg
    );
}

#[test]
fn v003_user_set_updated_at_fails() {
    let dir = tempdir().expect("tempdir");
    let mut db = test_db(dir.path());

    db.execute("CREATE (n:Person {name: 'Alice'})").expect("create");

    let result = db.execute("MATCH (n:Person) SET n._updated_at = 0");
    assert!(result.is_err(), "SET _updated_at should fail");
}

#[test]
fn v003_create_with_system_property_in_map_fails() {
    let dir = tempdir().expect("tempdir");
    let mut db = test_db(dir.path());

    // Attempt to set _created_at explicitly in CREATE properties
    let result = db.execute("CREATE (n:Person {name: 'Alice', _created_at: 0})");
    assert!(result.is_err(), "CREATE with _created_at should fail");
}

// ======================================================================
// V-004: Timestamp opt-out
// ======================================================================

#[test]
fn v004_no_timestamps_when_disabled() {
    let dir = tempdir().expect("tempdir");
    let mut db = test_db_no_temporal(dir.path());

    db.execute("CREATE (n:Person {name: 'Alice'})").expect("create");

    let result = db
        .execute("MATCH (n:Person) RETURN n._created_at")
        .expect("match");
    assert_eq!(result.rows.len(), 1);

    // With temporal tracking disabled, _created_at should be null
    let val = result.rows[0].get("n._created_at");
    assert!(
        val.is_none() || matches!(val, Some(Value::Null)),
        "expected null when temporal tracking disabled, got: {:?}",
        val
    );
}

#[test]
fn v004_set_allowed_on_system_props_when_disabled() {
    let dir = tempdir().expect("tempdir");
    let mut db = test_db_no_temporal(dir.path());

    db.execute("CREATE (n:Person {name: 'Alice'})").expect("create");

    // When temporal tracking is disabled, setting _created_at should still fail
    // (system property protection is always active)
    let result = db.execute("MATCH (n:Person) SET n._created_at = 0");
    assert!(result.is_err(), "SET _created_at should always fail");
}

// ======================================================================
// V-001 + V-002: Multiple node CREATE
// ======================================================================

#[test]
fn v001_multiple_creates_all_get_timestamps() {
    let dir = tempdir().expect("tempdir");
    let mut db = test_db(dir.path());

    db.execute("CREATE (a:Person {name: 'Alice'})").expect("c1");
    db.execute("CREATE (b:Person {name: 'Bob'})").expect("c2");

    let result = db
        .execute("MATCH (n:Person) RETURN n.name, n._created_at")
        .expect("match");
    assert_eq!(result.rows.len(), 2);

    for row in &result.rows {
        let val = row.get("n._created_at").expect("has _created_at");
        assert!(
            matches!(val, Value::DateTime(ms) if *ms > 0),
            "expected DateTime"
        );
    }
}
