#![warn(missing_docs)]
//! Core types, traits, and error definitions for the CypherLite graph database.

/// Database configuration and sync mode definitions.
pub mod config;
/// Error types for all CypherLite operations.
pub mod error;
/// Trait definitions for transactions and registries.
pub mod traits;
/// Core graph data types (nodes, edges, properties, identifiers).
pub mod types;

/// Plugin system: base trait, extension traits, and registry.
#[cfg(feature = "plugin")]
pub mod plugin;

pub use config::{DatabaseConfig, SyncMode};
pub use error::{CypherLiteError, Result};
pub use traits::{LabelRegistry, TransactionView};
pub use types::{Direction, EdgeId, NodeId, NodeRecord, PageId, PropertyValue, RelationshipRecord};

#[cfg(feature = "subgraph")]
pub use types::{GraphEntity, SubgraphId, SubgraphRecord};

#[cfg(feature = "hypergraph")]
pub use types::{HyperEdgeId, HyperEdgeRecord};
