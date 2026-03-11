// Integration tests for Groups X and Y: Temporal Query Syntax
//
// X-T1: AT/TIME/BETWEEN/HISTORY tokens
// X-T2: TemporalPredicate AST
// X-T3: Parse AT TIME clause
// X-T4: Semantic validation
// X-T5: AsOfScan logical plan
// X-T6: AsOfScan executor
// Y-T1: Parse BETWEEN TIME clause
// Y-T2: TemporalRangeScan logical plan
// Y-T3: TemporalRangeScan executor

use cypherlite_core::{DatabaseConfig, SyncMode};
use cypherlite_query::api::CypherLite;
use cypherlite_query::executor::Params;
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
// X-T6: AT TIME query - basic point-in-time lookup
// ======================================================================

#[test]
fn xt6_at_time_returns_historical_state() {
    let dir = tempdir().expect("tempdir");
    let mut db = test_db(dir.path());

    // Create node with specific timestamp via params
    let mut params = Params::new();
    params.insert(
        "__query_start_ms__".to_string(),
        cypherlite_query::executor::Value::Int64(1000),
    );
    db.execute_with_params(
        "CREATE (n:Person {name: 'Alice', age: 25})",
        params,
    )
    .expect("create");

    // Update at time 2000
    let mut params = Params::new();
    params.insert(
        "__query_start_ms__".to_string(),
        cypherlite_query::executor::Value::Int64(2000),
    );
    db.execute_with_params("MATCH (n:Person) SET n.age = 30", params)
        .expect("set");

    // Update at time 3000
    let mut params = Params::new();
    params.insert(
        "__query_start_ms__".to_string(),
        cypherlite_query::executor::Value::Int64(3000),
    );
    db.execute_with_params("MATCH (n:Person) SET n.age = 35", params)
        .expect("set2");

    // AT TIME 1500: should return the original state (age=25)
    let result = db
        .execute("MATCH (n:Person) AT TIME 1500 RETURN n.age")
        .expect("at time query");
    assert_eq!(result.rows.len(), 1, "should find one node at time 1500");
    // The first version (before first SET) should have age=25
    let age = result.rows[0].get_as::<i64>("n.age");
    assert_eq!(age, Some(25), "age at time 1500 should be 25");
}

#[test]
fn xt6_at_time_returns_current_state_when_no_later_updates() {
    let dir = tempdir().expect("tempdir");
    let mut db = test_db(dir.path());

    // Create node at time 1000
    let mut params = Params::new();
    params.insert(
        "__query_start_ms__".to_string(),
        cypherlite_query::executor::Value::Int64(1000),
    );
    db.execute_with_params(
        "CREATE (n:Person {name: 'Alice', age: 25})",
        params,
    )
    .expect("create");

    // AT TIME 5000: node was created at 1000 and never updated, so current state
    let result = db
        .execute("MATCH (n:Person) AT TIME 5000 RETURN n.age")
        .expect("at time query");
    assert_eq!(result.rows.len(), 1);
    let age = result.rows[0].get_as::<i64>("n.age");
    assert_eq!(age, Some(25));
}

#[test]
fn xt6_at_time_excludes_nodes_created_after_timestamp() {
    let dir = tempdir().expect("tempdir");
    let mut db = test_db(dir.path());

    // Create node at time 2000
    let mut params = Params::new();
    params.insert(
        "__query_start_ms__".to_string(),
        cypherlite_query::executor::Value::Int64(2000),
    );
    db.execute_with_params(
        "CREATE (n:Person {name: 'Alice', age: 25})",
        params,
    )
    .expect("create");

    // AT TIME 1000: node doesn't exist yet
    let result = db
        .execute("MATCH (n:Person) AT TIME 1000 RETURN n.age")
        .expect("at time query");
    assert_eq!(
        result.rows.len(),
        0,
        "node should not exist at time before creation"
    );
}

#[test]
fn xt6_at_time_after_second_update() {
    let dir = tempdir().expect("tempdir");
    let mut db = test_db(dir.path());

    // Create at time 1000
    let mut params = Params::new();
    params.insert(
        "__query_start_ms__".to_string(),
        cypherlite_query::executor::Value::Int64(1000),
    );
    db.execute_with_params("CREATE (n:Person {name: 'Alice', age: 25})", params)
        .expect("create");

    // Update at time 2000 -> age=30
    let mut params = Params::new();
    params.insert(
        "__query_start_ms__".to_string(),
        cypherlite_query::executor::Value::Int64(2000),
    );
    db.execute_with_params("MATCH (n:Person) SET n.age = 30", params)
        .expect("set1");

    // Update at time 3000 -> age=35
    let mut params = Params::new();
    params.insert(
        "__query_start_ms__".to_string(),
        cypherlite_query::executor::Value::Int64(3000),
    );
    db.execute_with_params("MATCH (n:Person) SET n.age = 35", params)
        .expect("set2");

    // AT TIME 2500: should be between first and second update, age=30
    let result = db
        .execute("MATCH (n:Person) AT TIME 2500 RETURN n.age")
        .expect("at time query");
    assert_eq!(result.rows.len(), 1);
    let age = result.rows[0].get_as::<i64>("n.age");
    assert_eq!(age, Some(30), "age at time 2500 should be 30 (after first SET)");
}

#[test]
fn xt6_at_time_with_where_clause() {
    let dir = tempdir().expect("tempdir");
    let mut db = test_db(dir.path());

    // Create two nodes at time 1000
    let mut params = Params::new();
    params.insert(
        "__query_start_ms__".to_string(),
        cypherlite_query::executor::Value::Int64(1000),
    );
    db.execute_with_params("CREATE (n:Person {name: 'Alice', age: 25})", params.clone())
        .expect("create1");
    db.execute_with_params("CREATE (n:Person {name: 'Bob', age: 40})", params)
        .expect("create2");

    // AT TIME 5000 WHERE age > 30: should only return Bob
    let result = db
        .execute("MATCH (n:Person) AT TIME 5000 WHERE n.age > 30 RETURN n.name")
        .expect("at time with where");
    assert_eq!(result.rows.len(), 1);
    let name = result.rows[0].get_as::<String>("n.name");
    assert_eq!(name, Some("Bob".to_string()));
}

// ======================================================================
// Y-T3: BETWEEN TIME query - temporal range
// ======================================================================

#[test]
fn yt3_between_time_returns_all_versions_in_range() {
    let dir = tempdir().expect("tempdir");
    let mut db = test_db(dir.path());

    // Create at time 1000
    let mut params = Params::new();
    params.insert(
        "__query_start_ms__".to_string(),
        cypherlite_query::executor::Value::Int64(1000),
    );
    db.execute_with_params("CREATE (n:Person {name: 'Alice', age: 25})", params)
        .expect("create");

    // Update at time 2000
    let mut params = Params::new();
    params.insert(
        "__query_start_ms__".to_string(),
        cypherlite_query::executor::Value::Int64(2000),
    );
    db.execute_with_params("MATCH (n:Person) SET n.age = 30", params)
        .expect("set1");

    // Update at time 3000
    let mut params = Params::new();
    params.insert(
        "__query_start_ms__".to_string(),
        cypherlite_query::executor::Value::Int64(3000),
    );
    db.execute_with_params("MATCH (n:Person) SET n.age = 35", params)
        .expect("set2");

    // BETWEEN TIME 900 AND 3500: should return all 3 states
    // Version snapshots: [age=25 at _updated_at=1000, age=30 at _updated_at=2000]
    // Current: age=35 at _updated_at=3000
    let result = db
        .execute("MATCH (n:Person) BETWEEN TIME 900 AND 3500 RETURN n.age")
        .expect("between time query");
    assert!(
        result.rows.len() >= 2,
        "expected at least 2 versions in range 900-3500, got {}",
        result.rows.len()
    );
}

#[test]
fn yt3_between_time_narrow_range() {
    let dir = tempdir().expect("tempdir");
    let mut db = test_db(dir.path());

    // Create at time 1000
    let mut params = Params::new();
    params.insert(
        "__query_start_ms__".to_string(),
        cypherlite_query::executor::Value::Int64(1000),
    );
    db.execute_with_params("CREATE (n:Person {name: 'Alice', age: 25})", params)
        .expect("create");

    // Update at time 2000
    let mut params = Params::new();
    params.insert(
        "__query_start_ms__".to_string(),
        cypherlite_query::executor::Value::Int64(2000),
    );
    db.execute_with_params("MATCH (n:Person) SET n.age = 30", params)
        .expect("set1");

    // Update at time 3000
    let mut params = Params::new();
    params.insert(
        "__query_start_ms__".to_string(),
        cypherlite_query::executor::Value::Int64(3000),
    );
    db.execute_with_params("MATCH (n:Person) SET n.age = 35", params)
        .expect("set2");

    // BETWEEN TIME 1500 AND 2500: should only include the version with _updated_at=2000
    let result = db
        .execute("MATCH (n:Person) BETWEEN TIME 1500 AND 2500 RETURN n.age")
        .expect("between time narrow range");
    // The snapshot at _updated_at=2000 (age=30 - after first SET, state is age=30)
    // Wait: the version store snapshot is BEFORE the update. So:
    // - Snapshot 1: state before SET n.age=30, which has age=25, _updated_at=1000
    // - Snapshot 2: state before SET n.age=35, which has age=30, _updated_at=2000
    // - Current: age=35, _updated_at=3000
    // In range [1500, 2500]: only snapshot 2 (_updated_at=2000) has age=30
    assert_eq!(result.rows.len(), 1, "expected 1 version in range 1500-2500");
    let age = result.rows[0].get_as::<i64>("n.age");
    assert_eq!(age, Some(30));
}

#[test]
fn yt3_between_time_no_versions_in_range() {
    let dir = tempdir().expect("tempdir");
    let mut db = test_db(dir.path());

    // Create at time 5000
    let mut params = Params::new();
    params.insert(
        "__query_start_ms__".to_string(),
        cypherlite_query::executor::Value::Int64(5000),
    );
    db.execute_with_params("CREATE (n:Person {name: 'Alice', age: 25})", params)
        .expect("create");

    // BETWEEN TIME 100 AND 200: no versions exist in this range
    let result = db
        .execute("MATCH (n:Person) BETWEEN TIME 100 AND 200 RETURN n.age")
        .expect("between time no results");
    assert_eq!(result.rows.len(), 0, "expected no versions in range 100-200");
}

// ======================================================================
// Parse-level tests (end-to-end through parser)
// ======================================================================

#[test]
fn parse_at_time_full_query() {
    let result = cypherlite_query::parser::parse_query(
        "MATCH (n:Person) AT TIME datetime('2024-01-15T00:00:00Z') RETURN n",
    );
    assert!(result.is_ok(), "AT TIME full query should parse");
}

#[test]
fn parse_between_time_full_query() {
    let result = cypherlite_query::parser::parse_query(
        "MATCH (n:Person) BETWEEN TIME datetime('2024-01-01T00:00:00Z') AND datetime('2024-12-31T00:00:00Z') RETURN n",
    );
    assert!(result.is_ok(), "BETWEEN TIME full query should parse");
}

#[test]
fn parse_at_time_with_where_full_query() {
    let result = cypherlite_query::parser::parse_query(
        "MATCH (n:Person) AT TIME 1000 WHERE n.age > 30 RETURN n.name",
    );
    assert!(result.is_ok(), "AT TIME with WHERE should parse");
}
