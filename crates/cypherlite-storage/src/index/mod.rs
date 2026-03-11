// Property index infrastructure for CypherLite.
//
// Provides BTreeMap-backed in-memory property indexes that speed up
// node lookups by (label_id, prop_key_id, value).

use std::collections::BTreeMap;
use std::collections::HashMap;

use cypherlite_core::{NodeId, PropertyValue};
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// PropertyValueKey: Ord wrapper around PropertyValue
// ---------------------------------------------------------------------------

/// An ordered wrapper around `PropertyValue` so it can be used as a BTreeMap key.
///
/// Ordering: Null < Bool(false) < Bool(true) < Int64 < Float64 < String < Bytes < Array.
/// For Float64, total ordering is applied: NaN == NaN, -0.0 < +0.0.
#[derive(Debug, Clone)]
pub struct PropertyValueKey(pub PropertyValue);

impl PartialEq for PropertyValueKey {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == std::cmp::Ordering::Equal
    }
}

impl Eq for PropertyValueKey {}

impl PropertyValueKey {
    /// Returns a sort-stable discriminant for the variant.
    fn discriminant(&self) -> u8 {
        self.0.type_tag()
    }
}

impl PartialOrd for PropertyValueKey {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for PropertyValueKey {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        use std::cmp::Ordering;
        let d1 = self.discriminant();
        let d2 = other.discriminant();
        if d1 != d2 {
            return d1.cmp(&d2);
        }
        match (&self.0, &other.0) {
            (PropertyValue::Null, PropertyValue::Null) => Ordering::Equal,
            (PropertyValue::Bool(a), PropertyValue::Bool(b)) => a.cmp(b),
            (PropertyValue::Int64(a), PropertyValue::Int64(b)) => a.cmp(b),
            (PropertyValue::Float64(a), PropertyValue::Float64(b)) => {
                // Total ordering for f64: use to_bits after normalizing NaN and -0.0
                total_cmp_f64(*a, *b)
            }
            (PropertyValue::String(a), PropertyValue::String(b)) => a.cmp(b),
            (PropertyValue::Bytes(a), PropertyValue::Bytes(b)) => a.cmp(b),
            (PropertyValue::Array(a), PropertyValue::Array(b)) => {
                let len_ord = a.len().cmp(&b.len());
                if len_ord != Ordering::Equal {
                    return len_ord;
                }
                for (av, bv) in a.iter().zip(b.iter()) {
                    let kv_a = PropertyValueKey(av.clone());
                    let kv_b = PropertyValueKey(bv.clone());
                    let c = kv_a.cmp(&kv_b);
                    if c != Ordering::Equal {
                        return c;
                    }
                }
                Ordering::Equal
            }
            (PropertyValue::DateTime(a), PropertyValue::DateTime(b)) => a.cmp(b),
            _ => Ordering::Equal, // same discriminant means same variant
        }
    }
}

/// Total ordering for f64 matching IEEE 754 totalOrder.
fn total_cmp_f64(a: f64, b: f64) -> std::cmp::Ordering {
    // Use the standard library total_cmp (stabilized in Rust 1.62)
    a.total_cmp(&b)
}

// ---------------------------------------------------------------------------
// IndexDefinition
// ---------------------------------------------------------------------------

/// Metadata describing a property index.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IndexDefinition {
    /// User-visible name of this index.
    pub name: String,
    /// The label ID this index applies to.
    pub label_id: u32,
    /// The property key ID this index covers.
    pub prop_key_id: u32,
}

// ---------------------------------------------------------------------------
// PropertyIndex
// ---------------------------------------------------------------------------

/// A BTreeMap-backed property index mapping property values to node IDs.
#[derive(Debug, Clone, Default)]
pub struct PropertyIndex {
    tree: BTreeMap<PropertyValueKey, Vec<NodeId>>,
}

impl PropertyIndex {
    /// Create a new empty property index.
    pub fn new() -> Self {
        Self {
            tree: BTreeMap::new(),
        }
    }

    /// Insert a (value, node_id) pair into the index.
    pub fn insert(&mut self, value: &PropertyValue, node_id: NodeId) {
        let key = PropertyValueKey(value.clone());
        self.tree.entry(key).or_default().push(node_id);
    }

    /// Remove a (value, node_id) pair from the index.
    pub fn remove(&mut self, value: &PropertyValue, node_id: NodeId) {
        let key = PropertyValueKey(value.clone());
        if let Some(ids) = self.tree.get_mut(&key) {
            ids.retain(|id| *id != node_id);
            if ids.is_empty() {
                self.tree.remove(&key);
            }
        }
    }

    /// Look up all node IDs with the exact given value.
    pub fn lookup(&self, value: &PropertyValue) -> Vec<NodeId> {
        let key = PropertyValueKey(value.clone());
        self.tree.get(&key).cloned().unwrap_or_default()
    }

    /// Range query: return all node IDs whose indexed value is in [min, max] (inclusive).
    pub fn range(
        &self,
        min: &PropertyValue,
        max: &PropertyValue,
    ) -> Vec<NodeId> {
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
// IndexManager
// ---------------------------------------------------------------------------

/// Manages all property indexes for a storage engine.
#[derive(Debug, Clone, Default)]
pub struct IndexManager {
    /// Map from index name to (definition, property_index).
    indexes: HashMap<String, (IndexDefinition, PropertyIndex)>,
}

impl IndexManager {
    /// Create a new empty index manager.
    pub fn new() -> Self {
        Self {
            indexes: HashMap::new(),
        }
    }

    /// Create a new index. Returns error if an index with the same name already exists.
    pub fn create_index(
        &mut self,
        name: String,
        label_id: u32,
        prop_key_id: u32,
    ) -> cypherlite_core::Result<()> {
        if self.indexes.contains_key(&name) {
            return Err(cypherlite_core::CypherLiteError::ConstraintViolation(
                format!("index '{}' already exists", name),
            ));
        }
        let def = IndexDefinition {
            name: name.clone(),
            label_id,
            prop_key_id,
        };
        self.indexes.insert(name, (def, PropertyIndex::new()));
        Ok(())
    }

    /// Drop an index by name. Returns error if the index does not exist.
    pub fn drop_index(&mut self, name: &str) -> cypherlite_core::Result<()> {
        if self.indexes.remove(name).is_none() {
            return Err(cypherlite_core::CypherLiteError::ConstraintViolation(
                format!("index '{}' does not exist", name),
            ));
        }
        Ok(())
    }

    /// Find an index by (label_id, prop_key_id) if one exists.
    pub fn find_index(&self, label_id: u32, prop_key_id: u32) -> Option<&PropertyIndex> {
        self.indexes
            .values()
            .find(|(def, _)| def.label_id == label_id && def.prop_key_id == prop_key_id)
            .map(|(_, idx)| idx)
    }

    /// Find a mutable index by (label_id, prop_key_id) if one exists.
    pub fn find_index_mut(
        &mut self,
        label_id: u32,
        prop_key_id: u32,
    ) -> Option<&mut PropertyIndex> {
        self.indexes
            .values_mut()
            .find(|(def, _)| def.label_id == label_id && def.prop_key_id == prop_key_id)
            .map(|(_, idx)| idx)
    }

    /// Get all index definitions.
    pub fn definitions(&self) -> Vec<&IndexDefinition> {
        self.indexes.values().map(|(def, _)| def).collect()
    }

    /// Get a mutable reference to an index by name.
    pub fn get_index_mut(&mut self, name: &str) -> Option<&mut PropertyIndex> {
        self.indexes.get_mut(name).map(|(_, idx)| idx)
    }

    /// Iterate over all indexes as (definition, property_index) pairs.
    pub fn iter(&self) -> impl Iterator<Item = (&IndexDefinition, &PropertyIndex)> {
        self.indexes.values().map(|(def, idx)| (def, idx))
    }

    /// Iterate mutably over all indexes.
    pub fn iter_mut(&mut self) -> impl Iterator<Item = (&IndexDefinition, &mut PropertyIndex)> {
        self.indexes.values_mut().map(|(def, idx)| (def as &IndexDefinition, idx))
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use cypherlite_core::PropertyValue;

    // ======================================================================
    // TASK-091: IndexDefinition and IndexManager structure
    // ======================================================================

    #[test]
    fn test_index_definition_creation() {
        let def = IndexDefinition {
            name: "idx_person_name".to_string(),
            label_id: 0,
            prop_key_id: 1,
        };
        assert_eq!(def.name, "idx_person_name");
        assert_eq!(def.label_id, 0);
        assert_eq!(def.prop_key_id, 1);
    }

    #[test]
    fn test_index_definition_serde_roundtrip() {
        let def = IndexDefinition {
            name: "idx_test".to_string(),
            label_id: 5,
            prop_key_id: 10,
        };
        let bytes = bincode::serialize(&def).expect("serialize");
        let loaded: IndexDefinition = bincode::deserialize(&bytes).expect("deserialize");
        assert_eq!(def, loaded);
    }

    #[test]
    fn test_index_manager_empty() {
        let mgr = IndexManager::new();
        assert!(mgr.definitions().is_empty());
    }

    // ======================================================================
    // TASK-092: PropertyIndex with BTreeMap
    // ======================================================================

    #[test]
    fn test_property_value_key_ord_null() {
        let a = PropertyValueKey(PropertyValue::Null);
        let b = PropertyValueKey(PropertyValue::Null);
        assert_eq!(a.cmp(&b), std::cmp::Ordering::Equal);
    }

    #[test]
    fn test_property_value_key_ord_int64() {
        let a = PropertyValueKey(PropertyValue::Int64(1));
        let b = PropertyValueKey(PropertyValue::Int64(2));
        assert!(a < b);
    }

    #[test]
    fn test_property_value_key_ord_float64_nan() {
        let a = PropertyValueKey(PropertyValue::Float64(f64::NAN));
        let b = PropertyValueKey(PropertyValue::Float64(f64::NAN));
        assert_eq!(a.cmp(&b), std::cmp::Ordering::Equal);
    }

    #[test]
    fn test_property_value_key_ord_cross_type() {
        // Null < Bool < Int64 < Float64 < String
        let null_k = PropertyValueKey(PropertyValue::Null);
        let bool_k = PropertyValueKey(PropertyValue::Bool(true));
        let int_k = PropertyValueKey(PropertyValue::Int64(0));
        let float_k = PropertyValueKey(PropertyValue::Float64(0.0));
        let str_k = PropertyValueKey(PropertyValue::String("a".into()));
        assert!(null_k < bool_k);
        assert!(bool_k < int_k);
        assert!(int_k < float_k);
        assert!(float_k < str_k);
    }

    #[test]
    fn test_property_value_key_ord_string() {
        let a = PropertyValueKey(PropertyValue::String("alpha".into()));
        let b = PropertyValueKey(PropertyValue::String("beta".into()));
        assert!(a < b);
    }

    #[test]
    fn test_property_index_insert_and_lookup() {
        let mut idx = PropertyIndex::new();
        let val = PropertyValue::String("Alice".into());
        idx.insert(&val, NodeId(1));
        idx.insert(&val, NodeId(2));

        let result = idx.lookup(&val);
        assert_eq!(result, vec![NodeId(1), NodeId(2)]);
    }

    #[test]
    fn test_property_index_lookup_empty() {
        let idx = PropertyIndex::new();
        let result = idx.lookup(&PropertyValue::Int64(42));
        assert!(result.is_empty());
    }

    #[test]
    fn test_property_index_remove() {
        let mut idx = PropertyIndex::new();
        let val = PropertyValue::Int64(100);
        idx.insert(&val, NodeId(1));
        idx.insert(&val, NodeId(2));
        idx.remove(&val, NodeId(1));

        let result = idx.lookup(&val);
        assert_eq!(result, vec![NodeId(2)]);
    }

    #[test]
    fn test_property_index_remove_last_cleans_entry() {
        let mut idx = PropertyIndex::new();
        let val = PropertyValue::Int64(100);
        idx.insert(&val, NodeId(1));
        idx.remove(&val, NodeId(1));

        let result = idx.lookup(&val);
        assert!(result.is_empty());
        // Verify the entry is truly removed from the BTreeMap
        assert!(idx.tree.is_empty());
    }

    #[test]
    fn test_property_index_range_query() {
        let mut idx = PropertyIndex::new();
        for i in 1..=10 {
            idx.insert(&PropertyValue::Int64(i), NodeId(i as u64));
        }

        // Range [3, 7] should return nodes 3, 4, 5, 6, 7
        let result = idx.range(&PropertyValue::Int64(3), &PropertyValue::Int64(7));
        let mut ids: Vec<u64> = result.iter().map(|n| n.0).collect();
        ids.sort();
        assert_eq!(ids, vec![3, 4, 5, 6, 7]);
    }

    #[test]
    fn test_property_index_range_empty() {
        let mut idx = PropertyIndex::new();
        idx.insert(&PropertyValue::Int64(1), NodeId(1));
        idx.insert(&PropertyValue::Int64(10), NodeId(10));

        // Range [5, 8] should be empty
        let result = idx.range(&PropertyValue::Int64(5), &PropertyValue::Int64(8));
        assert!(result.is_empty());
    }

    #[test]
    fn test_property_index_range_single_value() {
        let mut idx = PropertyIndex::new();
        idx.insert(&PropertyValue::Int64(5), NodeId(1));
        idx.insert(&PropertyValue::Int64(5), NodeId(2));

        let result = idx.range(&PropertyValue::Int64(5), &PropertyValue::Int64(5));
        assert_eq!(result.len(), 2);
    }

    // ======================================================================
    // TASK-093: IndexManager create_index / drop_index
    // ======================================================================

    #[test]
    fn test_index_manager_create_index() {
        let mut mgr = IndexManager::new();
        mgr.create_index("idx_person_name".to_string(), 0, 1)
            .expect("create");
        assert_eq!(mgr.definitions().len(), 1);
        assert_eq!(mgr.definitions()[0].name, "idx_person_name");
    }

    #[test]
    fn test_index_manager_create_duplicate_fails() {
        let mut mgr = IndexManager::new();
        mgr.create_index("idx_test".to_string(), 0, 1)
            .expect("create");
        let result = mgr.create_index("idx_test".to_string(), 0, 1);
        assert!(result.is_err());
    }

    #[test]
    fn test_index_manager_drop_index() {
        let mut mgr = IndexManager::new();
        mgr.create_index("idx_test".to_string(), 0, 1)
            .expect("create");
        mgr.drop_index("idx_test").expect("drop");
        assert!(mgr.definitions().is_empty());
    }

    #[test]
    fn test_index_manager_drop_nonexistent_fails() {
        let mut mgr = IndexManager::new();
        let result = mgr.drop_index("nonexistent");
        assert!(result.is_err());
    }

    #[test]
    fn test_index_manager_find_index() {
        let mut mgr = IndexManager::new();
        mgr.create_index("idx_test".to_string(), 0, 1)
            .expect("create");

        // Insert a value into the index
        let idx = mgr.find_index_mut(0, 1).expect("should find");
        idx.insert(&PropertyValue::String("Alice".into()), NodeId(1));

        // Lookup via immutable reference
        let idx_ref = mgr.find_index(0, 1).expect("should find");
        let result = idx_ref.lookup(&PropertyValue::String("Alice".into()));
        assert_eq!(result, vec![NodeId(1)]);
    }

    #[test]
    fn test_index_manager_find_index_not_found() {
        let mgr = IndexManager::new();
        assert!(mgr.find_index(99, 99).is_none());
    }

    // ======================================================================
    // U-001/U-004: PropertyValueKey ordering includes DateTime
    // ======================================================================

    #[test]
    fn test_property_value_key_ord_datetime() {
        let a = PropertyValueKey(PropertyValue::DateTime(1_000));
        let b = PropertyValueKey(PropertyValue::DateTime(2_000));
        assert!(a < b);
        assert_eq!(
            PropertyValueKey(PropertyValue::DateTime(1_000)).cmp(
                &PropertyValueKey(PropertyValue::DateTime(1_000))
            ),
            std::cmp::Ordering::Equal
        );
    }

    #[test]
    fn test_property_value_key_datetime_after_array() {
        // DateTime (tag 7) should come after Array (tag 6)
        let array_k = PropertyValueKey(PropertyValue::Array(vec![]));
        let dt_k = PropertyValueKey(PropertyValue::DateTime(0));
        assert!(array_k < dt_k);
    }

    #[test]
    fn test_property_index_datetime_range_query() {
        let mut idx = PropertyIndex::new();
        // Insert DateTime values
        for i in 1..=5 {
            idx.insert(
                &PropertyValue::DateTime(i * 1_000_000),
                NodeId(i as u64),
            );
        }

        // Range [2M, 4M] should return nodes 2, 3, 4
        let result = idx.range(
            &PropertyValue::DateTime(2_000_000),
            &PropertyValue::DateTime(4_000_000),
        );
        let mut ids: Vec<u64> = result.iter().map(|n| n.0).collect();
        ids.sort();
        assert_eq!(ids, vec![2, 3, 4]);
    }
}
