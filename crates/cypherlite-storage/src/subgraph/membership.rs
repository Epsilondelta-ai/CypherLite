// MembershipIndex: tracks which nodes belong to which subgraphs
//
// GG-004: Forward index (subgraph -> nodes)
// GG-004: Reverse index (node -> subgraphs)

use cypherlite_core::{NodeId, SubgraphId};
use std::collections::BTreeMap;

/// In-memory membership index tracking node-subgraph relationships.
///
/// Maintains both forward (subgraph -> nodes) and reverse (node -> subgraphs)
/// indexes for bidirectional lookups.
pub struct MembershipIndex {
    /// Forward index: subgraph_id -> list of node IDs.
    forward: BTreeMap<u64, Vec<u64>>,
    /// Reverse index: node_id -> list of subgraph IDs.
    reverse: BTreeMap<u64, Vec<u64>>,
}

impl MembershipIndex {
    /// Create a new empty membership index.
    pub fn new() -> Self {
        Self {
            forward: BTreeMap::new(),
            reverse: BTreeMap::new(),
        }
    }

    /// Add a node as a member of a subgraph.
    /// Idempotent: adding a duplicate has no effect.
    pub fn add(&mut self, subgraph_id: SubgraphId, node_id: NodeId) {
        let fwd = self.forward.entry(subgraph_id.0).or_default();
        if !fwd.contains(&node_id.0) {
            fwd.push(node_id.0);
        }
        let rev = self.reverse.entry(node_id.0).or_default();
        if !rev.contains(&subgraph_id.0) {
            rev.push(subgraph_id.0);
        }
    }

    /// Remove a node from a subgraph.
    /// Returns true if the membership existed and was removed.
    pub fn remove(&mut self, subgraph_id: SubgraphId, node_id: NodeId) -> bool {
        let removed = if let Some(fwd) = self.forward.get_mut(&subgraph_id.0) {
            let before = fwd.len();
            fwd.retain(|&id| id != node_id.0);
            fwd.len() < before
        } else {
            false
        };

        if removed {
            if let Some(rev) = self.reverse.get_mut(&node_id.0) {
                rev.retain(|&id| id != subgraph_id.0);
                if rev.is_empty() {
                    self.reverse.remove(&node_id.0);
                }
            }
            // Clean up empty forward entries
            if let Some(fwd) = self.forward.get(&subgraph_id.0) {
                if fwd.is_empty() {
                    self.forward.remove(&subgraph_id.0);
                }
            }
        }
        removed
    }

    /// Remove all members from a subgraph.
    /// Returns the list of removed node IDs.
    pub fn remove_all(&mut self, subgraph_id: SubgraphId) -> Vec<NodeId> {
        let node_ids = self.forward.remove(&subgraph_id.0).unwrap_or_default();

        // Clean reverse indexes
        for &nid in &node_ids {
            if let Some(rev) = self.reverse.get_mut(&nid) {
                rev.retain(|&sid| sid != subgraph_id.0);
                if rev.is_empty() {
                    self.reverse.remove(&nid);
                }
            }
        }

        node_ids.into_iter().map(NodeId).collect()
    }

    /// List all node members of a subgraph.
    pub fn members(&self, subgraph_id: SubgraphId) -> Vec<NodeId> {
        self.forward
            .get(&subgraph_id.0)
            .map(|ids| ids.iter().map(|&id| NodeId(id)).collect())
            .unwrap_or_default()
    }

    /// List all subgraphs that a node belongs to.
    pub fn memberships(&self, node_id: NodeId) -> Vec<SubgraphId> {
        self.reverse
            .get(&node_id.0)
            .map(|ids| ids.iter().map(|&id| SubgraphId(id)).collect())
            .unwrap_or_default()
    }
}

impl Default for MembershipIndex {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use cypherlite_core::{NodeId, SubgraphId};

    use super::*;

    // GG-004: MembershipIndex creation
    #[test]
    fn test_membership_index_new_is_empty() {
        let idx = MembershipIndex::new();
        assert!(idx.members(SubgraphId(1)).is_empty());
        assert!(idx.memberships(NodeId(1)).is_empty());
    }

    // GG-004: Add member
    #[test]
    fn test_add_member() {
        let mut idx = MembershipIndex::new();
        idx.add(SubgraphId(1), NodeId(10));
        let members = idx.members(SubgraphId(1));
        assert_eq!(members, vec![NodeId(10)]);
    }

    // GG-004: Add multiple members to same subgraph
    #[test]
    fn test_add_multiple_members() {
        let mut idx = MembershipIndex::new();
        idx.add(SubgraphId(1), NodeId(10));
        idx.add(SubgraphId(1), NodeId(20));
        idx.add(SubgraphId(1), NodeId(30));
        let members = idx.members(SubgraphId(1));
        assert_eq!(members.len(), 3);
        assert!(members.contains(&NodeId(10)));
        assert!(members.contains(&NodeId(20)));
        assert!(members.contains(&NodeId(30)));
    }

    // GG-004: Duplicate add is idempotent
    #[test]
    fn test_add_duplicate_member_is_idempotent() {
        let mut idx = MembershipIndex::new();
        idx.add(SubgraphId(1), NodeId(10));
        idx.add(SubgraphId(1), NodeId(10)); // duplicate
        let members = idx.members(SubgraphId(1));
        assert_eq!(members.len(), 1);
    }

    // GG-004: Reverse index (node -> subgraphs)
    #[test]
    fn test_reverse_index() {
        let mut idx = MembershipIndex::new();
        idx.add(SubgraphId(1), NodeId(10));
        idx.add(SubgraphId(2), NodeId(10));
        let memberships = idx.memberships(NodeId(10));
        assert_eq!(memberships.len(), 2);
        assert!(memberships.contains(&SubgraphId(1)));
        assert!(memberships.contains(&SubgraphId(2)));
    }

    // GG-004: Remove member
    #[test]
    fn test_remove_member() {
        let mut idx = MembershipIndex::new();
        idx.add(SubgraphId(1), NodeId(10));
        idx.add(SubgraphId(1), NodeId(20));
        let removed = idx.remove(SubgraphId(1), NodeId(10));
        assert!(removed);
        let members = idx.members(SubgraphId(1));
        assert_eq!(members, vec![NodeId(20)]);
        // Reverse index also updated
        assert!(idx.memberships(NodeId(10)).is_empty());
    }

    // GG-004: Remove nonexistent member returns false
    #[test]
    fn test_remove_nonexistent_member() {
        let mut idx = MembershipIndex::new();
        let removed = idx.remove(SubgraphId(1), NodeId(10));
        assert!(!removed);
    }

    // GG-004: Remove all members of a subgraph
    #[test]
    fn test_remove_all_members() {
        let mut idx = MembershipIndex::new();
        idx.add(SubgraphId(1), NodeId(10));
        idx.add(SubgraphId(1), NodeId(20));
        let removed = idx.remove_all(SubgraphId(1));
        assert_eq!(removed.len(), 2);
        assert!(idx.members(SubgraphId(1)).is_empty());
        // Reverse indexes also cleared
        assert!(idx.memberships(NodeId(10)).is_empty());
        assert!(idx.memberships(NodeId(20)).is_empty());
    }

    // GG-004: Remove all from empty subgraph returns empty vec
    #[test]
    fn test_remove_all_from_empty() {
        let mut idx = MembershipIndex::new();
        let removed = idx.remove_all(SubgraphId(999));
        assert!(removed.is_empty());
    }

    // GG-004: Default trait
    #[test]
    fn test_membership_index_default() {
        let idx = MembershipIndex::default();
        assert!(idx.members(SubgraphId(1)).is_empty());
    }
}
