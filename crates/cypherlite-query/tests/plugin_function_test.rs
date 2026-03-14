//! Integration tests for the ScalarFunction plugin system (Phase 10b).
//!
//! All tests are gated behind `#[cfg(feature = "plugin")]`.

#![cfg(feature = "plugin")]

use cypherlite_core::error::CypherLiteError;
use cypherlite_core::plugin::{Plugin, ScalarFunction};
use cypherlite_core::types::PropertyValue;
use cypherlite_query::api::CypherLite;
use cypherlite_query::executor::Value;

use tempfile::tempdir;

// ---------------------------------------------------------------------------
// Test helpers: sample ScalarFunction implementations
// ---------------------------------------------------------------------------

/// A scalar function that doubles an integer argument.
struct DoubleFunction;

impl Plugin for DoubleFunction {
    fn name(&self) -> &str {
        "double"
    }
    fn version(&self) -> &str {
        "1.0.0"
    }
}

impl ScalarFunction for DoubleFunction {
    fn call(&self, args: &[PropertyValue]) -> Result<PropertyValue, CypherLiteError> {
        if args.len() != 1 {
            return Err(CypherLiteError::PluginError(
                "double() requires exactly one argument".to_string(),
            ));
        }
        match &args[0] {
            PropertyValue::Int64(n) => Ok(PropertyValue::Int64(n * 2)),
            _ => Err(CypherLiteError::PluginError(
                "double() requires an integer argument".to_string(),
            )),
        }
    }
}

/// A scalar function that uppercases a string argument.
struct MyUpperFunction;

impl Plugin for MyUpperFunction {
    fn name(&self) -> &str {
        "my_upper"
    }
    fn version(&self) -> &str {
        "1.0.0"
    }
}

impl ScalarFunction for MyUpperFunction {
    fn call(&self, args: &[PropertyValue]) -> Result<PropertyValue, CypherLiteError> {
        if args.len() != 1 {
            return Err(CypherLiteError::PluginError(
                "my_upper() requires exactly one argument".to_string(),
            ));
        }
        match &args[0] {
            PropertyValue::String(s) => Ok(PropertyValue::String(s.to_uppercase())),
            _ => Err(CypherLiteError::PluginError(
                "my_upper() requires a string argument".to_string(),
            )),
        }
    }
}

fn test_config(dir: &std::path::Path) -> cypherlite_core::DatabaseConfig {
    cypherlite_core::DatabaseConfig {
        path: dir.join("test.cyl"),
        wal_sync_mode: cypherlite_core::SyncMode::Normal,
        ..Default::default()
    }
}

// ---------------------------------------------------------------------------
// TASK-007: ScalarFunction integration tests
// ---------------------------------------------------------------------------

/// P10B-001: Register a "double" ScalarFunction and call it from Cypher.
#[test]
fn test_plugin_scalar_double() {
    let dir = tempdir().expect("tempdir");
    let mut db = CypherLite::open(test_config(dir.path())).expect("open");

    db.register_scalar_function(Box::new(DoubleFunction))
        .expect("register");

    let result = db
        .execute("UNWIND [21] AS x RETURN double(x) AS val")
        .expect("execute");
    assert_eq!(result.rows.len(), 1);
    assert_eq!(result.rows[0].get("val"), Some(&Value::Int64(42)));
}

/// P10B-002: Register "my_upper", create a node, query with the plugin function.
#[test]
fn test_plugin_scalar_my_upper() {
    let dir = tempdir().expect("tempdir");
    let mut db = CypherLite::open(test_config(dir.path())).expect("open");

    db.register_scalar_function(Box::new(MyUpperFunction))
        .expect("register");

    db.execute("CREATE (n:Person {name: 'alice'})")
        .expect("create");

    let result = db
        .execute("MATCH (n:Person) RETURN my_upper(n.name) AS upper_name")
        .expect("execute");
    assert_eq!(result.rows.len(), 1);
    assert_eq!(
        result.rows[0].get("upper_name"),
        Some(&Value::String("ALICE".to_string()))
    );
}

/// P10B-003: Built-in functions still work when plugin feature is enabled.
#[test]
fn test_builtin_functions_still_work_with_plugin() {
    let dir = tempdir().expect("tempdir");
    let mut db = CypherLite::open(test_config(dir.path())).expect("open");

    db.execute("CREATE (n:Person {name: 'Alice'})-[:KNOWS]->(m:Person {name: 'Bob'})")
        .expect("create");

    // id() function
    let result = db
        .execute("MATCH (n:Person) RETURN id(n) AS nid")
        .expect("id query");
    assert_eq!(result.rows.len(), 2);

    // type() function
    let result = db
        .execute("MATCH ()-[r:KNOWS]->() RETURN type(r) AS rel_type")
        .expect("type query");
    assert_eq!(result.rows.len(), 1);
    assert_eq!(
        result.rows[0].get("rel_type"),
        Some(&Value::String("KNOWS".to_string()))
    );

    // labels() function
    let result = db
        .execute("MATCH (n:Person) RETURN labels(n) AS lbls")
        .expect("labels query");
    assert_eq!(result.rows.len(), 2);
}

/// P10B-004: Calling an unregistered function returns an error.
#[test]
fn test_unregistered_function_error() {
    let dir = tempdir().expect("tempdir");
    let mut db = CypherLite::open(test_config(dir.path())).expect("open");

    let result = db.execute("UNWIND [42] AS x RETURN nonexistent(x)");
    assert!(result.is_err());
    let err = result.expect_err("should fail");
    let err_msg = format!("{}", err);
    assert!(
        err_msg.contains("unknown function") || err_msg.contains("not found"),
        "expected 'unknown function' or 'not found' in error, got: {}",
        err_msg
    );
}

/// P10B-005: list_scalar_functions returns registered functions.
#[test]
fn test_list_scalar_functions() {
    let dir = tempdir().expect("tempdir");
    let mut db = CypherLite::open(test_config(dir.path())).expect("open");

    db.register_scalar_function(Box::new(DoubleFunction))
        .expect("register double");
    db.register_scalar_function(Box::new(MyUpperFunction))
        .expect("register my_upper");

    let fns = db.list_scalar_functions();
    assert_eq!(fns.len(), 2);

    let names: Vec<&str> = fns.iter().map(|(n, _)| *n).collect();
    assert!(names.contains(&"double"));
    assert!(names.contains(&"my_upper"));
}

/// P10B-006: Duplicate function registration returns error.
#[test]
fn test_duplicate_registration_error() {
    let dir = tempdir().expect("tempdir");
    let mut db = CypherLite::open(test_config(dir.path())).expect("open");

    db.register_scalar_function(Box::new(DoubleFunction))
        .expect("first register");

    let result = db.register_scalar_function(Box::new(DoubleFunction));
    assert!(result.is_err());
}
