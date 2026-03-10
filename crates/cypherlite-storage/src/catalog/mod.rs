// Catalog: BiMap-based String <-> u32 mapping for labels, prop keys, rel types

use std::collections::HashMap;

use cypherlite_core::LabelRegistry;
use serde::{Deserialize, Serialize};

/// A namespace for bidirectional String <-> u32 mapping.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct Namespace {
    name_to_id: HashMap<String, u32>,
    id_to_name: HashMap<u32, String>,
    next_id: u32,
}

impl Namespace {
    /// Get the ID for a name, creating a new mapping if it does not exist.
    fn get_or_create(&mut self, name: &str) -> u32 {
        if let Some(&id) = self.name_to_id.get(name) {
            return id;
        }
        let id = self.next_id;
        self.next_id += 1;
        self.name_to_id.insert(name.to_string(), id);
        self.id_to_name.insert(id, name.to_string());
        id
    }

    /// Look up an ID by name.
    fn id_by_name(&self, name: &str) -> Option<u32> {
        self.name_to_id.get(name).copied()
    }

    /// Look up a name by ID.
    fn name_by_id(&self, id: u32) -> Option<&str> {
        self.id_to_name.get(&id).map(|s| s.as_str())
    }
}

/// Catalog stores label, property key, and relationship type mappings.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Catalog {
    labels: Namespace,
    prop_keys: Namespace,
    rel_types: Namespace,
}

impl Catalog {
    /// Serialize the catalog to bytes.
    pub fn save(&self) -> Vec<u8> {
        bincode::serialize(self).expect("Catalog serialization should not fail")
    }

    /// Deserialize the catalog from bytes.
    pub fn load(data: &[u8]) -> cypherlite_core::Result<Self> {
        bincode::deserialize(data)
            .map_err(|e| cypherlite_core::CypherLiteError::SerializationError(e.to_string()))
    }
}

impl LabelRegistry for Catalog {
    fn get_or_create_label(&mut self, name: &str) -> u32 {
        self.labels.get_or_create(name)
    }

    fn label_id(&self, name: &str) -> Option<u32> {
        self.labels.id_by_name(name)
    }

    fn label_name(&self, id: u32) -> Option<&str> {
        self.labels.name_by_id(id)
    }

    fn get_or_create_rel_type(&mut self, name: &str) -> u32 {
        self.rel_types.get_or_create(name)
    }

    fn rel_type_id(&self, name: &str) -> Option<u32> {
        self.rel_types.id_by_name(name)
    }

    fn rel_type_name(&self, id: u32) -> Option<&str> {
        self.rel_types.name_by_id(id)
    }

    fn get_or_create_prop_key(&mut self, name: &str) -> u32 {
        self.prop_keys.get_or_create(name)
    }

    fn prop_key_id(&self, name: &str) -> Option<u32> {
        self.prop_keys.id_by_name(name)
    }

    fn prop_key_name(&self, id: u32) -> Option<&str> {
        self.prop_keys.name_by_id(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // REQ-CATALOG-010: Catalog default is empty
    #[test]
    fn test_catalog_default_is_empty() {
        let cat = Catalog::default();
        assert_eq!(cat.label_id("Person"), None);
        assert_eq!(cat.rel_type_id("KNOWS"), None);
        assert_eq!(cat.prop_key_id("name"), None);
    }

    // REQ-CATALOG-011: Get or create label assigns sequential IDs
    #[test]
    fn test_catalog_get_or_create_label() {
        let mut cat = Catalog::default();
        let id0 = cat.get_or_create_label("Person");
        let id1 = cat.get_or_create_label("Company");
        assert_eq!(id0, 0);
        assert_eq!(id1, 1);
        // Idempotent
        assert_eq!(cat.get_or_create_label("Person"), 0);
    }

    // REQ-CATALOG-012: Reverse lookup by ID
    #[test]
    fn test_catalog_label_name_by_id() {
        let mut cat = Catalog::default();
        let id = cat.get_or_create_label("Person");
        assert_eq!(cat.label_name(id), Some("Person"));
        assert_eq!(cat.label_name(999), None);
    }

    // REQ-CATALOG-013: Relationship type namespace
    #[test]
    fn test_catalog_rel_type_namespace() {
        let mut cat = Catalog::default();
        let id = cat.get_or_create_rel_type("KNOWS");
        assert_eq!(cat.rel_type_id("KNOWS"), Some(id));
        assert_eq!(cat.rel_type_name(id), Some("KNOWS"));
        // Idempotent
        assert_eq!(cat.get_or_create_rel_type("KNOWS"), id);
        // Different type
        let id2 = cat.get_or_create_rel_type("LIKES");
        assert_ne!(id, id2);
    }

    // REQ-CATALOG-014: Property key namespace
    #[test]
    fn test_catalog_prop_key_namespace() {
        let mut cat = Catalog::default();
        let id = cat.get_or_create_prop_key("name");
        assert_eq!(cat.prop_key_id("name"), Some(id));
        assert_eq!(cat.prop_key_name(id), Some("name"));
        assert_eq!(cat.get_or_create_prop_key("name"), id);
        let id2 = cat.get_or_create_prop_key("age");
        assert_ne!(id, id2);
    }

    // REQ-CATALOG-015: Namespaces are independent
    #[test]
    fn test_catalog_namespaces_independent() {
        let mut cat = Catalog::default();
        let label_id = cat.get_or_create_label("name");
        let rel_id = cat.get_or_create_rel_type("name");
        let prop_id = cat.get_or_create_prop_key("name");

        // Each namespace tracks independently
        assert_eq!(cat.label_id("name"), Some(label_id));
        assert_eq!(cat.rel_type_id("name"), Some(rel_id));
        assert_eq!(cat.prop_key_id("name"), Some(prop_id));

        // Reverse lookups stay within namespace
        assert_eq!(cat.label_name(label_id), Some("name"));
        assert_eq!(cat.rel_type_name(rel_id), Some("name"));
        assert_eq!(cat.prop_key_name(prop_id), Some("name"));
    }

    // REQ-CATALOG-016: Clone produces independent copy
    #[test]
    fn test_catalog_clone_is_independent() {
        let mut cat = Catalog::default();
        cat.get_or_create_label("Person");
        let mut cat2 = cat.clone();
        cat2.get_or_create_label("Company");
        // Original unchanged
        assert_eq!(cat.label_id("Company"), None);
        assert_eq!(cat2.label_id("Company"), Some(1));
    }

    // REQ-CATALOG-020: Save and load roundtrip preserves all mappings
    #[test]
    fn test_catalog_save_load_roundtrip() {
        let mut cat = Catalog::default();
        cat.get_or_create_label("Person");
        cat.get_or_create_label("Company");
        cat.get_or_create_rel_type("KNOWS");
        cat.get_or_create_prop_key("name");
        cat.get_or_create_prop_key("age");

        let bytes = cat.save();
        let loaded = Catalog::load(&bytes).expect("load");

        assert_eq!(loaded.label_id("Person"), Some(0));
        assert_eq!(loaded.label_id("Company"), Some(1));
        assert_eq!(loaded.label_name(0), Some("Person"));
        assert_eq!(loaded.label_name(1), Some("Company"));
        assert_eq!(loaded.rel_type_id("KNOWS"), Some(0));
        assert_eq!(loaded.rel_type_name(0), Some("KNOWS"));
        assert_eq!(loaded.prop_key_id("name"), Some(0));
        assert_eq!(loaded.prop_key_id("age"), Some(1));
        assert_eq!(loaded.prop_key_name(0), Some("name"));
        assert_eq!(loaded.prop_key_name(1), Some("age"));
    }

    // REQ-CATALOG-021: Empty catalog roundtrip
    #[test]
    fn test_catalog_save_load_empty() {
        let cat = Catalog::default();
        let bytes = cat.save();
        let loaded = Catalog::load(&bytes).expect("load");
        assert_eq!(loaded.label_id("anything"), None);
    }

    // REQ-CATALOG-022: Load from corrupted data returns error
    #[test]
    fn test_catalog_load_corrupted_data() {
        let result = Catalog::load(&[0xFF, 0xFF, 0xFF]);
        assert!(result.is_err());
    }

    // REQ-CATALOG-023: Loaded catalog supports continued use
    #[test]
    fn test_catalog_loaded_continues_id_sequence() {
        let mut cat = Catalog::default();
        cat.get_or_create_label("Person"); // id=0
        cat.get_or_create_label("Company"); // id=1

        let bytes = cat.save();
        let mut loaded = Catalog::load(&bytes).expect("load");

        // New label should get next ID
        let id = loaded.get_or_create_label("City");
        assert_eq!(id, 2);
        // Existing labels still work
        assert_eq!(loaded.get_or_create_label("Person"), 0);
    }
}
