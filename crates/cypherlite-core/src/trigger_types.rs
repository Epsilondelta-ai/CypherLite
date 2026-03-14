//! Trigger-related data types used by both the plugin system and the executor.
//!
//! These types are always available (not feature-gated) so that the executor
//! can construct [`TriggerContext`] values regardless of whether the `plugin`
//! feature is enabled.

use std::collections::HashMap;

use crate::types::PropertyValue;

/// The kind of graph entity a trigger is operating on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntityType {
    /// A node in the graph.
    Node,
    /// An edge (relationship) in the graph.
    Edge,
}

/// The CRUD operation that triggered the event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TriggerOperation {
    /// A new entity is being created.
    Create,
    /// An existing entity is being updated.
    Update,
    /// An entity is being deleted.
    Delete,
}

/// Context passed to trigger callbacks describing the affected entity.
#[derive(Debug, Clone)]
pub struct TriggerContext {
    /// Whether the entity is a node or edge.
    pub entity_type: EntityType,
    /// The internal ID of the entity.
    pub entity_id: u64,
    /// The label (for nodes) or relationship type (for edges), if available.
    pub label_or_type: Option<String>,
    /// The current or proposed properties of the entity.
    pub properties: HashMap<String, PropertyValue>,
    /// The operation being performed.
    pub operation: TriggerOperation,
}
