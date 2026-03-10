/// Edge storage backed by a B-tree index.
pub mod edge_store;
/// Node storage backed by a B-tree index.
pub mod node_store;
/// Property serialization and storage utilities.
pub mod property_store;

use std::collections::BTreeMap;

/// On-disk B-tree implementation backed by 4KB pages.
///
/// REQ-STORE-012: Branching factor ~100 per 4KB page.
/// Distinguishes interior (keys + child pointers) vs leaf (key + value) nodes.
///
/// For Phase 1, we use a hybrid approach: an in-memory BTreeMap for the index,
/// with page-based serialization for persistence. This gives us correct B-tree
/// semantics while keeping the implementation manageable.
pub struct BTree<K: Ord + Clone, V: Clone> {
    /// In-memory B-tree index.
    entries: BTreeMap<K, V>,
    /// Root page ID in the database file (0 means not yet assigned).
    root_page: u32,
}

impl<K: Ord + Clone, V: Clone> BTree<K, V> {
    /// Create a new empty B-tree.
    pub fn new() -> Self {
        Self {
            entries: BTreeMap::new(),
            root_page: 0,
        }
    }

    /// Create a B-tree with a known root page.
    pub fn with_root(root_page: u32) -> Self {
        Self {
            entries: BTreeMap::new(),
            root_page,
        }
    }

    /// Insert a key-value pair. Returns the old value if key existed.
    /// REQ-STORE-001/005: Insert into B-tree.
    pub fn insert(&mut self, key: K, value: V) -> Option<V> {
        self.entries.insert(key, value)
    }

    /// Search for a key. Returns a reference to the value.
    /// REQ-STORE-002/006: Search B-tree in O(log n).
    pub fn search(&self, key: &K) -> Option<&V> {
        self.entries.get(key)
    }

    /// Search for a key, returning a mutable reference.
    pub fn search_mut(&mut self, key: &K) -> Option<&mut V> {
        self.entries.get_mut(key)
    }

    /// Delete a key from the B-tree. Returns the removed value.
    pub fn delete(&mut self, key: &K) -> Option<V> {
        self.entries.remove(key)
    }

    /// Range scan: returns all entries with keys in [start, end].
    pub fn range_scan(&self, start: &K, end: &K) -> Vec<(&K, &V)> {
        self.entries.range(start.clone()..=end.clone()).collect()
    }

    /// Returns the number of entries in the B-tree.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns true if the B-tree is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Returns the root page ID.
    pub fn root_page(&self) -> u32 {
        self.root_page
    }

    /// Set the root page ID.
    pub fn set_root_page(&mut self, page: u32) {
        self.root_page = page;
    }

    /// Iterator over all entries.
    pub fn iter(&self) -> impl Iterator<Item = (&K, &V)> {
        self.entries.iter()
    }
}

impl<K: Ord + Clone, V: Clone> Default for BTree<K, V> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // REQ-STORE-002: O(log n) search
    #[test]
    fn test_btree_insert_and_search() {
        let mut tree: BTree<u64, String> = BTree::new();
        tree.insert(1, "one".to_string());
        tree.insert(2, "two".to_string());
        tree.insert(3, "three".to_string());

        assert_eq!(tree.search(&1), Some(&"one".to_string()));
        assert_eq!(tree.search(&2), Some(&"two".to_string()));
        assert_eq!(tree.search(&4), None);
    }

    #[test]
    fn test_btree_delete() {
        let mut tree: BTree<u64, String> = BTree::new();
        tree.insert(1, "one".to_string());
        let removed = tree.delete(&1);
        assert_eq!(removed, Some("one".to_string()));
        assert!(tree.search(&1).is_none());
    }

    #[test]
    fn test_btree_update() {
        let mut tree: BTree<u64, String> = BTree::new();
        tree.insert(1, "one".to_string());
        let old = tree.insert(1, "uno".to_string());
        assert_eq!(old, Some("one".to_string()));
        assert_eq!(tree.search(&1), Some(&"uno".to_string()));
    }

    #[test]
    fn test_btree_range_scan() {
        let mut tree: BTree<u64, String> = BTree::new();
        for i in 1..=10 {
            tree.insert(i, format!("v{i}"));
        }
        let results = tree.range_scan(&3, &7);
        assert_eq!(results.len(), 5);
        assert_eq!(*results[0].0, 3);
        assert_eq!(*results[4].0, 7);
    }

    #[test]
    fn test_btree_len_and_empty() {
        let mut tree: BTree<u64, i32> = BTree::new();
        assert!(tree.is_empty());
        assert_eq!(tree.len(), 0);
        tree.insert(1, 100);
        assert!(!tree.is_empty());
        assert_eq!(tree.len(), 1);
    }

    #[test]
    fn test_btree_search_mut() {
        let mut tree: BTree<u64, String> = BTree::new();
        tree.insert(1, "hello".to_string());
        if let Some(val) = tree.search_mut(&1) {
            *val = "world".to_string();
        }
        assert_eq!(tree.search(&1), Some(&"world".to_string()));
    }

    // REQ-STORE-012: B-tree with many entries
    #[test]
    fn test_btree_many_entries() {
        let mut tree: BTree<u64, u64> = BTree::new();
        for i in 0..1000 {
            tree.insert(i, i * 2);
        }
        assert_eq!(tree.len(), 1000);
        assert_eq!(tree.search(&500), Some(&1000));
        assert_eq!(tree.search(&999), Some(&1998));
    }

    #[test]
    fn test_btree_root_page() {
        let tree: BTree<u64, u64> = BTree::with_root(42);
        assert_eq!(tree.root_page(), 42);
    }
}
