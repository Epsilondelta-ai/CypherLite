//! Integration tests for the IndexPlugin system (Phase 10c).
//!
//! All tests are gated behind `#[cfg(feature = "plugin")]`.

#![cfg(feature = "plugin")]

use std::collections::HashMap;

use cypherlite_core::error::CypherLiteError;
use cypherlite_core::plugin::{IndexPlugin, Plugin};
use cypherlite_core::types::{NodeId, PropertyValue};
use cypherlite_query::api::CypherLite;

use tempfile::tempdir;

// ---------------------------------------------------------------------------
// Test helpers: sample IndexPlugin implementations
// ---------------------------------------------------------------------------

/// A simple hash-map backed index plugin for testing.
struct TestHashIndex {
    plugin_name: String,
    index_type: String,
    data: HashMap<PropertyValueKey, Vec<NodeId>>,
}

/// Wrapper to use PropertyValue as HashMap key (simplified for tests).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
enum PropertyValueKey {
    Null,
    Bool(bool),
    Int64(i64),
    String(String),
}

impl PropertyValueKey {
    fn from_property_value(pv: &PropertyValue) -> Self {
        match pv {
            PropertyValue::Null => Self::Null,
            PropertyValue::Bool(b) => Self::Bool(*b),
            PropertyValue::Int64(i) => Self::Int64(*i),
            PropertyValue::String(s) => Self::String(s.clone()),
            _ => Self::Null, // simplified fallback for tests
        }
    }
}

impl TestHashIndex {
    fn new(index_type: &str) -> Self {
        Self {
            plugin_name: format!("{}-index", index_type),
            index_type: index_type.to_string(),
            data: HashMap::new(),
        }
    }
}

impl Plugin for TestHashIndex {
    fn name(&self) -> &str {
        &self.plugin_name
    }
    fn version(&self) -> &str {
        "1.0.0"
    }
}

impl IndexPlugin for TestHashIndex {
    fn index_type(&self) -> &str {
        &self.index_type
    }

    fn insert(&mut self, key: &PropertyValue, node_id: NodeId) -> Result<(), CypherLiteError> {
        let k = PropertyValueKey::from_property_value(key);
        self.data.entry(k).or_default().push(node_id);
        Ok(())
    }

    fn remove(&mut self, key: &PropertyValue, node_id: NodeId) -> Result<(), CypherLiteError> {
        let k = PropertyValueKey::from_property_value(key);
        if let Some(ids) = self.data.get_mut(&k) {
            ids.retain(|id| *id != node_id);
            if ids.is_empty() {
                self.data.remove(&k);
            }
        }
        Ok(())
    }

    fn lookup(&self, key: &PropertyValue) -> Result<Vec<NodeId>, CypherLiteError> {
        let k = PropertyValueKey::from_property_value(key);
        Ok(self.data.get(&k).cloned().unwrap_or_default())
    }
}

/// A second index plugin with a different type, for multi-registration tests.
struct TestRTreeIndex;

impl Plugin for TestRTreeIndex {
    fn name(&self) -> &str {
        "rtree-index"
    }
    fn version(&self) -> &str {
        "0.1.0"
    }
}

impl IndexPlugin for TestRTreeIndex {
    fn index_type(&self) -> &str {
        "rtree"
    }

    fn insert(&mut self, _key: &PropertyValue, _node_id: NodeId) -> Result<(), CypherLiteError> {
        Ok(()) // stub
    }

    fn remove(&mut self, _key: &PropertyValue, _node_id: NodeId) -> Result<(), CypherLiteError> {
        Ok(()) // stub
    }

    fn lookup(&self, _key: &PropertyValue) -> Result<Vec<NodeId>, CypherLiteError> {
        Ok(vec![]) // stub
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
// P10C-001: Register a custom IndexPlugin and verify it is listed.
// ---------------------------------------------------------------------------

#[test]
fn test_register_index_plugin_and_list() {
    let dir = tempdir().expect("tempdir");
    let mut db = CypherLite::open(test_config(dir.path())).expect("open");

    db.register_index_plugin(Box::new(TestHashIndex::new("hash")))
        .expect("register");

    let plugins = db.list_index_plugins();
    assert_eq!(plugins.len(), 1);
    assert_eq!(plugins[0].0, "hash-index"); // name
    assert_eq!(plugins[0].1, "1.0.0"); // version
    assert_eq!(plugins[0].2, "hash"); // index_type
}

// ---------------------------------------------------------------------------
// P10C-002: Duplicate IndexPlugin registration returns error.
// ---------------------------------------------------------------------------

#[test]
fn test_duplicate_index_plugin_registration_error() {
    let dir = tempdir().expect("tempdir");
    let mut db = CypherLite::open(test_config(dir.path())).expect("open");

    db.register_index_plugin(Box::new(TestHashIndex::new("hash")))
        .expect("first register");

    let result = db.register_index_plugin(Box::new(TestHashIndex::new("hash")));
    assert!(result.is_err());
    let err_msg = format!("{}", result.expect_err("should fail"));
    assert!(
        err_msg.contains("already registered"),
        "expected 'already registered' in error, got: {}",
        err_msg
    );
}

// ---------------------------------------------------------------------------
// P10C-003: IndexPlugin insert/lookup round-trip through registry.
// ---------------------------------------------------------------------------

#[test]
fn test_index_plugin_insert_lookup_roundtrip() {
    let dir = tempdir().expect("tempdir");
    let mut db = CypherLite::open(test_config(dir.path())).expect("open");

    db.register_index_plugin(Box::new(TestHashIndex::new("hash")))
        .expect("register");

    // Insert via mutable access
    {
        let plugin = db
            .get_index_plugin_mut("hash-index")
            .expect("should find plugin");
        plugin
            .insert(&PropertyValue::String("Alice".into()), NodeId(1))
            .expect("insert");
        plugin
            .insert(&PropertyValue::String("Alice".into()), NodeId(2))
            .expect("insert");
        plugin
            .insert(&PropertyValue::String("Bob".into()), NodeId(3))
            .expect("insert");
    }

    // Lookup via immutable access
    {
        let plugin = db
            .get_index_plugin("hash-index")
            .expect("should find plugin");
        let alice_ids = plugin
            .lookup(&PropertyValue::String("Alice".into()))
            .expect("lookup");
        assert_eq!(alice_ids, vec![NodeId(1), NodeId(2)]);

        let bob_ids = plugin
            .lookup(&PropertyValue::String("Bob".into()))
            .expect("lookup");
        assert_eq!(bob_ids, vec![NodeId(3)]);

        let missing = plugin
            .lookup(&PropertyValue::String("Charlie".into()))
            .expect("lookup");
        assert!(missing.is_empty());
    }
}

// ---------------------------------------------------------------------------
// P10C-004: IndexPlugin remove then lookup returns empty.
// ---------------------------------------------------------------------------

#[test]
fn test_index_plugin_remove_then_lookup() {
    let dir = tempdir().expect("tempdir");
    let mut db = CypherLite::open(test_config(dir.path())).expect("open");

    db.register_index_plugin(Box::new(TestHashIndex::new("hash")))
        .expect("register");

    // Insert
    {
        let plugin = db
            .get_index_plugin_mut("hash-index")
            .expect("should find plugin");
        plugin
            .insert(&PropertyValue::Int64(42), NodeId(10))
            .expect("insert");
        plugin
            .insert(&PropertyValue::Int64(42), NodeId(20))
            .expect("insert");
    }

    // Remove one entry
    {
        let plugin = db
            .get_index_plugin_mut("hash-index")
            .expect("should find plugin");
        plugin
            .remove(&PropertyValue::Int64(42), NodeId(10))
            .expect("remove");
    }

    // Verify only one remains
    {
        let plugin = db
            .get_index_plugin("hash-index")
            .expect("should find plugin");
        let ids = plugin
            .lookup(&PropertyValue::Int64(42))
            .expect("lookup");
        assert_eq!(ids, vec![NodeId(20)]);
    }

    // Remove the last entry
    {
        let plugin = db
            .get_index_plugin_mut("hash-index")
            .expect("should find plugin");
        plugin
            .remove(&PropertyValue::Int64(42), NodeId(20))
            .expect("remove");
    }

    // Verify empty
    {
        let plugin = db
            .get_index_plugin("hash-index")
            .expect("should find plugin");
        let ids = plugin
            .lookup(&PropertyValue::Int64(42))
            .expect("lookup");
        assert!(ids.is_empty());
    }
}

// ---------------------------------------------------------------------------
// P10C-005: Multiple IndexPlugins with different types.
// ---------------------------------------------------------------------------

#[test]
fn test_multiple_index_plugins() {
    let dir = tempdir().expect("tempdir");
    let mut db = CypherLite::open(test_config(dir.path())).expect("open");

    db.register_index_plugin(Box::new(TestHashIndex::new("hash")))
        .expect("register hash");
    db.register_index_plugin(Box::new(TestRTreeIndex))
        .expect("register rtree");

    let plugins = db.list_index_plugins();
    assert_eq!(plugins.len(), 2);

    let names: Vec<&str> = plugins.iter().map(|(n, _, _)| *n).collect();
    assert!(names.contains(&"hash-index"));
    assert!(names.contains(&"rtree-index"));

    let types: Vec<&str> = plugins.iter().map(|(_, _, t)| *t).collect();
    assert!(types.contains(&"hash"));
    assert!(types.contains(&"rtree"));
}

// ---------------------------------------------------------------------------
// P10C-006: get_index_plugin for nonexistent name returns None.
// ---------------------------------------------------------------------------

#[test]
fn test_get_nonexistent_index_plugin() {
    let dir = tempdir().expect("tempdir");
    let db = CypherLite::open(test_config(dir.path())).expect("open");

    assert!(db.get_index_plugin("nonexistent").is_none());
}
