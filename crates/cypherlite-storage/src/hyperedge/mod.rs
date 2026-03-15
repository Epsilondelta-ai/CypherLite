// HyperEdgeStore: in-memory hyperedge entity storage
//
// HH-003: HyperEdgeStore module
// HH-004: CRUD operations
// HH-005: Integration with StorageEngine

/// Reverse index tracking entity-hyperedge relationships.
pub mod reverse_index;

use cypherlite_core::{GraphEntity, HyperEdgeId, HyperEdgeRecord, PropertyValue};
use std::collections::BTreeMap;

/// In-memory hyperedge store backed by a BTreeMap.
///
/// Stores hyperedge records keyed by their u64 ID.
/// Provides CRUD operations for hyperedge entities.
pub struct HyperEdgeStore {
    /// Storage: hyperedge_id -> HyperEdgeRecord
    records: BTreeMap<u64, HyperEdgeRecord>,
    /// Next available hyperedge ID.
    next_id: u64,
}

impl HyperEdgeStore {
    /// Create a new hyperedge store with the given starting ID.
    pub fn new(start_id: u64) -> Self {
        Self {
            records: BTreeMap::new(),
            next_id: start_id,
        }
    }

    /// Create a new hyperedge with the given type, sources, targets, and properties.
    /// Returns the assigned HyperEdgeId.
    pub fn create(
        &mut self,
        rel_type_id: u32,
        sources: Vec<GraphEntity>,
        targets: Vec<GraphEntity>,
        properties: Vec<(u32, PropertyValue)>,
    ) -> HyperEdgeId {
        let id = HyperEdgeId(self.next_id);
        let record = HyperEdgeRecord {
            id,
            rel_type_id,
            sources,
            targets,
            properties,
        };
        self.records.insert(self.next_id, record);
        self.next_id += 1;
        id
    }

    /// Get a hyperedge record by ID.
    pub fn get(&self, id: HyperEdgeId) -> Option<&HyperEdgeRecord> {
        self.records.get(&id.0)
    }

    /// Delete a hyperedge by ID. Returns the deleted record if found.
    pub fn delete(&mut self, id: HyperEdgeId) -> Option<HyperEdgeRecord> {
        self.records.remove(&id.0)
    }

    /// Returns the next available hyperedge ID.
    pub fn next_id(&self) -> u64 {
        self.next_id
    }

    /// Returns the number of hyperedges stored.
    pub fn len(&self) -> usize {
        self.records.len()
    }

    /// Returns true if the store is empty.
    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    /// Returns an iterator over all hyperedge records.
    pub fn all(&self) -> impl Iterator<Item = &HyperEdgeRecord> {
        self.records.values()
    }

    /// Insert a record that was loaded from persistent storage.
    ///
    /// Updates `next_id` if the loaded record's ID is >= current next_id,
    /// ensuring new IDs won't collide with loaded data.
    pub fn insert_loaded_record(&mut self, record: HyperEdgeRecord) {
        let id = record.id.0;
        self.records.insert(id, record);
        if id >= self.next_id {
            self.next_id = id + 1;
        }
    }
}

impl Default for HyperEdgeStore {
    fn default() -> Self {
        Self::new(1)
    }
}

#[cfg(test)]
mod tests {
    use cypherlite_core::{GraphEntity, HyperEdgeId, NodeId, PropertyValue};

    use super::*;

    // HH-003: HyperEdgeStore creation
    #[test]
    fn test_hyperedge_store_new_is_empty() {
        let store = HyperEdgeStore::new(1);
        assert_eq!(store.len(), 0);
        assert!(store.is_empty());
    }

    // HH-004: Create hyperedge
    #[test]
    fn test_create_hyperedge() {
        let mut store = HyperEdgeStore::new(1);
        let id = store.create(
            1,
            vec![GraphEntity::Node(NodeId(10))],
            vec![GraphEntity::Node(NodeId(20))],
            vec![],
        );
        assert_eq!(id, HyperEdgeId(1));
        assert_eq!(store.len(), 1);
        assert!(!store.is_empty());
    }

    // HH-004: Create multiple hyperedges (incrementing IDs)
    #[test]
    fn test_create_multiple_hyperedges() {
        let mut store = HyperEdgeStore::new(1);
        let id1 = store.create(1, vec![], vec![], vec![]);
        let id2 = store.create(2, vec![], vec![], vec![]);
        let id3 = store.create(3, vec![], vec![], vec![]);
        assert_eq!(id1, HyperEdgeId(1));
        assert_eq!(id2, HyperEdgeId(2));
        assert_eq!(id3, HyperEdgeId(3));
        assert_eq!(store.len(), 3);
    }

    // HH-004: Get hyperedge
    #[test]
    fn test_get_hyperedge() {
        let mut store = HyperEdgeStore::new(1);
        let id = store.create(
            5,
            vec![GraphEntity::Node(NodeId(1))],
            vec![GraphEntity::Node(NodeId(2))],
            vec![(1, PropertyValue::Int64(42))],
        );
        let record = store.get(id).expect("found");
        assert_eq!(record.id, id);
        assert_eq!(record.rel_type_id, 5);
        assert_eq!(record.sources.len(), 1);
        assert_eq!(record.targets.len(), 1);
        assert_eq!(record.properties.len(), 1);
    }

    // HH-004: Get nonexistent hyperedge returns None
    #[test]
    fn test_get_nonexistent_hyperedge() {
        let store = HyperEdgeStore::new(1);
        assert!(store.get(HyperEdgeId(999)).is_none());
    }

    // HH-004: Delete hyperedge
    #[test]
    fn test_delete_hyperedge() {
        let mut store = HyperEdgeStore::new(1);
        let id = store.create(1, vec![], vec![], vec![]);
        let deleted = store.delete(id);
        assert!(deleted.is_some());
        assert!(store.get(id).is_none());
        assert_eq!(store.len(), 0);
    }

    // HH-004: Delete nonexistent hyperedge returns None
    #[test]
    fn test_delete_nonexistent_hyperedge() {
        let mut store = HyperEdgeStore::new(1);
        assert!(store.delete(HyperEdgeId(999)).is_none());
    }

    // HH-004: all() iterator
    #[test]
    fn test_all_iterator() {
        let mut store = HyperEdgeStore::new(1);
        store.create(1, vec![], vec![], vec![]);
        store.create(2, vec![], vec![], vec![]);
        let all: Vec<_> = store.all().collect();
        assert_eq!(all.len(), 2);
    }

    // HH-004: next_id returns next available ID
    #[test]
    fn test_next_id() {
        let mut store = HyperEdgeStore::new(1);
        assert_eq!(store.next_id(), 1);
        store.create(1, vec![], vec![], vec![]);
        assert_eq!(store.next_id(), 2);
    }

    // HH-003: Default trait
    #[test]
    fn test_hyperedge_store_default() {
        let store = HyperEdgeStore::default();
        assert_eq!(store.len(), 0);
        assert_eq!(store.next_id(), 1);
    }
}
