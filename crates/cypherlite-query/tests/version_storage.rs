// Integration tests for Group W: Version Storage
//
// W-001: VersionStore Module
// W-002: Pre-Update Snapshot
// W-003: Version Chain Structure
// W-004: DatabaseHeader Extension
// W-005: Version Storage Opt-out

use cypherlite_core::{DatabaseConfig, LabelRegistry, SyncMode};
use cypherlite_query::api::CypherLite;
use cypherlite_query::executor::Value;
use cypherlite_storage::version::VersionRecord;
use tempfile::tempdir;

fn test_db(dir: &std::path::Path) -> CypherLite {
    let config = DatabaseConfig {
        path: dir.join("test.cyl"),
        wal_sync_mode: SyncMode::Normal,
        ..Default::default()
    };
    CypherLite::open(config).expect("open")
}

fn test_db_no_versioning(dir: &std::path::Path) -> CypherLite {
    let config = DatabaseConfig {
        path: dir.join("test.cyl"),
        wal_sync_mode: SyncMode::Normal,
        version_storage_enabled: false,
        ..Default::default()
    };
    CypherLite::open(config).expect("open")
}

// ======================================================================
// W-002: Pre-Update Snapshot on SET
// ======================================================================

#[test]
fn w002_set_creates_version_snapshot() {
    let dir = tempdir().expect("tempdir");
    let mut db = test_db(dir.path());

    db.execute("CREATE (n:Person {name: 'Alice', age: 25})")
        .expect("create");

    // SET updates the node, creating a snapshot of the previous state
    db.execute("MATCH (n:Person) SET n.age = 30").expect("set");

    // Check that version store has a snapshot
    let vs = db.engine().version_store();
    // The node ID is 1 (first created node)
    assert!(
        vs.version_count(1) >= 1,
        "expected at least one version snapshot after SET"
    );
}

#[test]
fn w002_multiple_sets_create_multiple_snapshots() {
    let dir = tempdir().expect("tempdir");
    let mut db = test_db(dir.path());

    db.execute("CREATE (n:Person {name: 'Alice', age: 25})")
        .expect("create");

    db.execute("MATCH (n:Person) SET n.age = 30").expect("set1");
    db.execute("MATCH (n:Person) SET n.age = 35").expect("set2");
    db.execute("MATCH (n:Person) SET n.age = 40").expect("set3");

    let vs = db.engine().version_store();
    assert_eq!(
        vs.version_count(1),
        3,
        "expected 3 version snapshots after 3 SETs"
    );
}

// ======================================================================
// W-003: Version Chain Structure
// ======================================================================

#[test]
fn w003_version_chain_preserves_property_history() {
    let dir = tempdir().expect("tempdir");
    let mut db = test_db(dir.path());

    db.execute("CREATE (n:Person {name: 'Alice', age: 25})")
        .expect("create");

    db.execute("MATCH (n:Person) SET n.age = 30").expect("set1");
    db.execute("MATCH (n:Person) SET n.age = 35").expect("set2");

    let vs = db.engine().version_store();
    let chain = vs.get_version_chain(1);
    assert_eq!(chain.len(), 2);

    // First snapshot should have age=25 (before first SET)
    match &chain[0].1 {
        VersionRecord::Node(node) => {
            let age_key = db.engine().catalog().prop_key_id("age").expect("age key");
            let age_val = node.properties.iter().find(|(k, _)| *k == age_key);
            assert!(age_val.is_some(), "first version should have age property");
        }
        _ => panic!("expected node version"),
    }

    // Second snapshot should have age=30 (before second SET)
    match &chain[1].1 {
        VersionRecord::Node(node) => {
            let age_key = db.engine().catalog().prop_key_id("age").expect("age key");
            let age_val = node.properties.iter().find(|(k, _)| *k == age_key);
            assert!(age_val.is_some(), "second version should have age property");
        }
        _ => panic!("expected node version"),
    }
}

#[test]
fn w003_current_state_is_live_not_versioned() {
    let dir = tempdir().expect("tempdir");
    let mut db = test_db(dir.path());

    db.execute("CREATE (n:Person {name: 'Alice', age: 25})")
        .expect("create");

    db.execute("MATCH (n:Person) SET n.age = 30").expect("set");

    // Current state should be age=30 (live in primary store)
    let result = db
        .execute("MATCH (n:Person) RETURN n.age")
        .expect("match");
    assert_eq!(result.rows[0].get_as::<i64>("n.age"), Some(30));

    // Version store should have the old state (age=25)
    let vs = db.engine().version_store();
    let latest = vs.get_latest_version(1).expect("has version");
    match latest {
        VersionRecord::Node(node) => {
            let age_key = db.engine().catalog().prop_key_id("age").expect("age key");
            let age_val = node
                .properties
                .iter()
                .find(|(k, _)| *k == age_key)
                .map(|(_, v)| v.clone());
            assert_eq!(
                age_val,
                Some(cypherlite_core::PropertyValue::Int64(25)),
                "versioned snapshot should have old value"
            );
        }
        _ => panic!("expected node version"),
    }
}

// ======================================================================
// W-002: REMOVE also creates snapshot
// ======================================================================

#[test]
fn w002_remove_creates_version_snapshot() {
    let dir = tempdir().expect("tempdir");
    let mut db = test_db(dir.path());

    db.execute("CREATE (n:Person {name: 'Alice', age: 25})")
        .expect("create");

    db.execute("MATCH (n:Person) REMOVE n.age").expect("remove");

    let vs = db.engine().version_store();
    assert!(
        vs.version_count(1) >= 1,
        "expected at least one version snapshot after REMOVE"
    );
}

// ======================================================================
// W-005: Version Storage Opt-out
// ======================================================================

#[test]
fn w005_no_snapshots_when_disabled() {
    let dir = tempdir().expect("tempdir");
    let mut db = test_db_no_versioning(dir.path());

    db.execute("CREATE (n:Person {name: 'Alice', age: 25})")
        .expect("create");

    db.execute("MATCH (n:Person) SET n.age = 30").expect("set");
    db.execute("MATCH (n:Person) SET n.age = 35").expect("set2");

    let vs = db.engine().version_store();
    assert_eq!(
        vs.total_versions(),
        0,
        "no snapshots should be created when version storage is disabled"
    );
}

// ======================================================================
// W-001: VersionStore accessible from StorageEngine
// ======================================================================

#[test]
fn w001_version_store_accessible() {
    let dir = tempdir().expect("tempdir");
    let db = test_db(dir.path());

    // Should be able to access version store
    let vs = db.engine().version_store();
    assert_eq!(vs.total_versions(), 0);
}
