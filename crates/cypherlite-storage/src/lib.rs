#![warn(missing_docs)]
//! Storage engine for CypherLite: page management, B-tree indexes, WAL, and transactions.

/// B-tree index structures for nodes and edges.
pub mod btree;
/// Catalog for label, property key, and relationship type name resolution.
pub mod catalog;
/// Hyperedge entity storage and reverse index.
#[cfg(feature = "hypergraph")]
pub mod hyperedge;
/// Property index infrastructure for fast node lookups.
pub mod index;
/// Page layout, buffer pool, and page manager.
pub mod page;
/// Subgraph entity storage and membership index.
#[cfg(feature = "subgraph")]
pub mod subgraph;
/// MVCC transaction management.
pub mod transaction;
/// Version storage for pre-update entity snapshots.
pub mod version;
/// Write-ahead log (WAL) for crash recovery.
pub mod wal;

use cypherlite_core::{
    DatabaseConfig, EdgeId, LabelRegistry, NodeId, NodeRecord, PageId, PropertyValue,
    RelationshipRecord, Result,
};
#[cfg(feature = "subgraph")]
use cypherlite_core::{SubgraphId, SubgraphRecord};

use btree::edge_store::EdgeStore;
use btree::node_store::NodeStore;
#[cfg(feature = "hypergraph")]
use cypherlite_core::{HyperEdgeId, HyperEdgeRecord};
#[cfg(feature = "hypergraph")]
use hyperedge::reverse_index::HyperEdgeReverseIndex;
#[cfg(feature = "hypergraph")]
use hyperedge::HyperEdgeStore;
use index::edge_index::EdgeIndexManager;
use index::IndexManager;
use page::buffer_pool::BufferPool;
use page::page_manager::PageManager;
use page::PAGE_SIZE;
#[cfg(feature = "subgraph")]
use subgraph::membership::MembershipIndex;
#[cfg(feature = "subgraph")]
use subgraph::SubgraphStore;
use transaction::mvcc::TransactionManager;
use version::VersionStore;
use wal::checkpoint::Checkpoint;
use wal::reader::WalReader;
use wal::recovery::Recovery;
use wal::writer::WalWriter;

/// The main storage engine for CypherLite.
///
/// Provides high-level access to node/edge CRUD, transactions,
/// WAL, and checkpoint operations.
#[allow(dead_code)]
pub struct StorageEngine {
    page_manager: PageManager,
    buffer_pool: BufferPool,
    wal_writer: WalWriter,
    wal_reader: WalReader,
    tx_manager: TransactionManager,
    node_store: NodeStore,
    edge_store: EdgeStore,
    catalog: catalog::Catalog,
    index_manager: IndexManager,
    edge_index_manager: EdgeIndexManager,
    version_store: VersionStore,
    #[cfg(feature = "subgraph")]
    subgraph_store: SubgraphStore,
    #[cfg(feature = "subgraph")]
    membership_index: MembershipIndex,
    #[cfg(feature = "hypergraph")]
    hyperedge_store: HyperEdgeStore,
    #[cfg(feature = "hypergraph")]
    hyperedge_reverse_index: HyperEdgeReverseIndex,
    config: DatabaseConfig,
}

impl StorageEngine {
    /// Open or create a CypherLite database.
    pub fn open(config: DatabaseConfig) -> Result<Self> {
        let wal_path = config.wal_path();

        // Try to open existing database, or create a new one
        let mut page_manager = if config.path.exists() {
            PageManager::open_database(&config)?
        } else {
            PageManager::create_database(&config)?
        };

        // Run recovery if WAL file exists
        let (_recovered, wal_reader) = if wal_path.exists() {
            Recovery::recover(&mut page_manager, &wal_path)?
        } else {
            (0, WalReader::new())
        };

        // Create or open WAL
        let wal_writer = if wal_path.exists() {
            WalWriter::open(&wal_path, config.wal_sync_mode.clone())?
        } else {
            WalWriter::create(&wal_path, 12345, config.wal_sync_mode.clone())?
        };

        let buffer_pool = BufferPool::new(config.cache_capacity);
        let tx_manager = TransactionManager::new();

        // Initialize ID counters from header
        let next_node_id = page_manager.header().next_node_id;
        let next_edge_id = page_manager.header().next_edge_id;
        let node_store = NodeStore::new(next_node_id);
        let edge_store = EdgeStore::new(next_edge_id);

        // Update tx manager with WAL frame count
        tx_manager.update_current_frame(wal_writer.frame_count());

        // GG-005: Initialize subgraph store from header
        #[cfg(feature = "subgraph")]
        let next_subgraph_id = page_manager.header().next_subgraph_id;
        #[cfg(feature = "subgraph")]
        let subgraph_store = if next_subgraph_id > 0 {
            SubgraphStore::new(next_subgraph_id)
        } else {
            SubgraphStore::new(1)
        };

        // HH-005: Initialize hyperedge store from header
        #[cfg(feature = "hypergraph")]
        let next_hyperedge_id = page_manager.header().next_hyperedge_id;
        #[cfg(feature = "hypergraph")]
        let hyperedge_store = if next_hyperedge_id > 0 {
            HyperEdgeStore::new(next_hyperedge_id)
        } else {
            HyperEdgeStore::new(1)
        };

        Ok(Self {
            page_manager,
            buffer_pool,
            wal_writer,
            wal_reader,
            tx_manager,
            node_store,
            edge_store,
            catalog: catalog::Catalog::default(),
            index_manager: IndexManager::new(),
            edge_index_manager: EdgeIndexManager::new(),
            version_store: VersionStore::new(),
            #[cfg(feature = "subgraph")]
            subgraph_store,
            #[cfg(feature = "subgraph")]
            membership_index: MembershipIndex::new(),
            #[cfg(feature = "hypergraph")]
            hyperedge_store,
            #[cfg(feature = "hypergraph")]
            hyperedge_reverse_index: HyperEdgeReverseIndex::new(),
            config,
        })
    }

    // -- Node CRUD --

    /// Create a new node.
    pub fn create_node(
        &mut self,
        labels: Vec<u32>,
        properties: Vec<(u32, PropertyValue)>,
    ) -> NodeId {
        let id = self
            .node_store
            .create_node(labels.clone(), properties.clone());
        // Update header with new next_node_id
        self.page_manager.header_mut().next_node_id = self.node_store.next_id();
        // Auto-update indexes: for each label and property, check if an index applies
        for &label_id in &labels {
            for (prop_key_id, value) in &properties {
                if let Some(idx) = self.index_manager.find_index_mut(label_id, *prop_key_id) {
                    idx.insert(value, id);
                }
            }
        }
        id
    }

    /// Get a node by ID.
    pub fn get_node(&self, node_id: NodeId) -> Option<&NodeRecord> {
        self.node_store.get_node(node_id)
    }

    /// Update a node's properties.
    pub fn update_node(
        &mut self,
        node_id: NodeId,
        properties: Vec<(u32, PropertyValue)>,
    ) -> Result<()> {
        // Capture old properties for index removal
        let old_node = self.node_store.get_node(node_id).cloned();

        // W-002: Pre-update snapshot into VersionStore
        if self.config.version_storage_enabled {
            if let Some(ref old) = old_node {
                self.version_store.snapshot_node(node_id.0, old.clone());
            }
        }

        self.node_store.update_node(node_id, properties.clone())?;
        // Update indexes: remove old values, insert new values
        if let Some(old) = old_node {
            for &label_id in &old.labels {
                // Remove old property values from indexes
                for (prop_key_id, old_value) in &old.properties {
                    if let Some(idx) = self.index_manager.find_index_mut(label_id, *prop_key_id) {
                        idx.remove(old_value, node_id);
                    }
                }
                // Insert new property values into indexes
                for (prop_key_id, new_value) in &properties {
                    if let Some(idx) = self.index_manager.find_index_mut(label_id, *prop_key_id) {
                        idx.insert(new_value, node_id);
                    }
                }
            }
        }
        Ok(())
    }

    /// Delete a node and all its connected edges.
    /// REQ-STORE-004: Delete all connected edges first.
    pub fn delete_node(&mut self, node_id: NodeId) -> Result<NodeRecord> {
        // Capture node data for index removal before deletion
        let node_data = self.node_store.get_node(node_id).cloned();
        // Delete connected edges first
        self.edge_store
            .delete_edges_for_node(node_id, &mut self.node_store)?;
        let deleted = self.node_store.delete_node(node_id)?;
        // Remove from all applicable indexes
        if let Some(node) = node_data {
            for &label_id in &node.labels {
                for (prop_key_id, value) in &node.properties {
                    if let Some(idx) = self.index_manager.find_index_mut(label_id, *prop_key_id) {
                        idx.remove(value, node_id);
                    }
                }
            }
        }
        Ok(deleted)
    }

    // -- Edge CRUD --

    /// Create a new edge.
    pub fn create_edge(
        &mut self,
        start_node: NodeId,
        end_node: NodeId,
        rel_type_id: u32,
        properties: Vec<(u32, PropertyValue)>,
    ) -> Result<EdgeId> {
        let id = self.edge_store.create_edge(
            start_node,
            end_node,
            rel_type_id,
            properties.clone(),
            &mut self.node_store,
        )?;
        self.page_manager.header_mut().next_edge_id = self.edge_store.next_id();
        // CC-T5: Auto-update edge indexes on CREATE
        for (prop_key_id, value) in &properties {
            if let Some(idx) = self
                .edge_index_manager
                .find_index_mut(rel_type_id, *prop_key_id)
            {
                idx.insert(value, id);
            }
        }
        Ok(id)
    }

    /// Get an edge by ID.
    pub fn get_edge(&self, edge_id: EdgeId) -> Option<&RelationshipRecord> {
        self.edge_store.get_edge(edge_id)
    }

    /// Update an edge's properties.
    pub fn update_edge(
        &mut self,
        edge_id: EdgeId,
        properties: Vec<(u32, PropertyValue)>,
    ) -> Result<()> {
        // CC-T5: Update edge indexes on SET
        let old_edge = self.edge_store.get_edge(edge_id).cloned();
        self.edge_store.update_edge(edge_id, properties.clone())?;
        if let Some(old) = old_edge {
            let rel_type_id = old.rel_type_id;
            // Remove old values from indexes
            for (prop_key_id, old_value) in &old.properties {
                if let Some(idx) = self
                    .edge_index_manager
                    .find_index_mut(rel_type_id, *prop_key_id)
                {
                    idx.remove(old_value, edge_id);
                }
            }
            // Insert new values into indexes
            for (prop_key_id, new_value) in &properties {
                if let Some(idx) = self
                    .edge_index_manager
                    .find_index_mut(rel_type_id, *prop_key_id)
                {
                    idx.insert(new_value, edge_id);
                }
            }
        }
        Ok(())
    }

    /// Get all edges connected to a node.
    pub fn get_edges_for_node(&self, node_id: NodeId) -> Vec<&RelationshipRecord> {
        self.edge_store
            .get_edges_for_node(node_id, &self.node_store)
    }

    /// Delete an edge.
    pub fn delete_edge(&mut self, edge_id: EdgeId) -> Result<RelationshipRecord> {
        // CC-T5: Capture data for index removal
        let edge_data = self.edge_store.get_edge(edge_id).cloned();
        let deleted = self.edge_store.delete_edge(edge_id, &mut self.node_store)?;
        // Remove from edge indexes
        if let Some(edge) = edge_data {
            for (prop_key_id, value) in &edge.properties {
                if let Some(idx) = self
                    .edge_index_manager
                    .find_index_mut(edge.rel_type_id, *prop_key_id)
                {
                    idx.remove(value, edge_id);
                }
            }
        }
        Ok(deleted)
    }

    // -- Scan operations --

    /// Scan all nodes in the database.
    pub fn scan_nodes(&self) -> Vec<&NodeRecord> {
        self.node_store.scan_all()
    }

    /// Scan nodes that have the given label.
    pub fn scan_nodes_by_label(&self, label_id: u32) -> Vec<&NodeRecord> {
        self.node_store.scan_by_label(label_id)
    }

    /// Scan edges of the given relationship type.
    pub fn scan_edges_by_type(&self, type_id: u32) -> Vec<&RelationshipRecord> {
        self.edge_store.scan_by_type(type_id)
    }

    // -- Transaction operations --

    /// Begin a read transaction.
    pub fn begin_read(&self) -> transaction::ReadTransaction {
        self.tx_manager.begin_read()
    }

    /// Begin a write transaction.
    pub fn begin_write(&self) -> Result<transaction::WriteTransaction> {
        self.tx_manager.begin_write()
    }

    // -- WAL operations --

    /// Write a page to the WAL (used internally).
    pub fn wal_write_page(&mut self, page_id: PageId, data: &[u8; PAGE_SIZE]) -> Result<u64> {
        let db_size = self.page_manager.header().page_count;
        self.wal_writer.write_frame(page_id, db_size, data)
    }

    /// Commit the current WAL transaction.
    pub fn wal_commit(&mut self) -> Result<u64> {
        let frame = self.wal_writer.commit()?;
        self.tx_manager.update_current_frame(frame);
        Ok(frame)
    }

    /// Discard uncommitted WAL frames.
    pub fn wal_discard(&mut self) {
        self.wal_writer.discard();
    }

    // -- Checkpoint --

    /// Run a checkpoint: copy WAL frames to main file.
    pub fn checkpoint(&mut self) -> Result<u64> {
        Checkpoint::run(
            &mut self.page_manager,
            &mut self.wal_writer,
            &mut self.wal_reader,
        )
    }

    // -- Misc --

    /// Flush the database header to disk.
    pub fn flush_header(&mut self) -> Result<()> {
        self.page_manager.flush_header()
    }

    /// Returns the number of nodes.
    pub fn node_count(&self) -> usize {
        self.node_store.len()
    }

    /// Returns the number of edges.
    pub fn edge_count(&self) -> usize {
        self.edge_store.len()
    }

    /// Find the first node matching all given labels and properties.
    ///
    /// Scans nodes by the first label (if any) for efficiency, then filters
    /// by remaining labels and all property key-value pairs (exact equality).
    /// Returns `None` if no match is found.
    pub fn find_node(
        &self,
        label_ids: &[u32],
        properties: &[(u32, PropertyValue)],
    ) -> Option<NodeId> {
        let candidates: Vec<&NodeRecord> = if let Some(&first_label) = label_ids.first() {
            self.scan_nodes_by_label(first_label)
        } else {
            self.scan_nodes()
        };

        for node in candidates {
            // Check all required labels
            let has_all_labels = label_ids.iter().all(|lid| node.labels.contains(lid));
            if !has_all_labels {
                continue;
            }
            // Check all required properties (exact equality)
            let has_all_props = properties
                .iter()
                .all(|(key, val)| node.properties.iter().any(|(k, v)| k == key && v == val));
            if has_all_props {
                return Some(node.node_id);
            }
        }
        None
    }

    /// Find the first edge from `start` to `end` with the given relationship type.
    ///
    /// Checks edges connected to the start node and returns the first one
    /// matching both the end node and relationship type.
    pub fn find_edge(&self, start: NodeId, end: NodeId, type_id: u32) -> Option<EdgeId> {
        let edges = self.get_edges_for_node(start);
        for edge in edges {
            if edge.start_node == start && edge.end_node == end && edge.rel_type_id == type_id {
                return Some(edge.edge_id);
            }
        }
        None
    }

    /// Returns a reference to the config.
    pub fn config(&self) -> &DatabaseConfig {
        &self.config
    }

    /// Scan nodes by (label, property_key, value) using index if available.
    ///
    /// If an index exists for (label_id, prop_key_id), uses the fast index lookup.
    /// Otherwise falls back to linear scan.
    pub fn scan_nodes_by_property(
        &self,
        label_id: u32,
        prop_key_id: u32,
        value: &PropertyValue,
    ) -> Vec<NodeId> {
        if let Some(idx) = self.index_manager.find_index(label_id, prop_key_id) {
            // Fast path: index lookup
            idx.lookup(value)
        } else {
            // Slow path: linear scan
            self.node_store
                .scan_by_label(label_id)
                .iter()
                .filter(|n| {
                    n.properties
                        .iter()
                        .any(|(k, v)| *k == prop_key_id && v == value)
                })
                .map(|n| n.node_id)
                .collect()
        }
    }

    /// Range scan by (label, property_key, min, max) using index if available.
    ///
    /// Returns node IDs where the property value is in [min, max] (inclusive).
    /// Uses index range query if available, otherwise falls back to linear scan.
    pub fn scan_nodes_by_range(
        &self,
        label_id: u32,
        prop_key_id: u32,
        min: &PropertyValue,
        max: &PropertyValue,
    ) -> Vec<NodeId> {
        if let Some(idx) = self.index_manager.find_index(label_id, prop_key_id) {
            // Fast path: index range query
            idx.range(min, max)
        } else {
            // Slow path: linear scan with comparison
            let min_key = index::PropertyValueKey(min.clone());
            let max_key = index::PropertyValueKey(max.clone());
            self.node_store
                .scan_by_label(label_id)
                .iter()
                .filter(|n| {
                    n.properties.iter().any(|(k, v)| {
                        if *k != prop_key_id {
                            return false;
                        }
                        let vk = index::PropertyValueKey(v.clone());
                        vk >= min_key && vk <= max_key
                    })
                })
                .map(|n| n.node_id)
                .collect()
        }
    }

    /// Returns a reference to the index manager.
    pub fn index_manager(&self) -> &IndexManager {
        &self.index_manager
    }

    /// Returns a mutable reference to the index manager.
    pub fn index_manager_mut(&mut self) -> &mut IndexManager {
        &mut self.index_manager
    }

    /// Returns a reference to the edge index manager.
    pub fn edge_index_manager(&self) -> &EdgeIndexManager {
        &self.edge_index_manager
    }

    /// Returns a mutable reference to the edge index manager.
    pub fn edge_index_manager_mut(&mut self) -> &mut EdgeIndexManager {
        &mut self.edge_index_manager
    }

    /// Scan edges by (rel_type_id, prop_key_id, value) using index if available.
    ///
    /// If an index exists for (rel_type_id, prop_key_id), uses the fast index lookup.
    /// Otherwise falls back to linear scan.
    pub fn scan_edges_by_property(
        &self,
        rel_type_id: u32,
        prop_key_id: u32,
        value: &PropertyValue,
    ) -> Vec<EdgeId> {
        if let Some(idx) = self.edge_index_manager.find_index(rel_type_id, prop_key_id) {
            idx.lookup(value)
        } else {
            // Slow path: linear scan
            self.edge_store
                .scan_by_type(rel_type_id)
                .iter()
                .filter(|e| {
                    e.properties
                        .iter()
                        .any(|(k, v)| *k == prop_key_id && v == value)
                })
                .map(|e| e.edge_id)
                .collect()
        }
    }

    /// Returns a reference to the catalog.
    pub fn catalog(&self) -> &catalog::Catalog {
        &self.catalog
    }

    /// Returns a mutable reference to the catalog.
    pub fn catalog_mut(&mut self) -> &mut catalog::Catalog {
        &mut self.catalog
    }

    /// Returns a reference to the version store.
    pub fn version_store(&self) -> &VersionStore {
        &self.version_store
    }

    /// Returns a mutable reference to the version store.
    pub fn version_store_mut(&mut self) -> &mut VersionStore {
        &mut self.version_store
    }

    // -- Subgraph operations (cfg-gated) --

    /// Create a new subgraph with the given properties and optional temporal anchor.
    #[cfg(feature = "subgraph")]
    pub fn create_subgraph(
        &mut self,
        properties: Vec<(u32, PropertyValue)>,
        temporal_anchor: Option<i64>,
    ) -> SubgraphId {
        let id = self.subgraph_store.create(properties, temporal_anchor);
        self.page_manager.header_mut().next_subgraph_id = self.subgraph_store.next_id();
        id
    }

    /// Get a subgraph record by ID.
    #[cfg(feature = "subgraph")]
    pub fn get_subgraph(&self, id: SubgraphId) -> Option<&SubgraphRecord> {
        self.subgraph_store.get(id)
    }

    /// Delete a subgraph by ID. Also removes all memberships.
    #[cfg(feature = "subgraph")]
    pub fn delete_subgraph(&mut self, id: SubgraphId) -> cypherlite_core::Result<SubgraphRecord> {
        // Remove all memberships first
        self.membership_index.remove_all(id);
        self.subgraph_store
            .delete(id)
            .ok_or(cypherlite_core::CypherLiteError::SubgraphNotFound(id.0))
    }

    /// Add a node as a member of a subgraph.
    #[cfg(feature = "subgraph")]
    pub fn add_member(
        &mut self,
        subgraph_id: SubgraphId,
        node_id: NodeId,
    ) -> cypherlite_core::Result<()> {
        if self.subgraph_store.get(subgraph_id).is_none() {
            return Err(cypherlite_core::CypherLiteError::SubgraphNotFound(
                subgraph_id.0,
            ));
        }
        if self.node_store.get_node(node_id).is_none() {
            return Err(cypherlite_core::CypherLiteError::NodeNotFound(node_id.0));
        }
        self.membership_index.add(subgraph_id, node_id);
        Ok(())
    }

    /// Remove a node from a subgraph.
    #[cfg(feature = "subgraph")]
    pub fn remove_member(
        &mut self,
        subgraph_id: SubgraphId,
        node_id: NodeId,
    ) -> cypherlite_core::Result<()> {
        if self.subgraph_store.get(subgraph_id).is_none() {
            return Err(cypherlite_core::CypherLiteError::SubgraphNotFound(
                subgraph_id.0,
            ));
        }
        self.membership_index.remove(subgraph_id, node_id);
        Ok(())
    }

    /// List all node members of a subgraph.
    #[cfg(feature = "subgraph")]
    pub fn list_members(&self, subgraph_id: SubgraphId) -> Vec<NodeId> {
        self.membership_index.members(subgraph_id)
    }

    /// Get all subgraphs that a node belongs to.
    #[cfg(feature = "subgraph")]
    pub fn get_subgraph_memberships(&self, node_id: NodeId) -> Vec<SubgraphId> {
        self.membership_index.memberships(node_id)
    }

    /// Scan all subgraph records.
    #[cfg(feature = "subgraph")]
    pub fn scan_subgraphs(&self) -> Vec<&SubgraphRecord> {
        self.subgraph_store.all().collect()
    }

    // -- Hyperedge operations (cfg-gated) --

    /// Create a new hyperedge with the given type, sources, targets, and properties.
    #[cfg(feature = "hypergraph")]
    pub fn create_hyperedge(
        &mut self,
        rel_type_id: u32,
        sources: Vec<cypherlite_core::GraphEntity>,
        targets: Vec<cypherlite_core::GraphEntity>,
        properties: Vec<(u32, PropertyValue)>,
    ) -> HyperEdgeId {
        let id =
            self.hyperedge_store
                .create(rel_type_id, sources.clone(), targets.clone(), properties);
        // Sync next_hyperedge_id with header
        self.page_manager.header_mut().next_hyperedge_id = self.hyperedge_store.next_id();
        // Update reverse index for all source and target participants
        for entity in sources.iter().chain(targets.iter()) {
            let raw_id = match entity {
                cypherlite_core::GraphEntity::Node(nid) => nid.0,
                cypherlite_core::GraphEntity::Subgraph(sid) => sid.0,
                #[cfg(feature = "hypergraph")]
                cypherlite_core::GraphEntity::HyperEdge(hid) => hid.0,
                #[cfg(feature = "hypergraph")]
                cypherlite_core::GraphEntity::TemporalRef(nid, _) => nid.0,
            };
            self.hyperedge_reverse_index.add(id.0, raw_id);
        }
        id
    }

    /// Get a hyperedge record by ID.
    #[cfg(feature = "hypergraph")]
    pub fn get_hyperedge(&self, id: HyperEdgeId) -> Option<&HyperEdgeRecord> {
        self.hyperedge_store.get(id)
    }

    /// Delete a hyperedge by ID. Also removes all reverse index entries.
    #[cfg(feature = "hypergraph")]
    pub fn delete_hyperedge(
        &mut self,
        id: HyperEdgeId,
    ) -> cypherlite_core::Result<HyperEdgeRecord> {
        // Remove all reverse index entries first
        self.hyperedge_reverse_index.remove_all(id.0);
        self.hyperedge_store
            .delete(id)
            .ok_or(cypherlite_core::CypherLiteError::HyperEdgeNotFound(id.0))
    }

    /// Scan all hyperedge records.
    #[cfg(feature = "hypergraph")]
    pub fn scan_hyperedges(&self) -> Vec<&HyperEdgeRecord> {
        self.hyperedge_store.all().collect()
    }

    /// Find all hyperedge IDs that an entity participates in (by raw entity ID).
    #[cfg(feature = "hypergraph")]
    pub fn hyperedges_for_entity(&self, raw_entity_id: u64) -> Vec<u64> {
        self.hyperedge_reverse_index.hyperedges_for(raw_entity_id)
    }
}

impl LabelRegistry for StorageEngine {
    fn get_or_create_label(&mut self, name: &str) -> u32 {
        self.catalog.get_or_create_label(name)
    }

    fn label_id(&self, name: &str) -> Option<u32> {
        self.catalog.label_id(name)
    }

    fn label_name(&self, id: u32) -> Option<&str> {
        self.catalog.label_name(id)
    }

    fn get_or_create_rel_type(&mut self, name: &str) -> u32 {
        self.catalog.get_or_create_rel_type(name)
    }

    fn rel_type_id(&self, name: &str) -> Option<u32> {
        self.catalog.rel_type_id(name)
    }

    fn rel_type_name(&self, id: u32) -> Option<&str> {
        self.catalog.rel_type_name(id)
    }

    fn get_or_create_prop_key(&mut self, name: &str) -> u32 {
        self.catalog.get_or_create_prop_key(name)
    }

    fn prop_key_id(&self, name: &str) -> Option<u32> {
        self.catalog.prop_key_id(name)
    }

    fn prop_key_name(&self, id: u32) -> Option<&str> {
        self.catalog.prop_key_name(id)
    }
}

impl Drop for StorageEngine {
    fn drop(&mut self) {
        // Flush WAL to main database file on close
        let _ = self.checkpoint();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cypherlite_core::{CypherLiteError, SyncMode};
    use tempfile::tempdir;

    fn test_engine(dir: &std::path::Path) -> StorageEngine {
        let config = DatabaseConfig {
            path: dir.join("test.cyl"),
            wal_sync_mode: SyncMode::Normal,
            ..Default::default()
        };
        StorageEngine::open(config).expect("open")
    }

    #[test]
    fn test_open_creates_database() {
        let dir = tempdir().expect("tempdir");
        let engine = test_engine(dir.path());
        assert_eq!(engine.node_count(), 0);
        assert_eq!(engine.edge_count(), 0);
    }

    #[test]
    fn test_create_and_get_node() {
        let dir = tempdir().expect("tempdir");
        let mut engine = test_engine(dir.path());
        let id = engine.create_node(vec![1, 2], vec![(1, PropertyValue::String("Alice".into()))]);
        let node = engine.get_node(id).expect("found");
        assert_eq!(node.node_id, id);
        assert_eq!(node.labels, vec![1, 2]);
        assert_eq!(engine.node_count(), 1);
    }

    #[test]
    fn test_update_node() {
        let dir = tempdir().expect("tempdir");
        let mut engine = test_engine(dir.path());
        let id = engine.create_node(vec![], vec![(1, PropertyValue::Int64(10))]);
        engine
            .update_node(id, vec![(1, PropertyValue::Int64(20))])
            .expect("update");
        let node = engine.get_node(id).expect("found");
        assert_eq!(node.properties[0].1, PropertyValue::Int64(20));
    }

    #[test]
    fn test_delete_node() {
        let dir = tempdir().expect("tempdir");
        let mut engine = test_engine(dir.path());
        let id = engine.create_node(vec![], vec![]);
        engine.delete_node(id).expect("delete");
        assert!(engine.get_node(id).is_none());
        assert_eq!(engine.node_count(), 0);
    }

    #[test]
    fn test_create_and_get_edge() {
        let dir = tempdir().expect("tempdir");
        let mut engine = test_engine(dir.path());
        let n1 = engine.create_node(vec![], vec![]);
        let n2 = engine.create_node(vec![], vec![]);
        let e = engine.create_edge(n1, n2, 1, vec![]).expect("edge");
        let edge = engine.get_edge(e).expect("found");
        assert_eq!(edge.start_node, n1);
        assert_eq!(edge.end_node, n2);
    }

    #[test]
    fn test_get_edges_for_node() {
        let dir = tempdir().expect("tempdir");
        let mut engine = test_engine(dir.path());
        let n1 = engine.create_node(vec![], vec![]);
        let n2 = engine.create_node(vec![], vec![]);
        let n3 = engine.create_node(vec![], vec![]);
        engine.create_edge(n1, n2, 1, vec![]).expect("e1");
        engine.create_edge(n1, n3, 2, vec![]).expect("e2");
        let edges = engine.get_edges_for_node(n1);
        assert_eq!(edges.len(), 2);
    }

    #[test]
    fn test_delete_edge() {
        let dir = tempdir().expect("tempdir");
        let mut engine = test_engine(dir.path());
        let n1 = engine.create_node(vec![], vec![]);
        let n2 = engine.create_node(vec![], vec![]);
        let e = engine.create_edge(n1, n2, 1, vec![]).expect("edge");
        engine.delete_edge(e).expect("delete");
        assert!(engine.get_edge(e).is_none());
    }

    // REQ-STORE-004: Delete node deletes connected edges first
    #[test]
    fn test_delete_node_cascades_edges() {
        let dir = tempdir().expect("tempdir");
        let mut engine = test_engine(dir.path());
        let n1 = engine.create_node(vec![], vec![]);
        let n2 = engine.create_node(vec![], vec![]);
        let e = engine.create_edge(n1, n2, 1, vec![]).expect("edge");
        engine.delete_node(n1).expect("delete");
        assert!(engine.get_edge(e).is_none());
        assert_eq!(engine.edge_count(), 0);
    }

    #[test]
    fn test_begin_read_transaction() {
        let dir = tempdir().expect("tempdir");
        let engine = test_engine(dir.path());
        let tx = engine.begin_read();
        assert_eq!(tx.tx_id(), 1);
    }

    #[test]
    fn test_begin_write_transaction() {
        let dir = tempdir().expect("tempdir");
        let engine = test_engine(dir.path());
        let tx = engine.begin_write().expect("write");
        assert_eq!(tx.tx_id(), 1);
    }

    // REQ-TX-010: Second write returns conflict
    #[test]
    fn test_write_conflict() {
        let dir = tempdir().expect("tempdir");
        let engine = test_engine(dir.path());
        let _w1 = engine.begin_write().expect("w1");
        let result = engine.begin_write();
        assert!(matches!(result, Err(CypherLiteError::TransactionConflict)));
    }

    #[test]
    fn test_wal_write_and_commit() {
        let dir = tempdir().expect("tempdir");
        let mut engine = test_engine(dir.path());
        let data = [0xAB; PAGE_SIZE];
        engine.wal_write_page(PageId(2), &data).expect("write");
        let frame = engine.wal_commit().expect("commit");
        assert!(frame > 0);
    }

    #[test]
    fn test_checkpoint() {
        let dir = tempdir().expect("tempdir");
        let mut engine = test_engine(dir.path());
        let data = [0xAB; PAGE_SIZE];
        engine.wal_write_page(PageId(2), &data).expect("write");
        engine.wal_commit().expect("commit");
        let count = engine.checkpoint().expect("checkpoint");
        assert_eq!(count, 1);
    }

    #[test]
    fn test_reopen_database() {
        let dir = tempdir().expect("tempdir");
        let config = DatabaseConfig {
            path: dir.path().join("test.cyl"),
            wal_sync_mode: SyncMode::Normal,
            ..Default::default()
        };

        // Create and populate
        {
            let mut engine = StorageEngine::open(config.clone()).expect("open");
            engine.create_node(vec![1], vec![(1, PropertyValue::String("Alice".into()))]);
            engine.flush_header().expect("flush");
        }

        // Reopen - header should preserve next_node_id
        {
            let engine = StorageEngine::open(config).expect("reopen");
            // Node data is in-memory B-tree, so it won't persist across restarts
            // without serialization. But header data (next_id) should persist.
            assert_eq!(engine.node_count(), 0); // in-memory only for Phase 1
        }
    }

    // TASK-006: scan_nodes
    #[test]
    fn test_scan_nodes_empty() {
        let dir = tempdir().expect("tempdir");
        let engine = test_engine(dir.path());
        let nodes = engine.scan_nodes();
        assert!(nodes.is_empty());
    }

    #[test]
    fn test_scan_nodes_returns_all() {
        let dir = tempdir().expect("tempdir");
        let mut engine = test_engine(dir.path());
        engine.create_node(vec![1], vec![]);
        engine.create_node(vec![2], vec![]);
        engine.create_node(vec![3], vec![]);
        let nodes = engine.scan_nodes();
        assert_eq!(nodes.len(), 3);
    }

    // TASK-007: scan_nodes_by_label
    #[test]
    fn test_scan_nodes_by_label_returns_matching() {
        let dir = tempdir().expect("tempdir");
        let mut engine = test_engine(dir.path());
        engine.create_node(vec![1, 2], vec![]);
        engine.create_node(vec![2, 3], vec![]);
        engine.create_node(vec![3], vec![]);
        let nodes = engine.scan_nodes_by_label(2);
        assert_eq!(nodes.len(), 2);
    }

    #[test]
    fn test_scan_nodes_by_label_nonexistent() {
        let dir = tempdir().expect("tempdir");
        let mut engine = test_engine(dir.path());
        engine.create_node(vec![1], vec![]);
        let nodes = engine.scan_nodes_by_label(999);
        assert!(nodes.is_empty());
    }

    // TASK-008: scan_edges_by_type
    #[test]
    fn test_scan_edges_by_type_returns_matching() {
        let dir = tempdir().expect("tempdir");
        let mut engine = test_engine(dir.path());
        let n1 = engine.create_node(vec![], vec![]);
        let n2 = engine.create_node(vec![], vec![]);
        let n3 = engine.create_node(vec![], vec![]);
        engine.create_edge(n1, n2, 1, vec![]).expect("e1");
        engine.create_edge(n1, n3, 2, vec![]).expect("e2");
        engine.create_edge(n2, n3, 1, vec![]).expect("e3");
        let edges = engine.scan_edges_by_type(1);
        assert_eq!(edges.len(), 2);
        for edge in &edges {
            assert_eq!(edge.rel_type_id, 1);
        }
    }

    #[test]
    fn test_scan_edges_by_type_empty() {
        let dir = tempdir().expect("tempdir");
        let engine = test_engine(dir.path());
        let edges = engine.scan_edges_by_type(1);
        assert!(edges.is_empty());
    }

    // TASK-080: find_node API
    #[test]
    fn test_find_node_returns_matching_node() {
        let dir = tempdir().expect("tempdir");
        let mut engine = test_engine(dir.path());
        let label_id = engine.get_or_create_label("Person");
        let name_key = engine.get_or_create_prop_key("name");
        let nid = engine.create_node(
            vec![label_id],
            vec![(name_key, PropertyValue::String("Alice".into()))],
        );
        let found = engine.find_node(
            &[label_id],
            &[(name_key, PropertyValue::String("Alice".into()))],
        );
        assert_eq!(found, Some(nid));
    }

    #[test]
    fn test_find_node_returns_none_when_no_match() {
        let dir = tempdir().expect("tempdir");
        let mut engine = test_engine(dir.path());
        let label_id = engine.get_or_create_label("Person");
        let name_key = engine.get_or_create_prop_key("name");
        engine.create_node(
            vec![label_id],
            vec![(name_key, PropertyValue::String("Alice".into()))],
        );
        let found = engine.find_node(
            &[label_id],
            &[(name_key, PropertyValue::String("Bob".into()))],
        );
        assert_eq!(found, None);
    }

    #[test]
    fn test_find_node_empty_db() {
        let dir = tempdir().expect("tempdir");
        let engine = test_engine(dir.path());
        let found = engine.find_node(&[0], &[]);
        assert_eq!(found, None);
    }

    #[test]
    fn test_find_node_multiple_labels() {
        let dir = tempdir().expect("tempdir");
        let mut engine = test_engine(dir.path());
        let person = engine.get_or_create_label("Person");
        let employee = engine.get_or_create_label("Employee");
        let name_key = engine.get_or_create_prop_key("name");
        let nid = engine.create_node(
            vec![person, employee],
            vec![(name_key, PropertyValue::String("Alice".into()))],
        );
        // Both labels required
        let found = engine.find_node(
            &[person, employee],
            &[(name_key, PropertyValue::String("Alice".into()))],
        );
        assert_eq!(found, Some(nid));
        // Only person label - should still match (node has both)
        let found2 = engine.find_node(
            &[person],
            &[(name_key, PropertyValue::String("Alice".into()))],
        );
        assert_eq!(found2, Some(nid));
    }

    #[test]
    fn test_find_node_no_properties() {
        let dir = tempdir().expect("tempdir");
        let mut engine = test_engine(dir.path());
        let label_id = engine.get_or_create_label("Person");
        let nid = engine.create_node(vec![label_id], vec![]);
        let found = engine.find_node(&[label_id], &[]);
        assert_eq!(found, Some(nid));
    }

    // TASK-081: find_edge API
    #[test]
    fn test_find_edge_returns_matching_edge() {
        let dir = tempdir().expect("tempdir");
        let mut engine = test_engine(dir.path());
        let n1 = engine.create_node(vec![], vec![]);
        let n2 = engine.create_node(vec![], vec![]);
        let type_id = engine.get_or_create_rel_type("KNOWS");
        let eid = engine.create_edge(n1, n2, type_id, vec![]).expect("edge");
        let found = engine.find_edge(n1, n2, type_id);
        assert_eq!(found, Some(eid));
    }

    #[test]
    fn test_find_edge_returns_none_wrong_type() {
        let dir = tempdir().expect("tempdir");
        let mut engine = test_engine(dir.path());
        let n1 = engine.create_node(vec![], vec![]);
        let n2 = engine.create_node(vec![], vec![]);
        let knows = engine.get_or_create_rel_type("KNOWS");
        let likes = engine.get_or_create_rel_type("LIKES");
        engine.create_edge(n1, n2, knows, vec![]).expect("edge");
        let found = engine.find_edge(n1, n2, likes);
        assert_eq!(found, None);
    }

    #[test]
    fn test_find_edge_returns_none_wrong_endpoints() {
        let dir = tempdir().expect("tempdir");
        let mut engine = test_engine(dir.path());
        let n1 = engine.create_node(vec![], vec![]);
        let n2 = engine.create_node(vec![], vec![]);
        let n3 = engine.create_node(vec![], vec![]);
        let type_id = engine.get_or_create_rel_type("KNOWS");
        engine.create_edge(n1, n2, type_id, vec![]).expect("edge");
        let found = engine.find_edge(n1, n3, type_id);
        assert_eq!(found, None);
    }

    #[test]
    fn test_find_edge_empty_db() {
        let dir = tempdir().expect("tempdir");
        let mut engine = test_engine(dir.path());
        let n1 = engine.create_node(vec![], vec![]);
        let n2 = engine.create_node(vec![], vec![]);
        let found = engine.find_edge(n1, n2, 0);
        assert_eq!(found, None);
    }

    // ======================================================================
    // TASK-095: Auto-update indexes on mutations
    // ======================================================================

    #[test]
    fn test_create_node_updates_index() {
        let dir = tempdir().expect("tempdir");
        let mut engine = test_engine(dir.path());
        let label_id = engine.get_or_create_label("Person");
        let name_key = engine.get_or_create_prop_key("name");

        // Create index before creating nodes
        engine
            .index_manager_mut()
            .create_index("idx_person_name".to_string(), label_id, name_key)
            .expect("create index");

        let nid = engine.create_node(
            vec![label_id],
            vec![(name_key, PropertyValue::String("Alice".into()))],
        );

        // Index should contain the new node
        let result = engine
            .index_manager()
            .find_index(label_id, name_key)
            .expect("index exists")
            .lookup(&PropertyValue::String("Alice".into()));
        assert_eq!(result, vec![nid]);
    }

    #[test]
    fn test_update_node_updates_index() {
        let dir = tempdir().expect("tempdir");
        let mut engine = test_engine(dir.path());
        let label_id = engine.get_or_create_label("Person");
        let name_key = engine.get_or_create_prop_key("name");

        engine
            .index_manager_mut()
            .create_index("idx_person_name".to_string(), label_id, name_key)
            .expect("create index");

        let nid = engine.create_node(
            vec![label_id],
            vec![(name_key, PropertyValue::String("Alice".into()))],
        );

        // Update the property
        engine
            .update_node(nid, vec![(name_key, PropertyValue::String("Bob".into()))])
            .expect("update");

        let idx = engine
            .index_manager()
            .find_index(label_id, name_key)
            .expect("idx");
        // Old value should not be in index
        assert!(idx
            .lookup(&PropertyValue::String("Alice".into()))
            .is_empty());
        // New value should be in index
        assert_eq!(idx.lookup(&PropertyValue::String("Bob".into())), vec![nid]);
    }

    #[test]
    fn test_delete_node_removes_from_index() {
        let dir = tempdir().expect("tempdir");
        let mut engine = test_engine(dir.path());
        let label_id = engine.get_or_create_label("Person");
        let name_key = engine.get_or_create_prop_key("name");

        engine
            .index_manager_mut()
            .create_index("idx_person_name".to_string(), label_id, name_key)
            .expect("create index");

        let nid = engine.create_node(
            vec![label_id],
            vec![(name_key, PropertyValue::String("Alice".into()))],
        );

        engine.delete_node(nid).expect("delete");

        let idx = engine
            .index_manager()
            .find_index(label_id, name_key)
            .expect("idx");
        assert!(idx
            .lookup(&PropertyValue::String("Alice".into()))
            .is_empty());
    }

    // ======================================================================
    // TASK-096: scan_nodes_by_property
    // ======================================================================

    #[test]
    fn test_scan_nodes_by_property_with_index() {
        let dir = tempdir().expect("tempdir");
        let mut engine = test_engine(dir.path());
        let label_id = engine.get_or_create_label("Person");
        let name_key = engine.get_or_create_prop_key("name");

        engine
            .index_manager_mut()
            .create_index("idx_person_name".to_string(), label_id, name_key)
            .expect("create index");

        let n1 = engine.create_node(
            vec![label_id],
            vec![(name_key, PropertyValue::String("Alice".into()))],
        );
        engine.create_node(
            vec![label_id],
            vec![(name_key, PropertyValue::String("Bob".into()))],
        );

        let result = engine.scan_nodes_by_property(
            label_id,
            name_key,
            &PropertyValue::String("Alice".into()),
        );
        assert_eq!(result, vec![n1]);
    }

    #[test]
    fn test_scan_nodes_by_property_without_index() {
        let dir = tempdir().expect("tempdir");
        let mut engine = test_engine(dir.path());
        let label_id = engine.get_or_create_label("Person");
        let name_key = engine.get_or_create_prop_key("name");

        // No index created -- should use linear scan
        let n1 = engine.create_node(
            vec![label_id],
            vec![(name_key, PropertyValue::String("Alice".into()))],
        );
        engine.create_node(
            vec![label_id],
            vec![(name_key, PropertyValue::String("Bob".into()))],
        );

        let result = engine.scan_nodes_by_property(
            label_id,
            name_key,
            &PropertyValue::String("Alice".into()),
        );
        assert_eq!(result, vec![n1]);
    }

    #[test]
    fn test_scan_nodes_by_property_both_paths_agree() {
        let dir = tempdir().expect("tempdir");
        let mut engine = test_engine(dir.path());
        let label_id = engine.get_or_create_label("Person");
        let name_key = engine.get_or_create_prop_key("name");

        // Create nodes without index
        engine.create_node(
            vec![label_id],
            vec![(name_key, PropertyValue::String("Alice".into()))],
        );
        engine.create_node(
            vec![label_id],
            vec![(name_key, PropertyValue::String("Bob".into()))],
        );
        engine.create_node(
            vec![label_id],
            vec![(name_key, PropertyValue::String("Alice".into()))],
        );

        // Linear scan result
        let without_idx = engine.scan_nodes_by_property(
            label_id,
            name_key,
            &PropertyValue::String("Alice".into()),
        );

        // Now create index and backfill
        engine
            .index_manager_mut()
            .create_index("idx".to_string(), label_id, name_key)
            .expect("create");
        // Backfill: manually insert existing nodes into the index
        let nodes: Vec<_> = engine
            .scan_nodes_by_label(label_id)
            .iter()
            .map(|n| (n.node_id, n.properties.clone()))
            .collect();
        for (nid, props) in &nodes {
            for (pk, v) in props {
                if *pk == name_key {
                    engine
                        .index_manager_mut()
                        .find_index_mut(label_id, name_key)
                        .expect("idx")
                        .insert(v, *nid);
                }
            }
        }

        let with_idx = engine.scan_nodes_by_property(
            label_id,
            name_key,
            &PropertyValue::String("Alice".into()),
        );

        // Both paths should return same IDs (order may differ)
        let mut a: Vec<u64> = without_idx.iter().map(|n| n.0).collect();
        let mut b: Vec<u64> = with_idx.iter().map(|n| n.0).collect();
        a.sort();
        b.sort();
        assert_eq!(a, b);
    }

    // ======================================================================
    // TASK-097: scan_nodes_by_range
    // ======================================================================

    #[test]
    fn test_scan_nodes_by_range_with_index() {
        let dir = tempdir().expect("tempdir");
        let mut engine = test_engine(dir.path());
        let label_id = engine.get_or_create_label("Person");
        let age_key = engine.get_or_create_prop_key("age");

        engine
            .index_manager_mut()
            .create_index("idx_person_age".to_string(), label_id, age_key)
            .expect("create index");

        for age in [20, 25, 30, 35, 40] {
            engine.create_node(vec![label_id], vec![(age_key, PropertyValue::Int64(age))]);
        }

        let result = engine.scan_nodes_by_range(
            label_id,
            age_key,
            &PropertyValue::Int64(25),
            &PropertyValue::Int64(35),
        );
        assert_eq!(result.len(), 3); // 25, 30, 35
    }

    #[test]
    fn test_scan_nodes_by_range_without_index() {
        let dir = tempdir().expect("tempdir");
        let mut engine = test_engine(dir.path());
        let label_id = engine.get_or_create_label("Person");
        let age_key = engine.get_or_create_prop_key("age");

        for age in [20, 25, 30, 35, 40] {
            engine.create_node(vec![label_id], vec![(age_key, PropertyValue::Int64(age))]);
        }

        let result = engine.scan_nodes_by_range(
            label_id,
            age_key,
            &PropertyValue::Int64(25),
            &PropertyValue::Int64(35),
        );
        assert_eq!(result.len(), 3);
    }

    // REQ-CATALOG-030: StorageEngine exposes catalog accessor
    #[test]
    fn test_storage_engine_has_catalog() {
        let dir = tempdir().expect("tempdir");
        let engine = test_engine(dir.path());
        let cat = engine.catalog();
        // Empty catalog on fresh database
        assert_eq!(cat.label_id("Person"), None);
    }

    // REQ-CATALOG-031: StorageEngine exposes mutable catalog accessor
    #[test]
    fn test_storage_engine_catalog_mut() {
        let dir = tempdir().expect("tempdir");
        let mut engine = test_engine(dir.path());
        let cat = engine.catalog_mut();
        let id = cat.get_or_create_label("Person");
        assert_eq!(engine.catalog().label_id("Person"), Some(id));
    }

    // REQ-CATALOG-032: StorageEngine implements LabelRegistry by delegation
    #[test]
    fn test_storage_engine_label_registry() {
        use cypherlite_core::LabelRegistry;

        let dir = tempdir().expect("tempdir");
        let mut engine = test_engine(dir.path());

        let label_id = engine.get_or_create_label("Person");
        assert_eq!(engine.label_id("Person"), Some(label_id));
        assert_eq!(engine.label_name(label_id), Some("Person"));

        let rel_id = engine.get_or_create_rel_type("KNOWS");
        assert_eq!(engine.rel_type_id("KNOWS"), Some(rel_id));
        assert_eq!(engine.rel_type_name(rel_id), Some("KNOWS"));

        let prop_id = engine.get_or_create_prop_key("name");
        assert_eq!(engine.prop_key_id("name"), Some(prop_id));
        assert_eq!(engine.prop_key_name(prop_id), Some("name"));
    }

    // ======================================================================
    // GG-005: StorageEngine subgraph integration tests
    // ======================================================================

    #[cfg(feature = "subgraph")]
    mod subgraph_engine_tests {
        use super::*;
        use cypherlite_core::SubgraphId;

        fn test_engine_sg(dir: &std::path::Path) -> StorageEngine {
            let config = DatabaseConfig {
                path: dir.join("test.cyl"),
                wal_sync_mode: SyncMode::Normal,
                ..Default::default()
            };
            StorageEngine::open(config).expect("open")
        }

        // GG-005: Create subgraph via StorageEngine
        #[test]
        fn test_engine_create_subgraph() {
            let dir = tempdir().expect("tempdir");
            let mut engine = test_engine_sg(dir.path());
            let id = engine.create_subgraph(vec![], None);
            assert_eq!(id, SubgraphId(1));
        }

        // GG-005: Get subgraph via StorageEngine
        #[test]
        fn test_engine_get_subgraph() {
            let dir = tempdir().expect("tempdir");
            let mut engine = test_engine_sg(dir.path());
            let id = engine
                .create_subgraph(vec![(1, PropertyValue::String("test".into()))], Some(1_000));
            let record = engine.get_subgraph(id).expect("found");
            assert_eq!(record.subgraph_id, id);
            assert_eq!(record.temporal_anchor, Some(1_000));
        }

        // GG-005: Get nonexistent subgraph returns None
        #[test]
        fn test_engine_get_nonexistent_subgraph() {
            let dir = tempdir().expect("tempdir");
            let engine = test_engine_sg(dir.path());
            assert!(engine.get_subgraph(SubgraphId(999)).is_none());
        }

        // GG-005: Delete subgraph via StorageEngine
        #[test]
        fn test_engine_delete_subgraph() {
            let dir = tempdir().expect("tempdir");
            let mut engine = test_engine_sg(dir.path());
            let id = engine.create_subgraph(vec![], None);
            engine.delete_subgraph(id).expect("delete");
            assert!(engine.get_subgraph(id).is_none());
        }

        // GG-005: Delete nonexistent subgraph returns error
        #[test]
        fn test_engine_delete_nonexistent_subgraph() {
            let dir = tempdir().expect("tempdir");
            let mut engine = test_engine_sg(dir.path());
            let result = engine.delete_subgraph(SubgraphId(999));
            assert!(result.is_err());
        }

        // GG-005: Add member to subgraph
        #[test]
        fn test_engine_add_member() {
            let dir = tempdir().expect("tempdir");
            let mut engine = test_engine_sg(dir.path());
            let sg = engine.create_subgraph(vec![], None);
            let n1 = engine.create_node(vec![], vec![]);
            engine.add_member(sg, n1).expect("add member");
            let members = engine.list_members(sg);
            assert_eq!(members.len(), 1);
            assert_eq!(members[0], n1);
        }

        // GG-005: Add member - subgraph not found
        #[test]
        fn test_engine_add_member_subgraph_not_found() {
            let dir = tempdir().expect("tempdir");
            let mut engine = test_engine_sg(dir.path());
            let n1 = engine.create_node(vec![], vec![]);
            let result = engine.add_member(SubgraphId(999), n1);
            assert!(result.is_err());
        }

        // GG-005: Add member - node not found
        #[test]
        fn test_engine_add_member_node_not_found() {
            let dir = tempdir().expect("tempdir");
            let mut engine = test_engine_sg(dir.path());
            let sg = engine.create_subgraph(vec![], None);
            let result = engine.add_member(sg, NodeId(999));
            assert!(result.is_err());
        }

        // GG-005: Remove member from subgraph
        #[test]
        fn test_engine_remove_member() {
            let dir = tempdir().expect("tempdir");
            let mut engine = test_engine_sg(dir.path());
            let sg = engine.create_subgraph(vec![], None);
            let n1 = engine.create_node(vec![], vec![]);
            engine.add_member(sg, n1).expect("add");
            engine.remove_member(sg, n1).expect("remove");
            assert!(engine.list_members(sg).is_empty());
        }

        // GG-005: Remove member - subgraph not found
        #[test]
        fn test_engine_remove_member_subgraph_not_found() {
            let dir = tempdir().expect("tempdir");
            let mut engine = test_engine_sg(dir.path());
            let n1 = engine.create_node(vec![], vec![]);
            let result = engine.remove_member(SubgraphId(999), n1);
            assert!(result.is_err());
        }

        // GG-005: List members of empty subgraph
        #[test]
        fn test_engine_list_members_empty() {
            let dir = tempdir().expect("tempdir");
            let mut engine = test_engine_sg(dir.path());
            let sg = engine.create_subgraph(vec![], None);
            assert!(engine.list_members(sg).is_empty());
        }

        // GG-005: Get subgraph memberships for node
        #[test]
        fn test_engine_get_subgraph_memberships() {
            let dir = tempdir().expect("tempdir");
            let mut engine = test_engine_sg(dir.path());
            let sg1 = engine.create_subgraph(vec![], None);
            let sg2 = engine.create_subgraph(vec![], None);
            let n1 = engine.create_node(vec![], vec![]);
            engine.add_member(sg1, n1).expect("add1");
            engine.add_member(sg2, n1).expect("add2");
            let memberships = engine.get_subgraph_memberships(n1);
            assert_eq!(memberships.len(), 2);
            assert!(memberships.contains(&sg1));
            assert!(memberships.contains(&sg2));
        }

        // GG-005: Delete subgraph cascades membership removal
        #[test]
        fn test_engine_delete_subgraph_cascades_memberships() {
            let dir = tempdir().expect("tempdir");
            let mut engine = test_engine_sg(dir.path());
            let sg = engine.create_subgraph(vec![], None);
            let n1 = engine.create_node(vec![], vec![]);
            let n2 = engine.create_node(vec![], vec![]);
            engine.add_member(sg, n1).expect("add1");
            engine.add_member(sg, n2).expect("add2");
            engine.delete_subgraph(sg).expect("delete");
            // Memberships should be gone
            assert!(engine.get_subgraph_memberships(n1).is_empty());
            assert!(engine.get_subgraph_memberships(n2).is_empty());
        }
    }

    // ======================================================================
    // HH-005: StorageEngine hyperedge integration tests
    // ======================================================================

    #[cfg(feature = "hypergraph")]
    mod hypergraph_engine_tests {
        use super::*;
        use cypherlite_core::{GraphEntity, HyperEdgeId};

        fn test_engine_hg(dir: &std::path::Path) -> StorageEngine {
            let config = DatabaseConfig {
                path: dir.join("test.cyl"),
                wal_sync_mode: SyncMode::Normal,
                ..Default::default()
            };
            StorageEngine::open(config).expect("open")
        }

        // HH-005: Create hyperedge via StorageEngine
        #[test]
        fn test_storage_engine_create_hyperedge() {
            let dir = tempdir().expect("tempdir");
            let mut engine = test_engine_hg(dir.path());
            let n1 = engine.create_node(vec![], vec![]);
            let n2 = engine.create_node(vec![], vec![]);
            let he = engine.create_hyperedge(
                1,
                vec![GraphEntity::Node(n1)],
                vec![GraphEntity::Node(n2)],
                vec![],
            );
            assert_eq!(he, HyperEdgeId(1));
        }

        // HH-005: Get hyperedge via StorageEngine
        #[test]
        fn test_storage_engine_get_hyperedge() {
            let dir = tempdir().expect("tempdir");
            let mut engine = test_engine_hg(dir.path());
            let n1 = engine.create_node(vec![], vec![]);
            let n2 = engine.create_node(vec![], vec![]);
            let he = engine.create_hyperedge(
                5,
                vec![GraphEntity::Node(n1)],
                vec![GraphEntity::Node(n2)],
                vec![(1, PropertyValue::Int64(42))],
            );
            let record = engine.get_hyperedge(he).expect("found");
            assert_eq!(record.id, he);
            assert_eq!(record.rel_type_id, 5);
            assert_eq!(record.sources.len(), 1);
            assert_eq!(record.targets.len(), 1);
            assert_eq!(record.properties.len(), 1);
        }

        // HH-005: Get nonexistent hyperedge returns None
        #[test]
        fn test_storage_engine_get_nonexistent_hyperedge() {
            let dir = tempdir().expect("tempdir");
            let engine = test_engine_hg(dir.path());
            assert!(engine.get_hyperedge(HyperEdgeId(999)).is_none());
        }

        // HH-005: Delete hyperedge via StorageEngine
        #[test]
        fn test_storage_engine_delete_hyperedge() {
            let dir = tempdir().expect("tempdir");
            let mut engine = test_engine_hg(dir.path());
            let he = engine.create_hyperedge(1, vec![], vec![], vec![]);
            engine.delete_hyperedge(he).expect("delete");
            assert!(engine.get_hyperedge(he).is_none());
        }

        // HH-005: Delete nonexistent hyperedge returns error
        #[test]
        fn test_storage_engine_delete_nonexistent_hyperedge() {
            let dir = tempdir().expect("tempdir");
            let mut engine = test_engine_hg(dir.path());
            let result = engine.delete_hyperedge(HyperEdgeId(999));
            assert!(result.is_err());
        }

        // HH-005: Reverse index is updated on create/delete
        #[test]
        fn test_storage_engine_reverse_index_update() {
            let dir = tempdir().expect("tempdir");
            let mut engine = test_engine_hg(dir.path());
            let n1 = engine.create_node(vec![], vec![]);
            let n2 = engine.create_node(vec![], vec![]);
            let he = engine.create_hyperedge(
                1,
                vec![GraphEntity::Node(n1)],
                vec![GraphEntity::Node(n2)],
                vec![],
            );
            // n1 participates in he
            let hes = engine.hyperedges_for_entity(n1.0);
            assert_eq!(hes.len(), 1);
            assert_eq!(hes[0], he.0);
            // n2 participates in he
            let hes = engine.hyperedges_for_entity(n2.0);
            assert_eq!(hes.len(), 1);
            // Delete hyperedge should clean reverse index
            engine.delete_hyperedge(he).expect("delete");
            assert!(engine.hyperedges_for_entity(n1.0).is_empty());
            assert!(engine.hyperedges_for_entity(n2.0).is_empty());
        }

        // HH-005: Scan all hyperedges
        #[test]
        fn test_storage_engine_scan_hyperedges() {
            let dir = tempdir().expect("tempdir");
            let mut engine = test_engine_hg(dir.path());
            engine.create_hyperedge(1, vec![], vec![], vec![]);
            engine.create_hyperedge(2, vec![], vec![], vec![]);
            let all = engine.scan_hyperedges();
            assert_eq!(all.len(), 2);
        }

        // HH-005: Header next_hyperedge_id is synced
        #[test]
        fn test_storage_engine_header_sync() {
            let dir = tempdir().expect("tempdir");
            let mut engine = test_engine_hg(dir.path());
            engine.create_hyperedge(1, vec![], vec![], vec![]);
            engine.create_hyperedge(2, vec![], vec![], vec![]);
            // After creating 2 hyperedges, next_hyperedge_id should be 3
            assert_eq!(engine.page_manager.header().next_hyperedge_id, 3);
        }
    }
}
