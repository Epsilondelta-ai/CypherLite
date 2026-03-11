// SubgraphStore: in-memory subgraph entity storage
//
// GG-003: SubgraphStore module
// GG-004: CRUD operations
// GG-005: Integration with StorageEngine

/// Membership index tracking node-subgraph relationships.
pub mod membership;

use cypherlite_core::{PropertyValue, SubgraphId, SubgraphRecord};
use std::collections::BTreeMap;

/// In-memory subgraph store backed by a BTreeMap.
///
/// Stores subgraph records keyed by their u64 ID.
/// Provides CRUD operations for subgraph entities.
pub struct SubgraphStore {
    /// Storage: subgraph_id -> SubgraphRecord
    records: BTreeMap<u64, SubgraphRecord>,
    /// Next available subgraph ID.
    next_id: u64,
}

impl SubgraphStore {
    /// Create a new subgraph store with the given starting ID.
    pub fn new(start_id: u64) -> Self {
        Self {
            records: BTreeMap::new(),
            next_id: start_id,
        }
    }

    /// Create a new subgraph with the given properties and optional temporal anchor.
    /// Returns the assigned SubgraphId.
    pub fn create(
        &mut self,
        properties: Vec<(u32, PropertyValue)>,
        temporal_anchor: Option<i64>,
    ) -> SubgraphId {
        let id = SubgraphId(self.next_id);
        let record = SubgraphRecord {
            subgraph_id: id,
            temporal_anchor,
            properties,
        };
        self.records.insert(self.next_id, record);
        self.next_id += 1;
        id
    }

    /// Get a subgraph record by ID.
    pub fn get(&self, id: SubgraphId) -> Option<&SubgraphRecord> {
        self.records.get(&id.0)
    }

    /// Delete a subgraph by ID. Returns the deleted record if found.
    pub fn delete(&mut self, id: SubgraphId) -> Option<SubgraphRecord> {
        self.records.remove(&id.0)
    }

    /// Returns the next available subgraph ID.
    pub fn next_id(&self) -> u64 {
        self.next_id
    }

    /// Returns the number of subgraphs stored.
    pub fn len(&self) -> usize {
        self.records.len()
    }

    /// Returns true if the store is empty.
    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    /// Returns an iterator over all subgraph records.
    pub fn all(&self) -> impl Iterator<Item = &SubgraphRecord> {
        self.records.values()
    }
}

impl Default for SubgraphStore {
    fn default() -> Self {
        Self::new(1)
    }
}

#[cfg(test)]
mod tests {
    use cypherlite_core::{PropertyValue, SubgraphId};

    use super::*;

    // GG-003: SubgraphStore creation
    #[test]
    fn test_subgraph_store_new_is_empty() {
        let store = SubgraphStore::new(1);
        assert_eq!(store.len(), 0);
    }

    // GG-004: Create subgraph
    #[test]
    fn test_create_subgraph() {
        let mut store = SubgraphStore::new(1);
        let id = store.create(vec![], None);
        assert_eq!(id, SubgraphId(1));
        assert_eq!(store.len(), 1);
    }

    // GG-004: Create subgraph with properties and temporal anchor
    #[test]
    fn test_create_subgraph_with_properties() {
        let mut store = SubgraphStore::new(1);
        let props = vec![(1, PropertyValue::String("my-graph".into()))];
        let id = store.create(props.clone(), Some(1_700_000_000_000));
        let record = store.get(id).expect("found");
        assert_eq!(record.subgraph_id, id);
        assert_eq!(record.temporal_anchor, Some(1_700_000_000_000));
        assert_eq!(record.properties, props);
    }

    // GG-004: Create multiple subgraphs (incrementing IDs)
    #[test]
    fn test_create_multiple_subgraphs() {
        let mut store = SubgraphStore::new(1);
        let id1 = store.create(vec![], None);
        let id2 = store.create(vec![], None);
        let id3 = store.create(vec![], None);
        assert_eq!(id1, SubgraphId(1));
        assert_eq!(id2, SubgraphId(2));
        assert_eq!(id3, SubgraphId(3));
        assert_eq!(store.len(), 3);
    }

    // GG-004: Get subgraph
    #[test]
    fn test_get_subgraph() {
        let mut store = SubgraphStore::new(1);
        let id = store.create(vec![(1, PropertyValue::Int64(42))], None);
        let record = store.get(id).expect("found");
        assert_eq!(record.subgraph_id, id);
        assert_eq!(record.properties.len(), 1);
    }

    // GG-004: Get nonexistent subgraph returns None
    #[test]
    fn test_get_nonexistent_subgraph() {
        let store = SubgraphStore::new(1);
        assert!(store.get(SubgraphId(999)).is_none());
    }

    // GG-004: Delete subgraph
    #[test]
    fn test_delete_subgraph() {
        let mut store = SubgraphStore::new(1);
        let id = store.create(vec![], None);
        let deleted = store.delete(id);
        assert!(deleted.is_some());
        assert!(store.get(id).is_none());
        assert_eq!(store.len(), 0);
    }

    // GG-004: Delete nonexistent subgraph returns None
    #[test]
    fn test_delete_nonexistent_subgraph() {
        let mut store = SubgraphStore::new(1);
        assert!(store.delete(SubgraphId(999)).is_none());
    }

    // GG-004: next_id returns next available ID
    #[test]
    fn test_next_id() {
        let mut store = SubgraphStore::new(1);
        assert_eq!(store.next_id(), 1);
        store.create(vec![], None);
        assert_eq!(store.next_id(), 2);
    }

    // GG-003: Default trait
    #[test]
    fn test_subgraph_store_default() {
        let store = SubgraphStore::default();
        assert_eq!(store.len(), 0);
        assert_eq!(store.next_id(), 1);
    }
}
