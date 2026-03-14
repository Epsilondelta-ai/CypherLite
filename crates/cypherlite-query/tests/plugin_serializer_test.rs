//! Integration tests for the Serializer plugin system (Phase 10d).
//!
//! All tests are gated behind `#[cfg(feature = "plugin")]`.

#![cfg(feature = "plugin")]

use cypherlite_core::error::CypherLiteError;
use cypherlite_core::plugin::{Plugin, Serializer};
use cypherlite_core::types::PropertyValue;
use cypherlite_query::api::CypherLite;

use std::collections::HashMap;
use tempfile::tempdir;

// ---------------------------------------------------------------------------
// Test helpers: sample Serializer implementations
// ---------------------------------------------------------------------------

/// A simple JSON-like serializer for testing purposes.
struct TestJsonSerializer;

impl Plugin for TestJsonSerializer {
    fn name(&self) -> &str {
        "json-serializer"
    }
    fn version(&self) -> &str {
        "1.0.0"
    }
}

impl Serializer for TestJsonSerializer {
    fn format(&self) -> &str {
        "json"
    }

    fn export(&self, data: &[HashMap<String, PropertyValue>]) -> Result<Vec<u8>, CypherLiteError> {
        let mut output = String::from("[");
        for (i, row) in data.iter().enumerate() {
            if i > 0 {
                output.push(',');
            }
            output.push('{');
            let mut entries: Vec<_> = row.iter().collect();
            entries.sort_by_key(|(k, _)| (*k).clone());
            for (j, (k, v)) in entries.iter().enumerate() {
                if j > 0 {
                    output.push(',');
                }
                output.push_str(&format!("\"{}\":", k));
                match v {
                    PropertyValue::String(s) => output.push_str(&format!("\"{}\"", s)),
                    PropertyValue::Int64(n) => output.push_str(&format!("{}", n)),
                    PropertyValue::Bool(b) => output.push_str(&format!("{}", b)),
                    PropertyValue::Float64(f) => output.push_str(&format!("{}", f)),
                    PropertyValue::Null => output.push_str("null"),
                    _ => output.push_str("null"),
                }
            }
            output.push('}');
        }
        output.push(']');
        Ok(output.into_bytes())
    }

    fn import(&self, bytes: &[u8]) -> Result<Vec<HashMap<String, PropertyValue>>, CypherLiteError> {
        if bytes.is_empty() {
            return Err(CypherLiteError::PluginError("empty input".into()));
        }
        // Minimal parser: just return a single row with the byte count for testing.
        let mut row = HashMap::new();
        row.insert(
            "byte_count".to_string(),
            PropertyValue::Int64(bytes.len() as i64),
        );
        Ok(vec![row])
    }
}

/// A CSV serializer for testing duplicate format detection.
struct TestCsvSerializer;

impl Plugin for TestCsvSerializer {
    fn name(&self) -> &str {
        "csv-serializer"
    }
    fn version(&self) -> &str {
        "1.0.0"
    }
}

impl Serializer for TestCsvSerializer {
    fn format(&self) -> &str {
        "csv"
    }

    fn export(&self, data: &[HashMap<String, PropertyValue>]) -> Result<Vec<u8>, CypherLiteError> {
        let mut output = String::new();
        if let Some(first) = data.first() {
            let mut keys: Vec<_> = first.keys().collect();
            keys.sort();
            output.push_str(
                &keys
                    .iter()
                    .map(|k| k.as_str())
                    .collect::<Vec<_>>()
                    .join(","),
            );
            output.push('\n');
            for row in data {
                let vals: Vec<String> = keys
                    .iter()
                    .map(|k| match row.get(*k) {
                        Some(PropertyValue::String(s)) => s.clone(),
                        Some(PropertyValue::Int64(n)) => n.to_string(),
                        Some(PropertyValue::Bool(b)) => b.to_string(),
                        _ => "".to_string(),
                    })
                    .collect();
                output.push_str(&vals.join(","));
                output.push('\n');
            }
        }
        Ok(output.into_bytes())
    }

    fn import(&self, bytes: &[u8]) -> Result<Vec<HashMap<String, PropertyValue>>, CypherLiteError> {
        if bytes.is_empty() {
            return Err(CypherLiteError::PluginError("empty input".into()));
        }
        Ok(vec![])
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
// TASK-010: Serializer plugin integration tests
// ---------------------------------------------------------------------------

/// P10D-001: Register a Serializer and verify it appears in list_serializers.
#[test]
fn test_register_serializer_and_list() {
    let dir = tempdir().expect("tempdir");
    let mut db = CypherLite::open(test_config(dir.path())).expect("open");

    db.register_serializer(Box::new(TestJsonSerializer))
        .expect("register json");

    let serializers = db.list_serializers();
    assert_eq!(serializers.len(), 1);
    assert!(serializers
        .iter()
        .any(|(name, _)| *name == "json-serializer"));
}

/// P10D-002: Register multiple serializers and verify all are listed.
#[test]
fn test_register_multiple_serializers() {
    let dir = tempdir().expect("tempdir");
    let mut db = CypherLite::open(test_config(dir.path())).expect("open");

    db.register_serializer(Box::new(TestJsonSerializer))
        .expect("register json");
    db.register_serializer(Box::new(TestCsvSerializer))
        .expect("register csv");

    let serializers = db.list_serializers();
    assert_eq!(serializers.len(), 2);

    let names: Vec<&str> = serializers.iter().map(|(n, _)| *n).collect();
    assert!(names.contains(&"json-serializer"));
    assert!(names.contains(&"csv-serializer"));
}

/// P10D-003: Duplicate serializer registration returns error.
#[test]
fn test_duplicate_serializer_registration_error() {
    let dir = tempdir().expect("tempdir");
    let mut db = CypherLite::open(test_config(dir.path())).expect("open");

    db.register_serializer(Box::new(TestJsonSerializer))
        .expect("first register");

    let result = db.register_serializer(Box::new(TestJsonSerializer));
    assert!(result.is_err());
    let err = format!("{}", result.expect_err("should fail"));
    assert!(
        err.contains("already registered"),
        "expected 'already registered' in error, got: {}",
        err
    );
}

/// P10D-004: export_data with unknown format returns UnsupportedFormat error.
#[test]
fn test_export_data_unsupported_format() {
    let dir = tempdir().expect("tempdir");
    let mut db = CypherLite::open(test_config(dir.path())).expect("open");

    let result = db.export_data("xml", "MATCH (n) RETURN n.name");
    assert!(result.is_err());
    let err = format!("{}", result.expect_err("should fail"));
    assert!(
        err.contains("Unsupported format") || err.contains("xml"),
        "expected UnsupportedFormat error, got: {}",
        err
    );
}

/// P10D-005: import_data with unknown format returns UnsupportedFormat error.
#[test]
fn test_import_data_unsupported_format() {
    let dir = tempdir().expect("tempdir");
    let db = CypherLite::open(test_config(dir.path())).expect("open");

    let result = db.import_data("xml", b"some data");
    assert!(result.is_err());
    let err = format!("{}", result.expect_err("should fail"));
    assert!(
        err.contains("Unsupported format") || err.contains("xml"),
        "expected UnsupportedFormat error, got: {}",
        err
    );
}

/// P10D-006: export_data executes a query and serializes results.
#[test]
fn test_export_data_executes_query() {
    let dir = tempdir().expect("tempdir");
    let mut db = CypherLite::open(test_config(dir.path())).expect("open");

    db.register_serializer(Box::new(TestJsonSerializer))
        .expect("register");

    db.execute("CREATE (n:Person {name: 'Alice', age: 30})")
        .expect("create");

    let bytes = db
        .export_data("json", "MATCH (n:Person) RETURN n.name, n.age")
        .expect("export");

    let output = String::from_utf8(bytes).expect("utf8");
    assert!(output.starts_with("[{"));
    assert!(output.ends_with("}]"));
    assert!(output.contains("Alice"));
    assert!(output.contains("30"));
}

/// P10D-007: import_data deserializes bytes through registered serializer.
#[test]
fn test_import_data_through_serializer() {
    let dir = tempdir().expect("tempdir");
    let mut db = CypherLite::open(test_config(dir.path())).expect("open");

    db.register_serializer(Box::new(TestJsonSerializer))
        .expect("register");

    let input = b"[{\"name\":\"Alice\"}]";
    let rows = db.import_data("json", input).expect("import");
    // TestJsonSerializer.import returns a single row with byte_count
    assert_eq!(rows.len(), 1);
    assert_eq!(
        rows[0].get("byte_count"),
        Some(&PropertyValue::Int64(input.len() as i64))
    );
}

/// P10D-008: export -> import roundtrip through registered serializer.
#[test]
fn test_export_import_roundtrip() {
    let dir = tempdir().expect("tempdir");
    let mut db = CypherLite::open(test_config(dir.path())).expect("open");

    db.register_serializer(Box::new(TestJsonSerializer))
        .expect("register");

    db.execute("CREATE (n:Person {name: 'Bob'})")
        .expect("create");

    // Export
    let bytes = db
        .export_data("json", "MATCH (n:Person) RETURN n.name")
        .expect("export");
    assert!(!bytes.is_empty());

    // Import the exported bytes
    let rows = db.import_data("json", &bytes).expect("import");
    assert!(!rows.is_empty());
}

/// P10D-009: export_data with empty result set produces valid output.
#[test]
fn test_export_data_empty_result() {
    let dir = tempdir().expect("tempdir");
    let mut db = CypherLite::open(test_config(dir.path())).expect("open");

    db.register_serializer(Box::new(TestJsonSerializer))
        .expect("register");

    let bytes = db
        .export_data("json", "MATCH (n:NonExistent) RETURN n.name")
        .expect("export empty");

    let output = String::from_utf8(bytes).expect("utf8");
    assert_eq!(output, "[]");
}

/// P10D-010: export_data filters out non-convertible values (Node, Edge).
#[test]
fn test_export_data_filters_graph_entities() {
    let dir = tempdir().expect("tempdir");
    let mut db = CypherLite::open(test_config(dir.path())).expect("open");

    db.register_serializer(Box::new(TestJsonSerializer))
        .expect("register");

    db.execute("CREATE (n:Person {name: 'Alice'})")
        .expect("create");

    // RETURN n returns a Node value (not convertible to PropertyValue).
    // RETURN n.name returns a String (convertible).
    // The export should include n.name but silently skip n.
    let bytes = db
        .export_data("json", "MATCH (n:Person) RETURN n, n.name")
        .expect("export with node");

    let output = String::from_utf8(bytes).expect("utf8");
    // n.name should be present, n (Node) should be filtered out
    assert!(output.contains("Alice"));
}
