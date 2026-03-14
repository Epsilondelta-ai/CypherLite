// Integration tests for Property Index DDL (TASK-100)
//
// Tests CREATE INDEX, DROP INDEX, index-assisted property lookup,
// auto-update on CREATE/SET/DELETE, and backfill on existing data.

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
// INDEX-T001: CREATE INDEX creates an index
// ======================================================================

#[test]
fn index_t001_create_index() {
    let dir = tempdir().expect("tempdir");
    let mut db = test_db(dir.path());

    db.execute("CREATE INDEX idx_person_name ON :Person(name)")
        .expect("create index");

    // Verify the index exists in the catalog
    let defs = db.engine().catalog().index_definitions();
    assert_eq!(defs.len(), 1);
    assert_eq!(defs[0].name, "idx_person_name");
}

#[test]
fn index_t001_create_index_without_name() {
    let dir = tempdir().expect("tempdir");
    let mut db = test_db(dir.path());

    db.execute("CREATE INDEX ON :Person(name)")
        .expect("create index");

    let defs = db.engine().catalog().index_definitions();
    assert_eq!(defs.len(), 1);
    assert_eq!(defs[0].name, "idx_Person_name");
}

#[test]
fn index_t001_create_duplicate_index_fails() {
    let dir = tempdir().expect("tempdir");
    let mut db = test_db(dir.path());

    db.execute("CREATE INDEX idx_test ON :Person(name)")
        .expect("create first");
    let result = db.execute("CREATE INDEX idx_test ON :Person(name)");
    assert!(result.is_err(), "duplicate index name should fail");
}

// ======================================================================
// INDEX-T002: DROP INDEX removes an index
// ======================================================================

#[test]
fn index_t002_drop_index() {
    let dir = tempdir().expect("tempdir");
    let mut db = test_db(dir.path());

    db.execute("CREATE INDEX idx_person_name ON :Person(name)")
        .expect("create index");
    db.execute("DROP INDEX idx_person_name")
        .expect("drop index");

    let defs = db.engine().catalog().index_definitions();
    assert!(defs.is_empty());
}

#[test]
fn index_t002_drop_nonexistent_fails() {
    let dir = tempdir().expect("tempdir");
    let mut db = test_db(dir.path());

    let result = db.execute("DROP INDEX nonexistent");
    assert!(result.is_err(), "dropping nonexistent index should fail");
}

// ======================================================================
// INDEX-T003: Index-assisted property lookup
// ======================================================================

#[test]
fn index_t003_property_lookup_with_index() {
    let dir = tempdir().expect("tempdir");
    let mut db = test_db(dir.path());

    // Create index first
    db.execute("CREATE INDEX idx_person_name ON :Person(name)")
        .expect("create index");

    // Create nodes (should be auto-indexed)
    db.execute("CREATE (n:Person {name: 'Alice', age: 30})")
        .expect("c1");
    db.execute("CREATE (n:Person {name: 'Bob', age: 25})")
        .expect("c2");
    db.execute("CREATE (n:Person {name: 'Charlie', age: 35})")
        .expect("c3");

    // Query should work (via index or scan -- both should produce same result)
    let result = db
        .execute("MATCH (n:Person) WHERE n.name = 'Alice' RETURN n.name")
        .expect("match");
    assert_eq!(result.rows.len(), 1);
    assert_eq!(
        result.rows[0].get_as::<String>("n.name"),
        Some("Alice".to_string())
    );
}

// ======================================================================
// INDEX-T004: Auto-update on CREATE node, SET property, DELETE node
// ======================================================================

#[test]
fn index_t004_auto_update_on_create() {
    let dir = tempdir().expect("tempdir");
    let mut db = test_db(dir.path());

    db.execute("CREATE INDEX idx_person_name ON :Person(name)")
        .expect("create index");

    db.execute("CREATE (n:Person {name: 'Alice'})")
        .expect("create node");

    // Verify index contains the node via the storage engine API
    use cypherlite_core::{LabelRegistry, PropertyValue};
    let label_id = db.engine().label_id("Person").expect("label");
    let prop_id = db.engine().prop_key_id("name").expect("prop");
    let result = db.engine().scan_nodes_by_property(
        label_id,
        prop_id,
        &PropertyValue::String("Alice".into()),
    );
    assert_eq!(result.len(), 1);
}

#[test]
fn index_t004_auto_update_on_set() {
    let dir = tempdir().expect("tempdir");
    let mut db = test_db(dir.path());

    db.execute("CREATE INDEX idx_person_name ON :Person(name)")
        .expect("create index");

    db.execute("CREATE (n:Person {name: 'Alice'})")
        .expect("create");
    db.execute("MATCH (n:Person) SET n.name = 'Alicia'")
        .expect("set");

    use cypherlite_core::{LabelRegistry, PropertyValue};
    let label_id = db.engine().label_id("Person").expect("label");
    let prop_id = db.engine().prop_key_id("name").expect("prop");

    // Old value should not be in index
    let old = db.engine().scan_nodes_by_property(
        label_id,
        prop_id,
        &PropertyValue::String("Alice".into()),
    );
    assert!(old.is_empty(), "old value should be removed from index");

    // New value should be in index
    let new = db.engine().scan_nodes_by_property(
        label_id,
        prop_id,
        &PropertyValue::String("Alicia".into()),
    );
    assert_eq!(new.len(), 1, "new value should be in index");
}

#[test]
fn index_t004_auto_update_on_delete() {
    let dir = tempdir().expect("tempdir");
    let mut db = test_db(dir.path());

    db.execute("CREATE INDEX idx_person_name ON :Person(name)")
        .expect("create index");

    db.execute("CREATE (n:Person {name: 'Alice'})")
        .expect("create");
    db.execute("MATCH (n:Person) DETACH DELETE n")
        .expect("delete");

    use cypherlite_core::{LabelRegistry, PropertyValue};
    let label_id = db.engine().label_id("Person").expect("label");
    let prop_id = db.engine().prop_key_id("name").expect("prop");
    let result = db.engine().scan_nodes_by_property(
        label_id,
        prop_id,
        &PropertyValue::String("Alice".into()),
    );
    assert!(
        result.is_empty(),
        "deleted node should be removed from index"
    );
}

// ======================================================================
// INDEX-T005: CREATE INDEX on existing data backfills the index
// ======================================================================

#[test]
fn index_t005_backfill_existing_data() {
    let dir = tempdir().expect("tempdir");
    let mut db = test_db(dir.path());

    // Create nodes BEFORE creating the index
    db.execute("CREATE (n:Person {name: 'Alice'})").expect("c1");
    db.execute("CREATE (n:Person {name: 'Bob'})").expect("c2");
    db.execute("CREATE (n:Person {name: 'Charlie'})")
        .expect("c3");

    // Now create the index -- should backfill existing data
    db.execute("CREATE INDEX idx_person_name ON :Person(name)")
        .expect("create index");

    // Verify backfill via storage engine API
    use cypherlite_core::{LabelRegistry, PropertyValue};
    let label_id = db.engine().label_id("Person").expect("label");
    let prop_id = db.engine().prop_key_id("name").expect("prop");

    let alice = db.engine().scan_nodes_by_property(
        label_id,
        prop_id,
        &PropertyValue::String("Alice".into()),
    );
    assert_eq!(alice.len(), 1, "Alice should be in the backfilled index");

    let bob =
        db.engine()
            .scan_nodes_by_property(label_id, prop_id, &PropertyValue::String("Bob".into()));
    assert_eq!(bob.len(), 1, "Bob should be in the backfilled index");

    let charlie = db.engine().scan_nodes_by_property(
        label_id,
        prop_id,
        &PropertyValue::String("Charlie".into()),
    );
    assert_eq!(
        charlie.len(),
        1,
        "Charlie should be in the backfilled index"
    );
}

// ======================================================================
// INDEX-T006: End-to-end -- index survives multiple operations
// ======================================================================

#[test]
fn index_t006_end_to_end_mixed_operations() {
    let dir = tempdir().expect("tempdir");
    let mut db = test_db(dir.path());

    // Create index
    db.execute("CREATE INDEX idx_person_name ON :Person(name)")
        .expect("create index");

    // Create nodes
    db.execute("CREATE (n:Person {name: 'Alice', age: 30})")
        .expect("c1");
    db.execute("CREATE (n:Person {name: 'Bob', age: 25})")
        .expect("c2");

    // Query
    let r1 = db
        .execute("MATCH (n:Person) WHERE n.name = 'Alice' RETURN n.age")
        .expect("q1");
    assert_eq!(r1.rows.len(), 1);
    assert_eq!(r1.rows[0].get_as::<i64>("n.age"), Some(30));

    // Update
    db.execute("MATCH (n:Person) WHERE n.name = 'Alice' SET n.name = 'Alicia'")
        .expect("update");

    // Old name should return nothing
    let r2 = db
        .execute("MATCH (n:Person) WHERE n.name = 'Alice' RETURN n")
        .expect("q2");
    assert!(r2.rows.is_empty());

    // New name should work
    let r3 = db
        .execute("MATCH (n:Person) WHERE n.name = 'Alicia' RETURN n.age")
        .expect("q3");
    assert_eq!(r3.rows.len(), 1);
    assert_eq!(r3.rows[0].get_as::<i64>("n.age"), Some(30));

    // Delete
    db.execute("MATCH (n:Person) WHERE n.name = 'Bob' DETACH DELETE n")
        .expect("delete");

    let r4 = db
        .execute("MATCH (n:Person) WHERE n.name = 'Bob' RETURN n")
        .expect("q4");
    assert!(r4.rows.is_empty());

    // Drop index
    db.execute("DROP INDEX idx_person_name")
        .expect("drop index");

    // Queries still work (via linear scan)
    let r5 = db
        .execute("MATCH (n:Person) WHERE n.name = 'Alicia' RETURN n.age")
        .expect("q5");
    assert_eq!(r5.rows.len(), 1);
}
