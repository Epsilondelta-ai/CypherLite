// Edge property index infrastructure for CypherLite.
//
// Provides BTreeMap-backed in-memory property indexes that speed up
// edge lookups by (rel_type_id, prop_key_id, value).

use std::collections::HashMap;

use cypherlite_core::EdgeId;
use cypherlite_core::PropertyValue;
use serde::{Deserialize, Serialize};

use super::PropertyValueKey;

// ---------------------------------------------------------------------------
// EdgeIndexDefinition
// ---------------------------------------------------------------------------

/// Metadata describing an edge property index.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EdgeIndexDefinition {
    /// User-visible name of this index.
    pub name: String,
    /// The relationship type ID this index applies to.
    pub rel_type_id: u32,
    /// The property key ID this index covers.
    pub prop_key_id: u32,
}

// ---------------------------------------------------------------------------
// EdgePropertyIndex
// ---------------------------------------------------------------------------

/// A BTreeMap-backed property index mapping property values to edge IDs.
#[derive(Debug, Clone, Default)]
pub struct EdgePropertyIndex {
    tree: std::collections::BTreeMap<PropertyValueKey, Vec<EdgeId>>,
}

impl EdgePropertyIndex {
    /// Create a new empty edge property index.
    pub fn new() -> Self {
        Self {
            tree: std::collections::BTreeMap::new(),
        }
    }

    /// Insert a (value, edge_id) pair into the index.
    pub fn insert(&mut self, value: &PropertyValue, edge_id: EdgeId) {
        let key = PropertyValueKey(value.clone());
        self.tree.entry(key).or_default().push(edge_id);
    }

    /// Remove a (value, edge_id) pair from the index.
    pub fn remove(&mut self, value: &PropertyValue, edge_id: EdgeId) {
        let key = PropertyValueKey(value.clone());
        if let Some(ids) = self.tree.get_mut(&key) {
            ids.retain(|id| *id != edge_id);
            if ids.is_empty() {
                self.tree.remove(&key);
            }
        }
    }

    /// Look up all edge IDs with the exact given value.
    pub fn lookup(&self, value: &PropertyValue) -> Vec<EdgeId> {
        let key = PropertyValueKey(value.clone());
        self.tree.get(&key).cloned().unwrap_or_default()
    }

    /// Range query: return all edge IDs whose indexed value is in [min, max] (inclusive).
    pub fn range(&self, min: &PropertyValue, max: &PropertyValue) -> Vec<EdgeId> {
        let min_key = PropertyValueKey(min.clone());
        let max_key = PropertyValueKey(max.clone());
        let mut result = Vec::new();
        for (_key, ids) in self.tree.range(min_key..=max_key) {
            result.extend(ids);
        }
        result
    }
}

// ---------------------------------------------------------------------------
// EdgeIndexManager
// ---------------------------------------------------------------------------

/// Manages all edge property indexes for a storage engine.
#[derive(Debug, Clone, Default)]
pub struct EdgeIndexManager {
    /// Map from index name to (definition, edge_property_index).
    indexes: HashMap<String, (EdgeIndexDefinition, EdgePropertyIndex)>,
}

impl EdgeIndexManager {
    /// Create a new empty edge index manager.
    pub fn new() -> Self {
        Self {
            indexes: HashMap::new(),
        }
    }

    /// Create a new edge index. Returns error if an index with the same name already exists.
    pub fn create_index(
        &mut self,
        name: String,
        rel_type_id: u32,
        prop_key_id: u32,
    ) -> cypherlite_core::Result<()> {
        if self.indexes.contains_key(&name) {
            return Err(cypherlite_core::CypherLiteError::ConstraintViolation(
                format!("edge index '{}' already exists", name),
            ));
        }
        let def = EdgeIndexDefinition {
            name: name.clone(),
            rel_type_id,
            prop_key_id,
        };
        self.indexes.insert(name, (def, EdgePropertyIndex::new()));
        Ok(())
    }

    /// Drop an edge index by name. Returns error if the index does not exist.
    pub fn drop_index(&mut self, name: &str) -> cypherlite_core::Result<()> {
        if self.indexes.remove(name).is_none() {
            return Err(cypherlite_core::CypherLiteError::ConstraintViolation(
                format!("edge index '{}' does not exist", name),
            ));
        }
        Ok(())
    }

    /// Find an edge index by (rel_type_id, prop_key_id) if one exists.
    pub fn find_index(&self, rel_type_id: u32, prop_key_id: u32) -> Option<&EdgePropertyIndex> {
        self.indexes
            .values()
            .find(|(def, _)| def.rel_type_id == rel_type_id && def.prop_key_id == prop_key_id)
            .map(|(_, idx)| idx)
    }

    /// Find a mutable edge index by (rel_type_id, prop_key_id) if one exists.
    pub fn find_index_mut(
        &mut self,
        rel_type_id: u32,
        prop_key_id: u32,
    ) -> Option<&mut EdgePropertyIndex> {
        self.indexes
            .values_mut()
            .find(|(def, _)| def.rel_type_id == rel_type_id && def.prop_key_id == prop_key_id)
            .map(|(_, idx)| idx)
    }

    /// Get all edge index definitions.
    pub fn definitions(&self) -> Vec<&EdgeIndexDefinition> {
        self.indexes.values().map(|(def, _)| def).collect()
    }

    /// Iterate over all edge indexes as (definition, edge_property_index) pairs.
    pub fn iter(&self) -> impl Iterator<Item = (&EdgeIndexDefinition, &EdgePropertyIndex)> {
        self.indexes.values().map(|(def, idx)| (def, idx))
    }

    /// Iterate mutably over all edge indexes.
    pub fn iter_mut(
        &mut self,
    ) -> impl Iterator<Item = (&EdgeIndexDefinition, &mut EdgePropertyIndex)> {
        self.indexes
            .values_mut()
            .map(|(def, idx)| (def as &EdgeIndexDefinition, idx))
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use cypherlite_core::PropertyValue;

    // CC-T1: EdgeIndexDefinition creation
    #[test]
    fn test_edge_index_definition_creation() {
        let def = EdgeIndexDefinition {
            name: "idx_knows_since".to_string(),
            rel_type_id: 0,
            prop_key_id: 1,
        };
        assert_eq!(def.name, "idx_knows_since");
        assert_eq!(def.rel_type_id, 0);
        assert_eq!(def.prop_key_id, 1);
    }

    #[test]
    fn test_edge_index_definition_serde_roundtrip() {
        let def = EdgeIndexDefinition {
            name: "idx_test".to_string(),
            rel_type_id: 5,
            prop_key_id: 10,
        };
        let bytes = bincode::serialize(&def).expect("serialize");
        let loaded: EdgeIndexDefinition = bincode::deserialize(&bytes).expect("deserialize");
        assert_eq!(def, loaded);
    }

    // CC-T1: EdgeIndexManager empty
    #[test]
    fn test_edge_index_manager_empty() {
        let mgr = EdgeIndexManager::new();
        assert!(mgr.definitions().is_empty());
    }

    // CC-T1: EdgePropertyIndex insert and lookup
    #[test]
    fn test_edge_property_index_insert_and_lookup() {
        let mut idx = EdgePropertyIndex::new();
        let val = PropertyValue::String("2024-01-01".into());
        idx.insert(&val, EdgeId(1));
        idx.insert(&val, EdgeId(2));

        let result = idx.lookup(&val);
        assert_eq!(result, vec![EdgeId(1), EdgeId(2)]);
    }

    #[test]
    fn test_edge_property_index_lookup_empty() {
        let idx = EdgePropertyIndex::new();
        let result = idx.lookup(&PropertyValue::Int64(42));
        assert!(result.is_empty());
    }

    #[test]
    fn test_edge_property_index_remove() {
        let mut idx = EdgePropertyIndex::new();
        let val = PropertyValue::Int64(100);
        idx.insert(&val, EdgeId(1));
        idx.insert(&val, EdgeId(2));
        idx.remove(&val, EdgeId(1));

        let result = idx.lookup(&val);
        assert_eq!(result, vec![EdgeId(2)]);
    }

    #[test]
    fn test_edge_property_index_remove_last_cleans_entry() {
        let mut idx = EdgePropertyIndex::new();
        let val = PropertyValue::Int64(100);
        idx.insert(&val, EdgeId(1));
        idx.remove(&val, EdgeId(1));
        assert!(idx.lookup(&val).is_empty());
        assert!(idx.tree.is_empty());
    }

    #[test]
    fn test_edge_property_index_range_query() {
        let mut idx = EdgePropertyIndex::new();
        for i in 1..=10 {
            idx.insert(&PropertyValue::Int64(i), EdgeId(i as u64));
        }
        let result = idx.range(&PropertyValue::Int64(3), &PropertyValue::Int64(7));
        let mut ids: Vec<u64> = result.iter().map(|e| e.0).collect();
        ids.sort();
        assert_eq!(ids, vec![3, 4, 5, 6, 7]);
    }

    // CC-T1: EdgeIndexManager create/drop
    #[test]
    fn test_edge_index_manager_create_index() {
        let mut mgr = EdgeIndexManager::new();
        mgr.create_index("idx_knows_since".to_string(), 0, 1)
            .expect("create");
        assert_eq!(mgr.definitions().len(), 1);
        assert_eq!(mgr.definitions()[0].name, "idx_knows_since");
    }

    #[test]
    fn test_edge_index_manager_create_duplicate_fails() {
        let mut mgr = EdgeIndexManager::new();
        mgr.create_index("idx_test".to_string(), 0, 1)
            .expect("create");
        let result = mgr.create_index("idx_test".to_string(), 0, 1);
        assert!(result.is_err());
    }

    #[test]
    fn test_edge_index_manager_drop_index() {
        let mut mgr = EdgeIndexManager::new();
        mgr.create_index("idx_test".to_string(), 0, 1)
            .expect("create");
        mgr.drop_index("idx_test").expect("drop");
        assert!(mgr.definitions().is_empty());
    }

    #[test]
    fn test_edge_index_manager_drop_nonexistent_fails() {
        let mut mgr = EdgeIndexManager::new();
        let result = mgr.drop_index("nonexistent");
        assert!(result.is_err());
    }

    #[test]
    fn test_edge_index_manager_find_index() {
        let mut mgr = EdgeIndexManager::new();
        mgr.create_index("idx_test".to_string(), 0, 1)
            .expect("create");

        let idx = mgr.find_index_mut(0, 1).expect("should find");
        idx.insert(&PropertyValue::String("val".into()), EdgeId(1));

        let idx_ref = mgr.find_index(0, 1).expect("should find");
        let result = idx_ref.lookup(&PropertyValue::String("val".into()));
        assert_eq!(result, vec![EdgeId(1)]);
    }

    #[test]
    fn test_edge_index_manager_find_index_not_found() {
        let mgr = EdgeIndexManager::new();
        assert!(mgr.find_index(99, 99).is_none());
    }

    // DateTime range on edge index
    #[test]
    fn test_edge_property_index_datetime_range() {
        let mut idx = EdgePropertyIndex::new();
        for i in 1..=5 {
            idx.insert(&PropertyValue::DateTime(i * 1_000_000), EdgeId(i as u64));
        }
        let result = idx.range(
            &PropertyValue::DateTime(2_000_000),
            &PropertyValue::DateTime(4_000_000),
        );
        let mut ids: Vec<u64> = result.iter().map(|e| e.0).collect();
        ids.sort();
        assert_eq!(ids, vec![2, 3, 4]);
    }
}
