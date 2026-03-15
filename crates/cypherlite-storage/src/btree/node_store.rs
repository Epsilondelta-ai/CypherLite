// NodeStore: Node B-tree CRUD (NodeId -> NodeRecord)

use cypherlite_core::{CypherLiteError, EdgeId, NodeId, NodeRecord, PropertyValue, Result};

use super::BTree;

/// Manages node records using a B-tree index.
///
/// REQ-STORE-001: Allocate unique NodeId, insert into Node B-tree.
/// REQ-STORE-002: Lookup by NodeId in O(log n).
/// REQ-STORE-003: Update node properties.
/// REQ-STORE-004: Delete node (after deleting connected edges).
pub struct NodeStore {
    tree: BTree<u64, NodeRecord>,
    next_id: u64,
}

impl NodeStore {
    /// Create a new empty node store.
    pub fn new(next_id: u64) -> Self {
        Self {
            tree: BTree::new(),
            next_id,
        }
    }

    /// Create a new node with a unique NodeId.
    /// REQ-STORE-001: Allocate unique NodeId(u64), insert NodeRecord.
    pub fn create_node(
        &mut self,
        labels: Vec<u32>,
        properties: Vec<(u32, PropertyValue)>,
    ) -> NodeId {
        let node_id = NodeId(self.next_id);
        self.next_id += 1;
        let record = NodeRecord {
            node_id,
            labels,
            properties,
            next_edge_id: None,
            overflow_page: None,
        };
        self.tree.insert(node_id.0, record);
        node_id
    }

    /// Get a node by its ID.
    /// REQ-STORE-002: Search Node B-tree in O(log n).
    pub fn get_node(&self, node_id: NodeId) -> Option<&NodeRecord> {
        self.tree.search(&node_id.0)
    }

    /// Get a mutable reference to a node.
    pub fn get_node_mut(&mut self, node_id: NodeId) -> Option<&mut NodeRecord> {
        self.tree.search_mut(&node_id.0)
    }

    /// Update a node's properties.
    /// REQ-STORE-003: Find in B-tree, update properties.
    pub fn update_node(
        &mut self,
        node_id: NodeId,
        properties: Vec<(u32, PropertyValue)>,
    ) -> Result<()> {
        match self.tree.search_mut(&node_id.0) {
            Some(record) => {
                record.properties = properties;
                Ok(())
            }
            None => Err(CypherLiteError::NodeNotFound(node_id.0)),
        }
    }

    /// Delete a node from the B-tree.
    /// REQ-STORE-004: Caller must delete all connected edges first.
    pub fn delete_node(&mut self, node_id: NodeId) -> Result<NodeRecord> {
        self.tree
            .delete(&node_id.0)
            .ok_or(CypherLiteError::NodeNotFound(node_id.0))
    }

    /// Returns the next node ID that will be allocated.
    pub fn next_id(&self) -> u64 {
        self.next_id
    }

    /// Returns the number of nodes.
    pub fn len(&self) -> usize {
        self.tree.len()
    }

    /// Returns true if the store is empty.
    pub fn is_empty(&self) -> bool {
        self.tree.is_empty()
    }

    /// Set the adjacency chain head for a node.
    pub fn set_next_edge(&mut self, node_id: NodeId, edge_id: Option<EdgeId>) -> Result<()> {
        match self.tree.search_mut(&node_id.0) {
            Some(record) => {
                record.next_edge_id = edge_id;
                Ok(())
            }
            None => Err(CypherLiteError::NodeNotFound(node_id.0)),
        }
    }

    /// Returns an iterator over all nodes.
    pub fn iter(&self) -> impl Iterator<Item = (&u64, &NodeRecord)> {
        self.tree.iter()
    }

    /// Scan all node records.
    pub fn scan_all(&self) -> Vec<&NodeRecord> {
        self.tree.iter().map(|(_, record)| record).collect()
    }

    /// Insert a record loaded from disk without modifying the next_id counter.
    ///
    /// Used during database startup to rebuild the in-memory B-tree from
    /// persisted data pages (R-PERSIST-031).
    pub fn insert_loaded_record(&mut self, record: NodeRecord) {
        self.tree.insert(record.node_id.0, record);
    }

    /// Scan nodes that contain the given label.
    pub fn scan_by_label(&self, label_id: u32) -> Vec<&NodeRecord> {
        self.tree
            .iter()
            .filter_map(|(_, record)| {
                if record.labels.contains(&label_id) {
                    Some(record)
                } else {
                    None
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // REQ-STORE-001: Create node allocates unique NodeId
    #[test]
    fn test_create_node() {
        let mut store = NodeStore::new(1);
        let id1 = store.create_node(vec![1], vec![(1, PropertyValue::String("Alice".into()))]);
        let id2 = store.create_node(vec![1], vec![(1, PropertyValue::String("Bob".into()))]);
        assert_eq!(id1, NodeId(1));
        assert_eq!(id2, NodeId(2));
        assert_eq!(store.len(), 2);
    }

    // REQ-STORE-002: Lookup by NodeId in O(log n)
    #[test]
    fn test_get_node() {
        let mut store = NodeStore::new(1);
        let id = store.create_node(vec![100], vec![(1, PropertyValue::Int64(42))]);
        let node = store.get_node(id).expect("found");
        assert_eq!(node.node_id, id);
        assert_eq!(node.labels, vec![100]);
    }

    #[test]
    fn test_get_nonexistent_node() {
        let store = NodeStore::new(1);
        assert!(store.get_node(NodeId(999)).is_none());
    }

    // REQ-STORE-003: Update node properties
    #[test]
    fn test_update_node() {
        let mut store = NodeStore::new(1);
        let id = store.create_node(vec![], vec![(1, PropertyValue::Int64(10))]);
        store
            .update_node(id, vec![(1, PropertyValue::Int64(20))])
            .expect("update");
        let node = store.get_node(id).expect("found");
        assert_eq!(node.properties[0].1, PropertyValue::Int64(20));
    }

    #[test]
    fn test_update_nonexistent_node() {
        let mut store = NodeStore::new(1);
        let result = store.update_node(NodeId(999), vec![]);
        assert!(matches!(result, Err(CypherLiteError::NodeNotFound(999))));
    }

    // REQ-STORE-004: Delete node
    #[test]
    fn test_delete_node() {
        let mut store = NodeStore::new(1);
        let id = store.create_node(vec![], vec![]);
        let deleted = store.delete_node(id).expect("delete");
        assert_eq!(deleted.node_id, id);
        assert!(store.get_node(id).is_none());
        assert_eq!(store.len(), 0);
    }

    #[test]
    fn test_delete_nonexistent_node() {
        let mut store = NodeStore::new(1);
        let result = store.delete_node(NodeId(999));
        assert!(matches!(result, Err(CypherLiteError::NodeNotFound(999))));
    }

    #[test]
    fn test_set_next_edge() {
        let mut store = NodeStore::new(1);
        let id = store.create_node(vec![], vec![]);
        assert!(store.get_node(id).expect("f").next_edge_id.is_none());

        store.set_next_edge(id, Some(EdgeId(10))).expect("set");
        assert_eq!(
            store.get_node(id).expect("f").next_edge_id,
            Some(EdgeId(10))
        );
    }

    #[test]
    fn test_next_id_increments() {
        let mut store = NodeStore::new(1);
        assert_eq!(store.next_id(), 1);
        store.create_node(vec![], vec![]);
        assert_eq!(store.next_id(), 2);
        store.create_node(vec![], vec![]);
        assert_eq!(store.next_id(), 3);
    }

    #[test]
    fn test_node_store_empty() {
        let store = NodeStore::new(1);
        assert!(store.is_empty());
        assert_eq!(store.len(), 0);
    }

    // TASK-006: scan_all returns all nodes
    #[test]
    fn test_scan_all_empty() {
        let store = NodeStore::new(1);
        let nodes = store.scan_all();
        assert!(nodes.is_empty());
    }

    #[test]
    fn test_scan_all_returns_all_nodes() {
        let mut store = NodeStore::new(1);
        store.create_node(vec![1], vec![]);
        store.create_node(vec![2], vec![]);
        store.create_node(vec![3], vec![]);
        let nodes = store.scan_all();
        assert_eq!(nodes.len(), 3);
    }

    // TASK-007: scan_by_label filters by label
    #[test]
    fn test_scan_by_label_returns_matching() {
        let mut store = NodeStore::new(1);
        store.create_node(vec![1, 2], vec![]);
        store.create_node(vec![2, 3], vec![]);
        store.create_node(vec![3], vec![]);
        let nodes = store.scan_by_label(2);
        assert_eq!(nodes.len(), 2);
    }

    #[test]
    fn test_scan_by_label_nonexistent_returns_empty() {
        let mut store = NodeStore::new(1);
        store.create_node(vec![1], vec![]);
        let nodes = store.scan_by_label(999);
        assert!(nodes.is_empty());
    }

    // Many nodes for B-tree performance
    #[test]
    fn test_many_nodes() {
        let mut store = NodeStore::new(1);
        for _ in 0..1000 {
            store.create_node(vec![1], vec![]);
        }
        assert_eq!(store.len(), 1000);
        assert!(store.get_node(NodeId(500)).is_some());
    }
}
