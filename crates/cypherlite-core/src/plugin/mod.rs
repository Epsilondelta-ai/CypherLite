//! Plugin system core types and traits.
//!
//! Defines the base [`Plugin`] trait, four extension traits
//! ([`ScalarFunction`], [`IndexPlugin`], [`Serializer`], [`Trigger`]),
//! and a generic [`PluginRegistry`] for managing plugin instances.

use std::collections::HashMap;

use crate::error::CypherLiteError;
use crate::types::{NodeId, PropertyValue};

// Re-export trigger types from this module for backward compatibility.
// The canonical definitions live in crate::trigger_types (always available).
pub use crate::trigger_types::{EntityType, TriggerContext, TriggerOperation};

// ---------------------------------------------------------------------------
// Base trait
// ---------------------------------------------------------------------------

/// Base trait that every plugin must implement.
///
/// The trait is object-safe and requires `Send + Sync` so that plugin
/// instances can be shared across threads.
pub trait Plugin: Send + Sync {
    /// Returns the unique name of this plugin.
    fn name(&self) -> &str;

    /// Returns the version string (e.g. `"1.2.3"`).
    fn version(&self) -> &str;
}

// ---------------------------------------------------------------------------
// Extension traits
// ---------------------------------------------------------------------------

/// A user-defined scalar function callable from Cypher queries.
///
/// Accepts zero or more [`PropertyValue`] arguments and returns a single
/// [`PropertyValue`].
pub trait ScalarFunction: Plugin {
    /// Execute the function with the given arguments.
    fn call(&self, args: &[PropertyValue]) -> Result<PropertyValue, CypherLiteError>;
}

/// A custom index implementation that can be plugged into the storage layer.
pub trait IndexPlugin: Plugin {
    /// Returns the identifier for this index type (e.g. `"btree"`, `"rtree"`).
    fn index_type(&self) -> &str;

    /// Insert a `(key, node_id)` entry into the index.
    fn insert(&mut self, key: &PropertyValue, node_id: NodeId) -> Result<(), CypherLiteError>;

    /// Remove a `(key, node_id)` entry from the index.
    fn remove(&mut self, key: &PropertyValue, node_id: NodeId) -> Result<(), CypherLiteError>;

    /// Look up all node IDs associated with the given key.
    fn lookup(&self, key: &PropertyValue) -> Result<Vec<NodeId>, CypherLiteError>;
}

/// A custom serialization format for import/export operations.
///
/// Uses `HashMap<String, PropertyValue>` as the row representation to avoid
/// a circular dependency on the query crate's `Row` type.
pub trait Serializer: Plugin {
    /// Returns the format identifier (e.g. `"json"`, `"csv"`).
    fn format(&self) -> &str;

    /// Serialize a slice of rows into bytes.
    fn export(
        &self,
        data: &[HashMap<String, PropertyValue>],
    ) -> Result<Vec<u8>, CypherLiteError>;

    /// Deserialize bytes into a vector of rows.
    fn import(
        &self,
        bytes: &[u8],
    ) -> Result<Vec<HashMap<String, PropertyValue>>, CypherLiteError>;
}

/// A trigger that fires before/after create, update, and delete operations.
pub trait Trigger: Plugin {
    /// Called before a new entity is created.
    fn on_before_create(&self, ctx: &TriggerContext) -> Result<(), CypherLiteError>;

    /// Called after a new entity has been created.
    fn on_after_create(&self, ctx: &TriggerContext) -> Result<(), CypherLiteError>;

    /// Called before an existing entity is updated.
    fn on_before_update(&self, ctx: &TriggerContext) -> Result<(), CypherLiteError>;

    /// Called after an existing entity has been updated.
    fn on_after_update(&self, ctx: &TriggerContext) -> Result<(), CypherLiteError>;

    /// Called before an entity is deleted.
    fn on_before_delete(&self, ctx: &TriggerContext) -> Result<(), CypherLiteError>;

    /// Called after an entity has been deleted.
    fn on_after_delete(&self, ctx: &TriggerContext) -> Result<(), CypherLiteError>;
}

// ---------------------------------------------------------------------------
// Supporting types (re-exported from crate::trigger_types)
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Plugin registry
// ---------------------------------------------------------------------------

/// A type-safe registry that stores and retrieves plugin instances by name.
///
/// `T` is typically a trait object such as `dyn Plugin`, `dyn ScalarFunction`,
/// or any other extension trait.
pub struct PluginRegistry<T: Plugin + ?Sized> {
    plugins: HashMap<String, Box<T>>,
}

impl<T: Plugin + ?Sized> PluginRegistry<T> {
    /// Create a new, empty registry.
    pub fn new() -> Self {
        Self {
            plugins: HashMap::new(),
        }
    }

    /// Register a plugin. Returns an error if a plugin with the same name is
    /// already registered.
    pub fn register(&mut self, plugin: Box<T>) -> Result<(), CypherLiteError> {
        let name = plugin.name().to_string();
        if self.plugins.contains_key(&name) {
            return Err(CypherLiteError::PluginError(format!(
                "Plugin already registered: {name}"
            )));
        }
        self.plugins.insert(name, plugin);
        Ok(())
    }

    /// Get an immutable reference to a plugin by name.
    pub fn get(&self, name: &str) -> Option<&T> {
        self.plugins.get(name).map(|b| b.as_ref())
    }

    /// Get a mutable reference to a plugin by name.
    pub fn get_mut(&mut self, name: &str) -> Option<&mut T> {
        self.plugins.get_mut(name).map(|b| b.as_mut())
    }

    /// Returns an iterator over the names of all registered plugins.
    pub fn list(&self) -> impl Iterator<Item = &str> {
        self.plugins.keys().map(|s| s.as_str())
    }

    /// Check whether a plugin with the given name is registered.
    pub fn contains(&self, name: &str) -> bool {
        self.plugins.contains_key(name)
    }
}

impl<T: Plugin + ?Sized> Default for PluginRegistry<T> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Verify EntityType and TriggerOperation derive Debug
    #[test]
    fn test_entity_type_debug() {
        assert_eq!(format!("{:?}", EntityType::Node), "Node");
        assert_eq!(format!("{:?}", EntityType::Edge), "Edge");
    }

    #[test]
    fn test_trigger_operation_debug() {
        assert_eq!(format!("{:?}", TriggerOperation::Create), "Create");
        assert_eq!(format!("{:?}", TriggerOperation::Update), "Update");
        assert_eq!(format!("{:?}", TriggerOperation::Delete), "Delete");
    }

    #[test]
    fn test_trigger_context_debug() {
        let ctx = TriggerContext {
            entity_type: EntityType::Node,
            entity_id: 1,
            label_or_type: None,
            properties: HashMap::new(),
            operation: TriggerOperation::Delete,
        };
        let debug_str = format!("{ctx:?}");
        assert!(debug_str.contains("Node"));
        assert!(debug_str.contains("Delete"));
    }
}
