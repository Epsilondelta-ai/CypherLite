// Integration tests for WITH clause (Group L, SPEC-DB-003)
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
// TASK-065: MATCH...WITH...RETURN pipeline
// ======================================================================

/// Basic WITH passthrough: MATCH (n:Person) WITH n RETURN n.name
#[test]
fn test_with_passthrough_match_with_return() {
    let dir = tempdir().expect("tempdir");
    let mut db = open_db(dir.path());

    db.execute("CREATE (n:Person {name: 'Alice', age: 30})")
        .expect("create");
    db.execute("CREATE (n:Person {name: 'Bob', age: 25})")
        .expect("create");

    let result = db
        .execute("MATCH (n:Person) WITH n RETURN n.name")
        .expect("with passthrough");
    assert_eq!(result.rows.len(), 2);

    let mut names: Vec<String> = result
        .rows
        .iter()
        .filter_map(|r| r.get_as::<String>("n.name"))
        .collect();
    names.sort();
    assert_eq!(names, vec!["Alice", "Bob"]);
}

/// WITH alias: MATCH (n:Person) WITH n.name AS name RETURN name
#[test]
fn test_with_alias_projection() {
    let dir = tempdir().expect("tempdir");
    let mut db = open_db(dir.path());

    db.execute("CREATE (n:Person {name: 'Alice'})").expect("create");
    db.execute("CREATE (n:Person {name: 'Bob'})").expect("create");

    let result = db
        .execute("MATCH (n:Person) WITH n.name AS name RETURN name")
        .expect("with alias");
    assert_eq!(result.rows.len(), 2);

    let mut names: Vec<String> = result
        .rows
        .iter()
        .filter_map(|r| r.get_as::<String>("name"))
        .collect();
    names.sort();
    assert_eq!(names, vec!["Alice", "Bob"]);
}

/// WITH multiple items: MATCH (n:Person) WITH n.name AS name, n.age AS age RETURN name, age
#[test]
fn test_with_multiple_items() {
    let dir = tempdir().expect("tempdir");
    let mut db = open_db(dir.path());

    db.execute("CREATE (n:Person {name: 'Alice', age: 30})")
        .expect("create");

    let result = db
        .execute("MATCH (n:Person) WITH n.name AS name, n.age AS age RETURN name, age")
        .expect("with multiple items");
    assert_eq!(result.rows.len(), 1);
    assert_eq!(
        result.rows[0].get_as::<String>("name"),
        Some("Alice".to_string())
    );
    assert_eq!(result.rows[0].get_as::<i64>("age"), Some(30));
}

// ======================================================================
// TASK-065: WITH WHERE filtering
// ======================================================================

/// WITH WHERE: MATCH (n:Person) WITH n WHERE n.age > 28 RETURN n.name
#[test]
fn test_with_where_filtering() {
    let dir = tempdir().expect("tempdir");
    let mut db = open_db(dir.path());

    db.execute("CREATE (n:Person {name: 'Alice', age: 30})")
        .expect("create");
    db.execute("CREATE (n:Person {name: 'Bob', age: 25})")
        .expect("create");
    db.execute("CREATE (n:Person {name: 'Charlie', age: 35})")
        .expect("create");

    let result = db
        .execute("MATCH (n:Person) WITH n WHERE n.age > 28 RETURN n.name")
        .expect("with where");
    assert_eq!(result.rows.len(), 2);

    let mut names: Vec<String> = result
        .rows
        .iter()
        .filter_map(|r| r.get_as::<String>("n.name"))
        .collect();
    names.sort();
    assert_eq!(names, vec!["Alice", "Charlie"]);
}

/// WITH alias + WHERE on alias: MATCH (n:Person) WITH n.age AS a WHERE a > 28 RETURN a
#[test]
fn test_with_alias_where_filtering() {
    let dir = tempdir().expect("tempdir");
    let mut db = open_db(dir.path());

    db.execute("CREATE (n:Person {name: 'Alice', age: 30})")
        .expect("create");
    db.execute("CREATE (n:Person {name: 'Bob', age: 25})")
        .expect("create");

    let result = db
        .execute("MATCH (n:Person) WITH n.age AS a WHERE a > 28 RETURN a")
        .expect("with alias where");
    assert_eq!(result.rows.len(), 1);
    assert_eq!(result.rows[0].get_as::<i64>("a"), Some(30));
}

// ======================================================================
// TASK-065: WITH DISTINCT
// ======================================================================

/// WITH DISTINCT: removes duplicate rows
#[test]
fn test_with_distinct() {
    let dir = tempdir().expect("tempdir");
    let mut db = open_db(dir.path());

    // Create multiple Person nodes with overlapping ages
    db.execute("CREATE (n:Person {name: 'Alice', age: 30})")
        .expect("create");
    db.execute("CREATE (n:Person {name: 'Bob', age: 30})")
        .expect("create");
    db.execute("CREATE (n:Person {name: 'Charlie', age: 25})")
        .expect("create");

    let result = db
        .execute("MATCH (n:Person) WITH DISTINCT n.age AS age RETURN age")
        .expect("with distinct");
    assert_eq!(result.rows.len(), 2);

    let mut ages: Vec<i64> = result
        .rows
        .iter()
        .filter_map(|r| r.get_as::<i64>("age"))
        .collect();
    ages.sort();
    assert_eq!(ages, vec![25, 30]);
}

// ======================================================================
// TASK-065: WITH + aggregation
// ======================================================================

/// WITH count(*): MATCH (n:Person) WITH count(*) AS total RETURN total
#[test]
fn test_with_count_star_aggregation() {
    let dir = tempdir().expect("tempdir");
    let mut db = open_db(dir.path());

    db.execute("CREATE (n:Person {name: 'Alice'})").expect("c1");
    db.execute("CREATE (n:Person {name: 'Bob'})").expect("c2");
    db.execute("CREATE (n:Person {name: 'Charlie'})").expect("c3");

    let result = db
        .execute("MATCH (n:Person) WITH count(*) AS total RETURN total")
        .expect("with count(*)");
    assert_eq!(result.rows.len(), 1);
    assert_eq!(result.rows[0].get_as::<i64>("total"), Some(3));
}

// ======================================================================
// TASK-065: Variable scope error after WITH
// ======================================================================

/// Referencing unprojected variable after WITH should fail with SemanticError
#[test]
fn test_with_scope_error_unprojected_variable() {
    let dir = tempdir().expect("tempdir");
    let mut db = open_db(dir.path());

    // MATCH (n:Person)-[r:KNOWS]->(m:Person) WITH n RETURN m
    // 'm' is not projected in WITH, so it should be inaccessible
    let result = db.execute(
        "MATCH (n:Person)-[r:KNOWS]->(m:Person) WITH n RETURN m",
    );
    assert!(result.is_err());
    let err = result.expect_err("should fail");
    match err {
        cypherlite_core::CypherLiteError::SemanticError(msg) => {
            assert!(
                msg.contains("undefined variable 'm'"),
                "expected undefined variable error, got: {}",
                msg
            );
        }
        other => panic!("expected SemanticError, got: {other}"),
    }
}

/// Referencing original variable when aliased in WITH should fail
#[test]
fn test_with_scope_error_aliased_variable() {
    let dir = tempdir().expect("tempdir");
    let mut db = open_db(dir.path());

    // MATCH (n:Person) WITH n.name AS name RETURN n
    // 'n' is not projected in WITH (only 'name' alias is), so it should fail
    let result = db.execute("MATCH (n:Person) WITH n.name AS name RETURN n");
    assert!(result.is_err());
    let err = result.expect_err("should fail");
    match err {
        cypherlite_core::CypherLiteError::SemanticError(msg) => {
            assert!(
                msg.contains("undefined variable 'n'"),
                "expected undefined variable error, got: {}",
                msg
            );
        }
        other => panic!("expected SemanticError, got: {other}"),
    }
}
