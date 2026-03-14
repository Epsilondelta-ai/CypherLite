//! Integration tests for the plugin system (Phase 10a).
//!
//! These tests verify:
//! - Plugin trait object safety and Send+Sync bounds
//! - All 4 extension traits (ScalarFunction, IndexPlugin, Serializer, Trigger)
//! - PluginRegistry CRUD operations
//! - Plugin error variant formatting

#![cfg(feature = "plugin")]

use std::collections::HashMap;

use cypherlite_core::error::CypherLiteError;
use cypherlite_core::plugin::{
    EntityType, IndexPlugin, Plugin, PluginRegistry, ScalarFunction, Serializer, Trigger,
    TriggerContext, TriggerOperation,
};
use cypherlite_core::types::{NodeId, PropertyValue};

// ---------------------------------------------------------------------------
// Mock implementations for testing
// ---------------------------------------------------------------------------

/// Minimal Plugin impl used for base-trait tests.
struct MockPlugin {
    name: String,
    version: String,
}

impl MockPlugin {
    fn new(name: &str, version: &str) -> Self {
        Self {
            name: name.to_string(),
            version: version.to_string(),
        }
    }
}

impl Plugin for MockPlugin {
    fn name(&self) -> &str {
        &self.name
    }
    fn version(&self) -> &str {
        &self.version
    }
}

// -- ScalarFunction mock --

struct MockScalarFn;

impl Plugin for MockScalarFn {
    fn name(&self) -> &str {
        "mock_scalar"
    }
    fn version(&self) -> &str {
        "0.1.0"
    }
}

impl ScalarFunction for MockScalarFn {
    fn call(&self, args: &[PropertyValue]) -> Result<PropertyValue, CypherLiteError> {
        // Return the first argument or Null.
        Ok(args.first().cloned().unwrap_or(PropertyValue::Null))
    }
}

// -- IndexPlugin mock --

struct MockIndexPlugin {
    entries: HashMap<Vec<u8>, Vec<NodeId>>,
}

impl MockIndexPlugin {
    fn new() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }
}

impl Plugin for MockIndexPlugin {
    fn name(&self) -> &str {
        "mock_index"
    }
    fn version(&self) -> &str {
        "0.1.0"
    }
}

impl IndexPlugin for MockIndexPlugin {
    fn index_type(&self) -> &str {
        "mock_btree"
    }

    fn insert(&mut self, key: &PropertyValue, node_id: NodeId) -> Result<(), CypherLiteError> {
        let key_bytes = format!("{key:?}").into_bytes();
        self.entries.entry(key_bytes).or_default().push(node_id);
        Ok(())
    }

    fn remove(&mut self, key: &PropertyValue, node_id: NodeId) -> Result<(), CypherLiteError> {
        let key_bytes = format!("{key:?}").into_bytes();
        if let Some(ids) = self.entries.get_mut(&key_bytes) {
            ids.retain(|id| *id != node_id);
        }
        Ok(())
    }

    fn lookup(&self, key: &PropertyValue) -> Result<Vec<NodeId>, CypherLiteError> {
        let key_bytes = format!("{key:?}").into_bytes();
        Ok(self.entries.get(&key_bytes).cloned().unwrap_or_default())
    }
}

// -- Serializer mock --

struct MockSerializer;

impl Plugin for MockSerializer {
    fn name(&self) -> &str {
        "mock_serializer"
    }
    fn version(&self) -> &str {
        "0.1.0"
    }
}

impl Serializer for MockSerializer {
    fn format(&self) -> &str {
        "mock_json"
    }

    fn export(&self, data: &[HashMap<String, PropertyValue>]) -> Result<Vec<u8>, CypherLiteError> {
        // Trivial: return the number of rows as bytes.
        Ok((data.len() as u64).to_le_bytes().to_vec())
    }

    fn import(&self, bytes: &[u8]) -> Result<Vec<HashMap<String, PropertyValue>>, CypherLiteError> {
        if bytes.len() < 8 {
            return Err(CypherLiteError::UnsupportedFormat("too short".to_string()));
        }
        let count = u64::from_le_bytes(bytes[..8].try_into().unwrap()) as usize;
        Ok(vec![HashMap::new(); count])
    }
}

// -- Trigger mock --

struct MockTrigger {
    name: String,
}

impl MockTrigger {
    fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
        }
    }
}

impl Plugin for MockTrigger {
    fn name(&self) -> &str {
        &self.name
    }
    fn version(&self) -> &str {
        "0.1.0"
    }
}

impl Trigger for MockTrigger {
    fn on_before_create(&self, _ctx: &TriggerContext) -> Result<(), CypherLiteError> {
        Ok(())
    }
    fn on_after_create(&self, _ctx: &TriggerContext) -> Result<(), CypherLiteError> {
        Ok(())
    }
    fn on_before_update(&self, _ctx: &TriggerContext) -> Result<(), CypherLiteError> {
        Ok(())
    }
    fn on_after_update(&self, _ctx: &TriggerContext) -> Result<(), CypherLiteError> {
        Ok(())
    }
    fn on_before_delete(&self, _ctx: &TriggerContext) -> Result<(), CypherLiteError> {
        Ok(())
    }
    fn on_after_delete(&self, _ctx: &TriggerContext) -> Result<(), CypherLiteError> {
        Ok(())
    }
}

// ===========================================================================
// Tests
// ===========================================================================

// ---- Object safety: every trait must be usable as a trait object -----------

#[test]
fn test_plugin_trait_object_safety() {
    let p: Box<dyn Plugin> = Box::new(MockPlugin::new("test", "1.0.0"));
    assert_eq!(p.name(), "test");
    assert_eq!(p.version(), "1.0.0");
}

#[test]
fn test_scalar_function_trait_object_safety() {
    let f: Box<dyn ScalarFunction> = Box::new(MockScalarFn);
    assert_eq!(f.name(), "mock_scalar");
    let result = f.call(&[PropertyValue::Int64(42)]).unwrap();
    assert_eq!(result, PropertyValue::Int64(42));
}

#[test]
fn test_index_plugin_trait_object_safety() {
    // IndexPlugin has &mut self methods, so Box<dyn IndexPlugin> must work.
    let mut idx: Box<dyn IndexPlugin> = Box::new(MockIndexPlugin::new());
    assert_eq!(idx.index_type(), "mock_btree");
    idx.insert(&PropertyValue::Int64(1), NodeId(10)).unwrap();
    let results = idx.lookup(&PropertyValue::Int64(1)).unwrap();
    assert_eq!(results, vec![NodeId(10)]);
}

#[test]
fn test_serializer_trait_object_safety() {
    let s: Box<dyn Serializer> = Box::new(MockSerializer);
    assert_eq!(s.format(), "mock_json");

    let data = vec![HashMap::from([(
        "key".to_string(),
        PropertyValue::String("value".to_string()),
    )])];
    let bytes = s.export(&data).unwrap();
    let imported = s.import(&bytes).unwrap();
    assert_eq!(imported.len(), 1);
}

#[test]
fn test_trigger_trait_object_safety() {
    let t: Box<dyn Trigger> = Box::new(MockTrigger::new("audit_trigger"));
    assert_eq!(t.name(), "audit_trigger");

    let ctx = TriggerContext {
        entity_type: EntityType::Node,
        entity_id: 1,
        label_or_type: Some("Person".to_string()),
        properties: HashMap::new(),
        operation: TriggerOperation::Create,
    };
    assert!(t.on_before_create(&ctx).is_ok());
    assert!(t.on_after_create(&ctx).is_ok());
}

// ---- Send + Sync compile-time checks -------------------------------------

#[test]
fn test_all_traits_are_send_sync() {
    fn assert_send<T: Send>() {}
    fn assert_sync<T: Sync>() {}

    // Base trait
    assert_send::<Box<dyn Plugin>>();
    assert_sync::<Box<dyn Plugin>>();

    // Extension traits
    assert_send::<Box<dyn ScalarFunction>>();
    assert_sync::<Box<dyn ScalarFunction>>();

    assert_send::<Box<dyn IndexPlugin>>();
    assert_sync::<Box<dyn IndexPlugin>>();

    assert_send::<Box<dyn Serializer>>();
    assert_sync::<Box<dyn Serializer>>();

    assert_send::<Box<dyn Trigger>>();
    assert_sync::<Box<dyn Trigger>>();
}

// ---- PluginRegistry -------------------------------------------------------

#[test]
fn test_registry_register_and_get() {
    let mut registry = PluginRegistry::<dyn Plugin>::new();
    let plugin = MockPlugin::new("my_plugin", "1.0.0");
    registry.register(Box::new(plugin)).unwrap();

    let retrieved = registry.get("my_plugin");
    assert!(retrieved.is_some());
    assert_eq!(retrieved.unwrap().name(), "my_plugin");
    assert_eq!(retrieved.unwrap().version(), "1.0.0");
}

#[test]
fn test_registry_duplicate_name_returns_plugin_error() {
    let mut registry = PluginRegistry::<dyn Plugin>::new();
    let p1 = MockPlugin::new("dup", "1.0.0");
    let p2 = MockPlugin::new("dup", "2.0.0");

    registry.register(Box::new(p1)).unwrap();
    let err = registry.register(Box::new(p2)).unwrap_err();

    match err {
        CypherLiteError::PluginError(msg) => {
            assert!(
                msg.contains("dup"),
                "Error message should contain plugin name"
            );
        }
        other => panic!("Expected PluginError, got: {other}"),
    }
}

#[test]
fn test_registry_list_returns_all_registered() {
    let mut registry = PluginRegistry::<dyn Plugin>::new();
    registry
        .register(Box::new(MockPlugin::new("alpha", "1.0")))
        .unwrap();
    registry
        .register(Box::new(MockPlugin::new("beta", "2.0")))
        .unwrap();
    registry
        .register(Box::new(MockPlugin::new("gamma", "3.0")))
        .unwrap();

    let mut names: Vec<&str> = registry.list().collect();
    names.sort();
    assert_eq!(names, vec!["alpha", "beta", "gamma"]);
}

#[test]
fn test_registry_contains() {
    let mut registry = PluginRegistry::<dyn Plugin>::new();
    assert!(!registry.contains("x"));

    registry
        .register(Box::new(MockPlugin::new("x", "1.0")))
        .unwrap();
    assert!(registry.contains("x"));
    assert!(!registry.contains("y"));
}

#[test]
fn test_registry_get_mut() {
    let mut registry = PluginRegistry::<dyn IndexPlugin>::new();
    registry.register(Box::new(MockIndexPlugin::new())).unwrap();

    // Insert via get_mut
    let idx = registry.get_mut("mock_index").unwrap();
    idx.insert(&PropertyValue::Int64(5), NodeId(100)).unwrap();

    // Verify via get (immutable)
    let idx = registry.get("mock_index").unwrap();
    let results = idx.lookup(&PropertyValue::Int64(5)).unwrap();
    assert_eq!(results, vec![NodeId(100)]);
}

// ---- Error Display formatting ---------------------------------------------

#[test]
fn test_plugin_error_display() {
    let err = CypherLiteError::PluginError("load failed".to_string());
    assert_eq!(format!("{err}"), "Plugin error: load failed");
}

#[test]
fn test_function_not_found_error_display() {
    let err = CypherLiteError::FunctionNotFound("myFunc".to_string());
    assert_eq!(format!("{err}"), "Function not found: myFunc");
}

#[test]
fn test_unsupported_index_type_error_display() {
    let err = CypherLiteError::UnsupportedIndexType("rtree".to_string());
    assert_eq!(format!("{err}"), "Unsupported index type: rtree");
}

#[test]
fn test_unsupported_format_error_display() {
    let err = CypherLiteError::UnsupportedFormat("xml".to_string());
    assert_eq!(format!("{err}"), "Unsupported format: xml");
}

#[test]
fn test_trigger_error_display() {
    let err = CypherLiteError::TriggerError("validation failed".to_string());
    assert_eq!(format!("{err}"), "Trigger error: validation failed");
}

// ---- TriggerContext and enum construction ---------------------------------

#[test]
fn test_trigger_context_construction() {
    let mut props = HashMap::new();
    props.insert(
        "name".to_string(),
        PropertyValue::String("Alice".to_string()),
    );
    props.insert("age".to_string(), PropertyValue::Int64(30));

    let ctx = TriggerContext {
        entity_type: EntityType::Edge,
        entity_id: 42,
        label_or_type: Some("KNOWS".to_string()),
        properties: props,
        operation: TriggerOperation::Update,
    };

    assert!(matches!(ctx.entity_type, EntityType::Edge));
    assert_eq!(ctx.entity_id, 42);
    assert_eq!(ctx.label_or_type.as_deref(), Some("KNOWS"));
    assert_eq!(ctx.properties.len(), 2);
    assert!(matches!(ctx.operation, TriggerOperation::Update));
}

#[test]
fn test_trigger_operation_variants() {
    let _create = TriggerOperation::Create;
    let _update = TriggerOperation::Update;
    let _delete = TriggerOperation::Delete;
}

#[test]
fn test_entity_type_variants() {
    let _node = EntityType::Node;
    let _edge = EntityType::Edge;
}
