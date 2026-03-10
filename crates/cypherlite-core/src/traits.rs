// Transaction and registry trait definitions

/// A read-only view of the database at a specific point in time.
pub trait TransactionView {
    /// Returns the WAL frame number representing this transaction's snapshot.
    fn snapshot_frame(&self) -> u64;
}

/// Registry for resolving string names to u32 IDs (labels, property keys, relationship types).
pub trait LabelRegistry {
    /// Get or create a label ID for the given name.
    fn get_or_create_label(&mut self, name: &str) -> u32;
    /// Look up a label ID by name (returns None if not found).
    fn label_id(&self, name: &str) -> Option<u32>;
    /// Look up a label name by ID (returns None if not found).
    fn label_name(&self, id: u32) -> Option<&str>;

    /// Get or create a relationship type ID.
    fn get_or_create_rel_type(&mut self, name: &str) -> u32;
    /// Look up a relationship type ID by name.
    fn rel_type_id(&self, name: &str) -> Option<u32>;
    /// Look up a relationship type name by ID.
    fn rel_type_name(&self, id: u32) -> Option<&str>;

    /// Get or create a property key ID.
    fn get_or_create_prop_key(&mut self, name: &str) -> u32;
    /// Look up a property key ID by name.
    fn prop_key_id(&self, name: &str) -> Option<u32>;
    /// Look up a property key name by ID.
    fn prop_key_name(&self, id: u32) -> Option<&str>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    struct MockTxView {
        frame: u64,
    }

    impl TransactionView for MockTxView {
        fn snapshot_frame(&self) -> u64 {
            self.frame
        }
    }

    // REQ-TX-001: Read transactions capture snapshot point
    #[test]
    fn test_transaction_view_snapshot_frame() {
        let view = MockTxView { frame: 42 };
        assert_eq!(view.snapshot_frame(), 42);
    }

    #[test]
    fn test_transaction_view_zero_frame() {
        let view = MockTxView { frame: 0 };
        assert_eq!(view.snapshot_frame(), 0);
    }

    // Verify trait is object-safe
    #[test]
    fn test_transaction_view_is_object_safe() {
        let view: Box<dyn TransactionView> = Box::new(MockTxView { frame: 10 });
        assert_eq!(view.snapshot_frame(), 10);
    }

    // -- LabelRegistry tests --

    struct MockRegistry {
        labels: HashMap<String, u32>,
        labels_rev: HashMap<u32, String>,
        label_next: u32,
        rel_types: HashMap<String, u32>,
        rel_types_rev: HashMap<u32, String>,
        rel_next: u32,
        prop_keys: HashMap<String, u32>,
        prop_keys_rev: HashMap<u32, String>,
        prop_next: u32,
    }

    impl MockRegistry {
        fn new() -> Self {
            Self {
                labels: HashMap::new(),
                labels_rev: HashMap::new(),
                label_next: 0,
                rel_types: HashMap::new(),
                rel_types_rev: HashMap::new(),
                rel_next: 0,
                prop_keys: HashMap::new(),
                prop_keys_rev: HashMap::new(),
                prop_next: 0,
            }
        }
    }

    impl LabelRegistry for MockRegistry {
        fn get_or_create_label(&mut self, name: &str) -> u32 {
            if let Some(&id) = self.labels.get(name) {
                return id;
            }
            let id = self.label_next;
            self.label_next += 1;
            self.labels.insert(name.to_string(), id);
            self.labels_rev.insert(id, name.to_string());
            id
        }

        fn label_id(&self, name: &str) -> Option<u32> {
            self.labels.get(name).copied()
        }

        fn label_name(&self, id: u32) -> Option<&str> {
            self.labels_rev.get(&id).map(|s| s.as_str())
        }

        fn get_or_create_rel_type(&mut self, name: &str) -> u32 {
            if let Some(&id) = self.rel_types.get(name) {
                return id;
            }
            let id = self.rel_next;
            self.rel_next += 1;
            self.rel_types.insert(name.to_string(), id);
            self.rel_types_rev.insert(id, name.to_string());
            id
        }

        fn rel_type_id(&self, name: &str) -> Option<u32> {
            self.rel_types.get(name).copied()
        }

        fn rel_type_name(&self, id: u32) -> Option<&str> {
            self.rel_types_rev.get(&id).map(|s| s.as_str())
        }

        fn get_or_create_prop_key(&mut self, name: &str) -> u32 {
            if let Some(&id) = self.prop_keys.get(name) {
                return id;
            }
            let id = self.prop_next;
            self.prop_next += 1;
            self.prop_keys.insert(name.to_string(), id);
            self.prop_keys_rev.insert(id, name.to_string());
            id
        }

        fn prop_key_id(&self, name: &str) -> Option<u32> {
            self.prop_keys.get(name).copied()
        }

        fn prop_key_name(&self, id: u32) -> Option<&str> {
            self.prop_keys_rev.get(&id).map(|s| s.as_str())
        }
    }

    // REQ-CATALOG-001: Get or create label returns stable IDs
    #[test]
    fn test_label_registry_get_or_create_label() {
        let mut reg = MockRegistry::new();
        let id1 = reg.get_or_create_label("Person");
        let id2 = reg.get_or_create_label("Person");
        assert_eq!(id1, id2, "Same name must return same ID");
    }

    #[test]
    fn test_label_registry_different_labels_get_different_ids() {
        let mut reg = MockRegistry::new();
        let id1 = reg.get_or_create_label("Person");
        let id2 = reg.get_or_create_label("Company");
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_label_registry_lookup_by_name() {
        let mut reg = MockRegistry::new();
        assert_eq!(reg.label_id("Person"), None);
        let id = reg.get_or_create_label("Person");
        assert_eq!(reg.label_id("Person"), Some(id));
    }

    #[test]
    fn test_label_registry_lookup_by_id() {
        let mut reg = MockRegistry::new();
        assert_eq!(reg.label_name(0), None);
        let id = reg.get_or_create_label("Person");
        assert_eq!(reg.label_name(id), Some("Person"));
    }

    // REQ-CATALOG-002: Relationship type registry
    #[test]
    fn test_label_registry_rel_types() {
        let mut reg = MockRegistry::new();
        let id = reg.get_or_create_rel_type("KNOWS");
        assert_eq!(reg.rel_type_id("KNOWS"), Some(id));
        assert_eq!(reg.rel_type_name(id), Some("KNOWS"));
        assert_eq!(reg.get_or_create_rel_type("KNOWS"), id);
    }

    // REQ-CATALOG-003: Property key registry
    #[test]
    fn test_label_registry_prop_keys() {
        let mut reg = MockRegistry::new();
        let id = reg.get_or_create_prop_key("name");
        assert_eq!(reg.prop_key_id("name"), Some(id));
        assert_eq!(reg.prop_key_name(id), Some("name"));
        assert_eq!(reg.get_or_create_prop_key("name"), id);
    }

    // REQ-CATALOG-004: Namespaces are independent
    #[test]
    fn test_label_registry_namespaces_are_independent() {
        let mut reg = MockRegistry::new();
        let label_id = reg.get_or_create_label("name");
        let rel_id = reg.get_or_create_rel_type("name");
        let prop_id = reg.get_or_create_prop_key("name");
        // Same name in different namespaces can have same or different IDs,
        // but lookups must stay within their namespace.
        assert_eq!(reg.label_id("name"), Some(label_id));
        assert_eq!(reg.rel_type_id("name"), Some(rel_id));
        assert_eq!(reg.prop_key_id("name"), Some(prop_id));
    }
}
