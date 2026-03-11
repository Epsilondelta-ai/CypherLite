// VersionStore: pre-update snapshot storage for node/edge version history
//
// W-001: VersionStore module
// W-002: Pre-update snapshot
// W-003: Version chain structure

use cypherlite_core::{NodeRecord, RelationshipRecord};
use std::collections::BTreeMap;

/// A version record capturing the state of a node or relationship at a point in time.
#[derive(Debug, Clone, PartialEq)]
pub enum VersionRecord {
    /// Snapshot of a node record.
    Node(NodeRecord),
    /// Snapshot of a relationship record.
    Relationship(RelationshipRecord),
}

/// In-memory version store backed by a BTreeMap.
///
/// Keys are (entity_id, version_seq) where entity_id is the node/edge ID
/// and version_seq is a monotonically increasing sequence number per entity.
pub struct VersionStore {
    /// Storage: (entity_id, version_seq) -> VersionRecord
    versions: BTreeMap<(u64, u64), VersionRecord>,
    /// Next version sequence number per entity.
    next_seq: BTreeMap<u64, u64>,
}

impl VersionStore {
    /// Create a new empty version store.
    pub fn new() -> Self {
        Self {
            versions: BTreeMap::new(),
            next_seq: BTreeMap::new(),
        }
    }

    /// Snapshot a node record before an update.
    /// Returns the version sequence number assigned.
    pub fn snapshot_node(&mut self, entity_id: u64, record: NodeRecord) -> u64 {
        let seq = self.next_seq.entry(entity_id).or_insert(1);
        let current_seq = *seq;
        self.versions
            .insert((entity_id, current_seq), VersionRecord::Node(record));
        *seq += 1;
        current_seq
    }

    /// Snapshot a relationship record before an update.
    /// Returns the version sequence number assigned.
    pub fn snapshot_relationship(&mut self, entity_id: u64, record: RelationshipRecord) -> u64 {
        let seq = self.next_seq.entry(entity_id).or_insert(1);
        let current_seq = *seq;
        self.versions
            .insert((entity_id, current_seq), VersionRecord::Relationship(record));
        *seq += 1;
        current_seq
    }

    /// Get a specific version of an entity.
    pub fn get_version(&self, entity_id: u64, version_seq: u64) -> Option<&VersionRecord> {
        self.versions.get(&(entity_id, version_seq))
    }

    /// Get the latest version of an entity (the most recent snapshot).
    pub fn get_latest_version(&self, entity_id: u64) -> Option<&VersionRecord> {
        let current_seq = self.next_seq.get(&entity_id)?;
        if *current_seq <= 1 {
            return None;
        }
        self.versions.get(&(entity_id, *current_seq - 1))
    }

    /// Get the full version chain for an entity (oldest to newest).
    pub fn get_version_chain(&self, entity_id: u64) -> Vec<(u64, &VersionRecord)> {
        self.versions
            .range((entity_id, 0)..=(entity_id, u64::MAX))
            .map(|((_, seq), record)| (*seq, record))
            .collect()
    }

    /// Get the number of versions stored for an entity.
    pub fn version_count(&self, entity_id: u64) -> u64 {
        self.next_seq.get(&entity_id).copied().unwrap_or(1) - 1
    }

    /// Get the total number of version records stored.
    pub fn total_versions(&self) -> usize {
        self.versions.len()
    }
}

impl Default for VersionStore {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cypherlite_core::{Direction, EdgeId, NodeId, PageId, PropertyValue};

    fn sample_node(id: u64, name: &str) -> NodeRecord {
        NodeRecord {
            node_id: NodeId(id),
            labels: vec![1],
            properties: vec![(1, PropertyValue::String(name.to_string()))],
            next_edge_id: None,
            overflow_page: None,
        }
    }

    fn sample_edge(id: u64) -> RelationshipRecord {
        RelationshipRecord {
            edge_id: EdgeId(id),
            start_node: NodeId(1),
            end_node: NodeId(2),
            rel_type_id: 1,
            direction: Direction::Outgoing,
            next_out_edge: None,
            next_in_edge: None,
            properties: vec![(1, PropertyValue::String("v1".to_string()))],
        }
    }

    // W-001: VersionStore creation
    #[test]
    fn test_version_store_new_is_empty() {
        let store = VersionStore::new();
        assert_eq!(store.total_versions(), 0);
    }

    // W-002: Pre-update snapshot for node
    #[test]
    fn test_snapshot_node() {
        let mut store = VersionStore::new();
        let node = sample_node(1, "Alice");
        let seq = store.snapshot_node(1, node.clone());
        assert_eq!(seq, 1);
        assert_eq!(store.total_versions(), 1);

        let version = store.get_version(1, 1).expect("version exists");
        assert_eq!(*version, VersionRecord::Node(node));
    }

    // W-002: Pre-update snapshot for relationship
    #[test]
    fn test_snapshot_relationship() {
        let mut store = VersionStore::new();
        let edge = sample_edge(1);
        let seq = store.snapshot_relationship(1, edge.clone());
        assert_eq!(seq, 1);

        let version = store.get_version(1, 1).expect("version exists");
        assert_eq!(*version, VersionRecord::Relationship(edge));
    }

    // W-002: Multiple snapshots create incrementing sequence numbers
    #[test]
    fn test_multiple_snapshots_incrementing_seq() {
        let mut store = VersionStore::new();

        let seq1 = store.snapshot_node(1, sample_node(1, "v1"));
        let seq2 = store.snapshot_node(1, sample_node(1, "v2"));
        let seq3 = store.snapshot_node(1, sample_node(1, "v3"));

        assert_eq!(seq1, 1);
        assert_eq!(seq2, 2);
        assert_eq!(seq3, 3);
        assert_eq!(store.version_count(1), 3);
    }

    // W-003: Version chain retrieval
    #[test]
    fn test_version_chain() {
        let mut store = VersionStore::new();

        store.snapshot_node(1, sample_node(1, "v1"));
        store.snapshot_node(1, sample_node(1, "v2"));
        store.snapshot_node(1, sample_node(1, "v3"));

        let chain = store.get_version_chain(1);
        assert_eq!(chain.len(), 3);
        assert_eq!(chain[0].0, 1); // oldest first
        assert_eq!(chain[2].0, 3); // newest last
    }

    // W-003: Get latest version
    #[test]
    fn test_get_latest_version() {
        let mut store = VersionStore::new();

        store.snapshot_node(1, sample_node(1, "v1"));
        store.snapshot_node(1, sample_node(1, "v2"));

        let latest = store.get_latest_version(1).expect("latest exists");
        match latest {
            VersionRecord::Node(n) => {
                assert_eq!(
                    n.properties[0].1,
                    PropertyValue::String("v2".to_string())
                );
            }
            _ => panic!("expected node version"),
        }
    }

    // W-003: Get latest version for nonexistent entity
    #[test]
    fn test_get_latest_version_nonexistent() {
        let store = VersionStore::new();
        assert!(store.get_latest_version(999).is_none());
    }

    // W-003: Version chain for nonexistent entity is empty
    #[test]
    fn test_version_chain_empty() {
        let store = VersionStore::new();
        let chain = store.get_version_chain(999);
        assert!(chain.is_empty());
    }

    // W-002: Independent version sequences per entity
    #[test]
    fn test_independent_sequences_per_entity() {
        let mut store = VersionStore::new();

        let s1 = store.snapshot_node(1, sample_node(1, "A"));
        let s2 = store.snapshot_node(2, sample_node(2, "B"));
        let s3 = store.snapshot_node(1, sample_node(1, "A2"));

        assert_eq!(s1, 1);
        assert_eq!(s2, 1); // different entity, starts at 1
        assert_eq!(s3, 2); // same entity as s1, increments

        assert_eq!(store.version_count(1), 2);
        assert_eq!(store.version_count(2), 1);
    }

    // W-001: Default trait
    #[test]
    fn test_version_store_default() {
        let store = VersionStore::default();
        assert_eq!(store.total_versions(), 0);
    }

    // W-002: Version count for unseen entity
    #[test]
    fn test_version_count_unseen_entity() {
        let store = VersionStore::new();
        assert_eq!(store.version_count(999), 0);
    }
}
