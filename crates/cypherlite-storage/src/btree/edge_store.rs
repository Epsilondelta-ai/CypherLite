// EdgeStore: Edge B-tree CRUD + Index-Free Adjacency chain

use cypherlite_core::{
    CypherLiteError, Direction, EdgeId, NodeId, PropertyValue, RelationshipRecord, Result,
};

use super::node_store::NodeStore;
use super::BTree;

/// Manages edge records using a B-tree index with adjacency chain maintenance.
///
/// REQ-STORE-005: Allocate unique EdgeId, insert RelationshipRecord, update adjacency chains.
/// REQ-STORE-006: Lookup by EdgeId in O(log n).
/// REQ-STORE-007: Walk linked list from node's next_edge_id (Index-Free Adjacency).
/// REQ-STORE-008: Delete edge, update adjacency chain pointers.
pub struct EdgeStore {
    tree: BTree<u64, RelationshipRecord>,
    next_id: u64,
}

impl EdgeStore {
    /// Create a new empty edge store.
    pub fn new(next_id: u64) -> Self {
        Self {
            tree: BTree::new(),
            next_id,
        }
    }

    /// Create a new edge, updating adjacency chains on both nodes.
    /// REQ-STORE-005: Allocate EdgeId, insert, update adjacency chains.
    pub fn create_edge(
        &mut self,
        start_node: NodeId,
        end_node: NodeId,
        rel_type_id: u32,
        properties: Vec<(u32, PropertyValue)>,
        node_store: &mut NodeStore,
    ) -> Result<EdgeId> {
        let edge_id = EdgeId(self.next_id);
        self.next_id += 1;

        // Get current adjacency chain heads
        let start_record = node_store
            .get_node(start_node)
            .ok_or(CypherLiteError::NodeNotFound(start_node.0))?;
        let prev_out_edge = start_record.next_edge_id;

        let end_record = node_store
            .get_node(end_node)
            .ok_or(CypherLiteError::NodeNotFound(end_node.0))?;
        let prev_in_edge = end_record.next_edge_id;

        let record = RelationshipRecord {
            edge_id,
            start_node,
            end_node,
            rel_type_id,
            direction: Direction::Outgoing,
            next_out_edge: prev_out_edge,
            next_in_edge: prev_in_edge,
            properties,
            #[cfg(feature = "subgraph")]
            start_is_subgraph: false,
            #[cfg(feature = "subgraph")]
            end_is_subgraph: false,
        };

        self.tree.insert(edge_id.0, record);

        // Update adjacency chain heads: prepend to both node chains
        node_store.set_next_edge(start_node, Some(edge_id))?;
        if start_node != end_node {
            node_store.set_next_edge(end_node, Some(edge_id))?;
        }

        Ok(edge_id)
    }

    /// Get an edge by its ID.
    /// REQ-STORE-006: Search Edge B-tree in O(log n).
    pub fn get_edge(&self, edge_id: EdgeId) -> Option<&RelationshipRecord> {
        self.tree.search(&edge_id.0)
    }

    /// Get a mutable reference to an edge.
    pub fn get_edge_mut(&mut self, edge_id: EdgeId) -> Option<&mut RelationshipRecord> {
        self.tree.search_mut(&edge_id.0)
    }

    /// Update an edge's properties.
    pub fn update_edge(
        &mut self,
        edge_id: EdgeId,
        properties: Vec<(u32, PropertyValue)>,
    ) -> Result<()> {
        let record = self
            .tree
            .search_mut(&edge_id.0)
            .ok_or(CypherLiteError::EdgeNotFound(edge_id.0))?;
        record.properties = properties;
        Ok(())
    }

    /// Get all edges connected to a node by walking the adjacency chain.
    /// REQ-STORE-007: Walk linked list from node's next_edge_id.
    pub fn get_edges_for_node(
        &self,
        node_id: NodeId,
        _node_store: &NodeStore,
    ) -> Vec<&RelationshipRecord> {
        // Since our adjacency chain uses a simple "head pointer" approach,
        // we need to walk through all edges and filter by node involvement.
        // In a full implementation, we'd follow the chain pointers.
        self.tree
            .iter()
            .filter_map(|(_, record)| {
                if record.start_node == node_id || record.end_node == node_id {
                    Some(record)
                } else {
                    None
                }
            })
            .collect()
    }

    /// Delete an edge, updating adjacency chains.
    /// REQ-STORE-008: Remove from Edge B-tree, update adjacency chain pointers.
    pub fn delete_edge(
        &mut self,
        edge_id: EdgeId,
        node_store: &mut NodeStore,
    ) -> Result<RelationshipRecord> {
        let record = self
            .tree
            .delete(&edge_id.0)
            .ok_or(CypherLiteError::EdgeNotFound(edge_id.0))?;

        // Update adjacency chains: if the deleted edge was the head,
        // set the next edge as the new head
        if let Some(start_node) = node_store.get_node(record.start_node) {
            if start_node.next_edge_id == Some(edge_id) {
                let next = record.next_out_edge;
                node_store.set_next_edge(record.start_node, next)?;
            }
        }

        if record.start_node != record.end_node {
            if let Some(end_node) = node_store.get_node(record.end_node) {
                if end_node.next_edge_id == Some(edge_id) {
                    let next = record.next_in_edge;
                    node_store.set_next_edge(record.end_node, next)?;
                }
            }
        }

        Ok(record)
    }

    /// Delete all edges connected to a node.
    /// Used by NodeStore when deleting a node (REQ-STORE-004).
    pub fn delete_edges_for_node(
        &mut self,
        node_id: NodeId,
        node_store: &mut NodeStore,
    ) -> Result<Vec<RelationshipRecord>> {
        // Collect edge IDs first to avoid borrow issues
        let edge_ids: Vec<EdgeId> = self
            .tree
            .iter()
            .filter(|(_, record)| record.start_node == node_id || record.end_node == node_id)
            .map(|(_, record)| record.edge_id)
            .collect();

        let mut deleted = Vec::new();
        for eid in edge_ids {
            if let Ok(record) = self.delete_edge(eid, node_store) {
                deleted.push(record);
            }
        }
        Ok(deleted)
    }

    /// Returns the next edge ID that will be allocated.
    pub fn next_id(&self) -> u64 {
        self.next_id
    }

    /// Returns the number of edges.
    pub fn len(&self) -> usize {
        self.tree.len()
    }

    /// Returns true if the store is empty.
    pub fn is_empty(&self) -> bool {
        self.tree.is_empty()
    }

    /// Returns an iterator over all edges.
    pub fn iter(&self) -> impl Iterator<Item = (&u64, &RelationshipRecord)> {
        self.tree.iter()
    }

    /// Scan edges that match the given relationship type.
    pub fn scan_by_type(&self, type_id: u32) -> Vec<&RelationshipRecord> {
        self.tree
            .iter()
            .filter_map(|(_, record)| {
                if record.rel_type_id == type_id {
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

    fn setup() -> (NodeStore, EdgeStore) {
        (NodeStore::new(1), EdgeStore::new(1))
    }

    // REQ-STORE-005: Create edge with unique EdgeId
    #[test]
    fn test_create_edge() {
        let (mut ns, mut es) = setup();
        let n1 = ns.create_node(vec![], vec![]);
        let n2 = ns.create_node(vec![], vec![]);

        let e1 = es.create_edge(n1, n2, 1, vec![], &mut ns).expect("create");
        assert_eq!(e1, EdgeId(1));
        assert_eq!(es.len(), 1);
    }

    // REQ-STORE-005: Adjacency chain updated on creation
    #[test]
    fn test_create_edge_updates_adjacency() {
        let (mut ns, mut es) = setup();
        let n1 = ns.create_node(vec![], vec![]);
        let n2 = ns.create_node(vec![], vec![]);

        let e1 = es.create_edge(n1, n2, 1, vec![], &mut ns).expect("e1");
        assert_eq!(ns.get_node(n1).expect("n1").next_edge_id, Some(e1));
        assert_eq!(ns.get_node(n2).expect("n2").next_edge_id, Some(e1));
    }

    // REQ-STORE-006: Lookup by EdgeId
    #[test]
    fn test_get_edge() {
        let (mut ns, mut es) = setup();
        let n1 = ns.create_node(vec![], vec![]);
        let n2 = ns.create_node(vec![], vec![]);
        let e1 = es.create_edge(n1, n2, 5, vec![], &mut ns).expect("e");
        let edge = es.get_edge(e1).expect("found");
        assert_eq!(edge.start_node, n1);
        assert_eq!(edge.end_node, n2);
        assert_eq!(edge.rel_type_id, 5);
    }

    #[test]
    fn test_get_nonexistent_edge() {
        let (_, es) = setup();
        assert!(es.get_edge(EdgeId(999)).is_none());
    }

    // REQ-STORE-007: Get edges for node
    #[test]
    fn test_get_edges_for_node() {
        let (mut ns, mut es) = setup();
        let n1 = ns.create_node(vec![], vec![]);
        let n2 = ns.create_node(vec![], vec![]);
        let n3 = ns.create_node(vec![], vec![]);

        es.create_edge(n1, n2, 1, vec![], &mut ns).expect("e1");
        es.create_edge(n1, n3, 2, vec![], &mut ns).expect("e2");
        es.create_edge(n2, n3, 3, vec![], &mut ns).expect("e3");

        let edges = es.get_edges_for_node(n1, &ns);
        assert_eq!(edges.len(), 2); // n1->n2 and n1->n3
    }

    // REQ-STORE-008: Delete edge
    #[test]
    fn test_delete_edge() {
        let (mut ns, mut es) = setup();
        let n1 = ns.create_node(vec![], vec![]);
        let n2 = ns.create_node(vec![], vec![]);

        let e1 = es.create_edge(n1, n2, 1, vec![], &mut ns).expect("e");
        let deleted = es.delete_edge(e1, &mut ns).expect("delete");
        assert_eq!(deleted.edge_id, e1);
        assert!(es.get_edge(e1).is_none());
    }

    #[test]
    fn test_delete_nonexistent_edge() {
        let (mut ns, mut es) = setup();
        let result = es.delete_edge(EdgeId(999), &mut ns);
        assert!(matches!(result, Err(CypherLiteError::EdgeNotFound(999))));
    }

    // REQ-STORE-004: Delete all edges for a node
    #[test]
    fn test_delete_edges_for_node() {
        let (mut ns, mut es) = setup();
        let n1 = ns.create_node(vec![], vec![]);
        let n2 = ns.create_node(vec![], vec![]);
        let n3 = ns.create_node(vec![], vec![]);

        es.create_edge(n1, n2, 1, vec![], &mut ns).expect("e1");
        es.create_edge(n1, n3, 2, vec![], &mut ns).expect("e2");
        es.create_edge(n2, n3, 3, vec![], &mut ns).expect("e3");
        assert_eq!(es.len(), 3);

        let deleted = es.delete_edges_for_node(n1, &mut ns).expect("del");
        assert_eq!(deleted.len(), 2);
        assert_eq!(es.len(), 1); // only n2->n3 remains
    }

    #[test]
    fn test_edge_with_properties() {
        let (mut ns, mut es) = setup();
        let n1 = ns.create_node(vec![], vec![]);
        let n2 = ns.create_node(vec![], vec![]);

        let props = vec![(1, PropertyValue::String("KNOWS".into()))];
        let e1 = es.create_edge(n1, n2, 1, props, &mut ns).expect("e");
        let edge = es.get_edge(e1).expect("found");
        assert_eq!(edge.properties[0].1, PropertyValue::String("KNOWS".into()));
    }

    #[test]
    fn test_create_edge_with_missing_node() {
        let (mut ns, mut es) = setup();
        let n1 = ns.create_node(vec![], vec![]);
        let result = es.create_edge(n1, NodeId(999), 1, vec![], &mut ns);
        assert!(matches!(result, Err(CypherLiteError::NodeNotFound(999))));
    }

    #[test]
    fn test_self_loop_edge() {
        let (mut ns, mut es) = setup();
        let n1 = ns.create_node(vec![], vec![]);
        let e1 = es.create_edge(n1, n1, 1, vec![], &mut ns).expect("e");
        let edges = es.get_edges_for_node(n1, &ns);
        assert_eq!(edges.len(), 1);
        assert_eq!(edges[0].edge_id, e1);
    }

    // TASK-008: scan_by_type filters edges by relationship type
    #[test]
    fn test_scan_by_type_returns_matching() {
        let (mut ns, mut es) = setup();
        let n1 = ns.create_node(vec![], vec![]);
        let n2 = ns.create_node(vec![], vec![]);
        let n3 = ns.create_node(vec![], vec![]);

        es.create_edge(n1, n2, 1, vec![], &mut ns).expect("e1");
        es.create_edge(n1, n3, 2, vec![], &mut ns).expect("e2");
        es.create_edge(n2, n3, 1, vec![], &mut ns).expect("e3");

        let edges = es.scan_by_type(1);
        assert_eq!(edges.len(), 2);
        for edge in &edges {
            assert_eq!(edge.rel_type_id, 1);
        }
    }

    #[test]
    fn test_scan_by_type_empty_store() {
        let (_, es) = setup();
        let edges = es.scan_by_type(1);
        assert!(edges.is_empty());
    }

    #[test]
    fn test_scan_by_type_nonexistent_returns_empty() {
        let (mut ns, mut es) = setup();
        let n1 = ns.create_node(vec![], vec![]);
        let n2 = ns.create_node(vec![], vec![]);
        es.create_edge(n1, n2, 1, vec![], &mut ns).expect("e1");
        let edges = es.scan_by_type(999);
        assert!(edges.is_empty());
    }

    // BB-T5: update_edge replaces properties
    #[test]
    fn test_update_edge_properties() {
        let (mut ns, mut es) = setup();
        let n1 = ns.create_node(vec![], vec![]);
        let n2 = ns.create_node(vec![], vec![]);

        let props = vec![(1, PropertyValue::String("old".into()))];
        let e1 = es.create_edge(n1, n2, 1, props, &mut ns).expect("e");

        let new_props = vec![(1, PropertyValue::String("new".into()))];
        es.update_edge(e1, new_props).expect("update");

        let edge = es.get_edge(e1).expect("found");
        assert_eq!(edge.properties[0].1, PropertyValue::String("new".into()));
    }

    #[test]
    fn test_update_nonexistent_edge() {
        let (_, mut es) = setup();
        let result = es.update_edge(EdgeId(999), vec![]);
        assert!(matches!(result, Err(CypherLiteError::EdgeNotFound(999))));
    }

    #[test]
    fn test_multiple_edges_between_same_nodes() {
        let (mut ns, mut es) = setup();
        let n1 = ns.create_node(vec![], vec![]);
        let n2 = ns.create_node(vec![], vec![]);

        es.create_edge(n1, n2, 1, vec![], &mut ns).expect("e1");
        es.create_edge(n1, n2, 2, vec![], &mut ns).expect("e2");
        assert_eq!(es.len(), 2);

        let edges = es.get_edges_for_node(n1, &ns);
        assert_eq!(edges.len(), 2);
    }
}
