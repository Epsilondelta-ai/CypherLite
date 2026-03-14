// HyperEdgeReverseIndex: tracks which entities participate in which hyperedges
//
// HH-004: Forward index (hyperedge -> participant raw IDs)
// HH-004: Reverse index (participant raw ID -> hyperedges)

use std::collections::BTreeMap;

/// In-memory reverse index tracking entity-hyperedge participation.
///
/// Maintains both forward (hyperedge -> participants) and reverse
/// (participant -> hyperedges) indexes for bidirectional lookups.
/// Participant IDs are stored as raw u64 values.
pub struct HyperEdgeReverseIndex {
    /// Forward index: hyperedge_id -> list of participant raw IDs.
    forward: BTreeMap<u64, Vec<u64>>,
    /// Reverse index: participant_raw_id -> list of hyperedge IDs.
    reverse: BTreeMap<u64, Vec<u64>>,
}

impl HyperEdgeReverseIndex {
    /// Create a new empty reverse index.
    pub fn new() -> Self {
        Self {
            forward: BTreeMap::new(),
            reverse: BTreeMap::new(),
        }
    }

    /// Add a participant to a hyperedge.
    /// Idempotent: adding a duplicate has no effect.
    pub fn add(&mut self, hyperedge_id: u64, participant_id: u64) {
        let fwd = self.forward.entry(hyperedge_id).or_default();
        if !fwd.contains(&participant_id) {
            fwd.push(participant_id);
        }
        let rev = self.reverse.entry(participant_id).or_default();
        if !rev.contains(&hyperedge_id) {
            rev.push(hyperedge_id);
        }
    }

    /// Remove a participant from a hyperedge.
    /// Returns true if the participation existed and was removed.
    pub fn remove(&mut self, hyperedge_id: u64, participant_id: u64) -> bool {
        let removed = if let Some(fwd) = self.forward.get_mut(&hyperedge_id) {
            let before = fwd.len();
            fwd.retain(|&id| id != participant_id);
            fwd.len() < before
        } else {
            false
        };

        if removed {
            if let Some(rev) = self.reverse.get_mut(&participant_id) {
                rev.retain(|&id| id != hyperedge_id);
                if rev.is_empty() {
                    self.reverse.remove(&participant_id);
                }
            }
            // Clean up empty forward entries
            if let Some(fwd) = self.forward.get(&hyperedge_id) {
                if fwd.is_empty() {
                    self.forward.remove(&hyperedge_id);
                }
            }
        }
        removed
    }

    /// Remove all participants from a hyperedge.
    /// Returns the list of removed participant raw IDs.
    pub fn remove_all(&mut self, hyperedge_id: u64) -> Vec<u64> {
        let participant_ids = self.forward.remove(&hyperedge_id).unwrap_or_default();

        // Clean reverse indexes
        for &pid in &participant_ids {
            if let Some(rev) = self.reverse.get_mut(&pid) {
                rev.retain(|&hid| hid != hyperedge_id);
                if rev.is_empty() {
                    self.reverse.remove(&pid);
                }
            }
        }

        participant_ids
    }

    /// List all hyperedge IDs that an entity participates in.
    pub fn hyperedges_for(&self, participant_id: u64) -> Vec<u64> {
        self.reverse
            .get(&participant_id)
            .cloned()
            .unwrap_or_default()
    }

    /// List all participant raw IDs for a hyperedge.
    pub fn participants_for(&self, hyperedge_id: u64) -> Vec<u64> {
        self.forward.get(&hyperedge_id).cloned().unwrap_or_default()
    }
}

impl Default for HyperEdgeReverseIndex {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // HH-004: ReverseIndex creation
    #[test]
    fn test_reverse_index_new_is_empty() {
        let idx = HyperEdgeReverseIndex::new();
        assert!(idx.hyperedges_for(1).is_empty());
        assert!(idx.participants_for(1).is_empty());
    }

    // HH-004: Add participant
    #[test]
    fn test_add_participant() {
        let mut idx = HyperEdgeReverseIndex::new();
        idx.add(1, 10);
        let participants = idx.participants_for(1);
        assert_eq!(participants, vec![10]);
    }

    // HH-004: Add multiple participants
    #[test]
    fn test_add_multiple_participants() {
        let mut idx = HyperEdgeReverseIndex::new();
        idx.add(1, 10);
        idx.add(1, 20);
        idx.add(1, 30);
        let participants = idx.participants_for(1);
        assert_eq!(participants.len(), 3);
        assert!(participants.contains(&10));
        assert!(participants.contains(&20));
        assert!(participants.contains(&30));
    }

    // HH-004: Duplicate add is idempotent
    #[test]
    fn test_add_duplicate_is_idempotent() {
        let mut idx = HyperEdgeReverseIndex::new();
        idx.add(1, 10);
        idx.add(1, 10); // duplicate
        let participants = idx.participants_for(1);
        assert_eq!(participants.len(), 1);
    }

    // HH-004: Reverse lookup (entity -> hyperedges)
    #[test]
    fn test_hyperedges_for_entity() {
        let mut idx = HyperEdgeReverseIndex::new();
        idx.add(1, 10);
        idx.add(2, 10);
        idx.add(3, 10);
        let hyperedges = idx.hyperedges_for(10);
        assert_eq!(hyperedges.len(), 3);
        assert!(hyperedges.contains(&1));
        assert!(hyperedges.contains(&2));
        assert!(hyperedges.contains(&3));
    }

    // HH-004: Forward lookup (hyperedge -> participants)
    #[test]
    fn test_participants_for_hyperedge() {
        let mut idx = HyperEdgeReverseIndex::new();
        idx.add(1, 10);
        idx.add(1, 20);
        let participants = idx.participants_for(1);
        assert_eq!(participants.len(), 2);
    }

    // HH-004: Remove participant
    #[test]
    fn test_remove_participant() {
        let mut idx = HyperEdgeReverseIndex::new();
        idx.add(1, 10);
        idx.add(1, 20);
        let removed = idx.remove(1, 10);
        assert!(removed);
        let participants = idx.participants_for(1);
        assert_eq!(participants, vec![20]);
        // Reverse index also updated
        assert!(idx.hyperedges_for(10).is_empty());
    }

    // HH-004: Remove nonexistent participant returns false
    #[test]
    fn test_remove_nonexistent_participant() {
        let mut idx = HyperEdgeReverseIndex::new();
        let removed = idx.remove(1, 10);
        assert!(!removed);
    }

    // HH-004: Remove all participants of a hyperedge
    #[test]
    fn test_remove_all_participants() {
        let mut idx = HyperEdgeReverseIndex::new();
        idx.add(1, 10);
        idx.add(1, 20);
        let removed = idx.remove_all(1);
        assert_eq!(removed.len(), 2);
        assert!(idx.participants_for(1).is_empty());
        // Reverse indexes also cleared
        assert!(idx.hyperedges_for(10).is_empty());
        assert!(idx.hyperedges_for(20).is_empty());
    }

    // HH-004: Empty list cleanup after last remove
    #[test]
    fn test_empty_list_cleanup() {
        let mut idx = HyperEdgeReverseIndex::new();
        idx.add(1, 10);
        idx.remove(1, 10);
        // Both forward and reverse maps should have empty entries cleaned up
        assert!(idx.participants_for(1).is_empty());
        assert!(idx.hyperedges_for(10).is_empty());
    }

    // HH-004: Remove all from empty hyperedge returns empty vec
    #[test]
    fn test_remove_all_from_empty() {
        let mut idx = HyperEdgeReverseIndex::new();
        let removed = idx.remove_all(999);
        assert!(removed.is_empty());
    }

    // HH-004: Default trait
    #[test]
    fn test_reverse_index_default() {
        let idx = HyperEdgeReverseIndex::default();
        assert!(idx.hyperedges_for(1).is_empty());
    }
}
