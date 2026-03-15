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

use std::collections::HashMap;

use cypherlite_core::{
    CypherLiteError, DatabaseConfig, EdgeId, LabelRegistry, NodeId, NodeRecord, PageId,
    PropertyValue, RelationshipRecord, Result,
};
#[cfg(feature = "subgraph")]
use cypherlite_core::{SubgraphId, SubgraphRecord};
use fs2::FileExt;

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
    /// Exclusive file lock on the .cyl file, held for the engine's lifetime.
    /// Dropped automatically when StorageEngine is dropped, releasing the lock.
    lock_file: std::fs::File,
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
    /// Maps node_id -> data page ID where its record is stored.
    node_page_map: HashMap<u64, u32>,
    /// Maps edge_id -> data page ID where its record is stored.
    edge_page_map: HashMap<u64, u32>,
    /// Current node data page with free space: (page_id, page_buffer).
    current_node_data_page: Option<(u32, [u8; PAGE_SIZE])>,
    /// Current edge data page with free space: (page_id, page_buffer).
    current_edge_data_page: Option<(u32, [u8; PAGE_SIZE])>,
    /// Current subgraph data page with free space.
    #[cfg(feature = "subgraph")]
    current_subgraph_data_page: Option<(u32, [u8; PAGE_SIZE])>,
    /// Current hyperedge data page with free space.
    #[cfg(feature = "hypergraph")]
    current_hyperedge_data_page: Option<(u32, [u8; PAGE_SIZE])>,
    /// Current version data page with free space.
    current_version_data_page: Option<(u32, [u8; PAGE_SIZE])>,
}

impl StorageEngine {
    /// Open or create a CypherLite database.
    ///
    /// Acquires an exclusive file lock (flock) on the `.cyl` file. If the lock
    /// cannot be acquired (e.g. another process holds it), returns
    /// [`CypherLiteError::DatabaseLocked`].
    pub fn open(config: DatabaseConfig) -> Result<Self> {
        let wal_path = config.wal_path();
        let db_exists = config.path.exists();

        // R-PERSIST-035: Acquire exclusive file lock on .cyl file.
        // Use a .lock sidecar file so we don't interfere with PageManager's
        // own file I/O (creating an empty .cyl before PageManager would break
        // its exists() check).
        let lock_path = config.path.with_extension("cyl-lock");
        let lock_file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&lock_path)
            .map_err(CypherLiteError::IoError)?;

        lock_file
            .try_lock_exclusive()
            .map_err(|_| CypherLiteError::DatabaseLocked(config.path.display().to_string()))?;

        // Try to open existing database, or create a new one
        let mut page_manager = if db_exists {
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

        let mut engine = Self {
            lock_file,
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
            node_page_map: HashMap::new(),
            edge_page_map: HashMap::new(),
            current_node_data_page: None,
            current_edge_data_page: None,
            #[cfg(feature = "subgraph")]
            current_subgraph_data_page: None,
            #[cfg(feature = "hypergraph")]
            current_hyperedge_data_page: None,
            current_version_data_page: None,
        };

        // R-PERSIST-012: Load catalog from persisted pages before node/edge loading
        // so that label/property/rel-type IDs are available.
        engine.load_catalog()?;

        // R-PERSIST-031/032: Load persisted data from disk pages into memory.
        engine.load_nodes_from_pages()?;
        engine.load_edges_from_pages()?;
        // R-PERSIST-050/051/052: Load feature-gated store data from disk.
        #[cfg(feature = "subgraph")]
        engine.load_subgraphs_from_pages()?;
        #[cfg(feature = "hypergraph")]
        engine.load_hyperedges_from_pages()?;
        engine.load_versions_from_pages()?;

        Ok(engine)
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
        // R-PERSIST-001: Persist node record to data page via WAL
        if let Some(record) = self.node_store.get_node(id).cloned() {
            let _ = self.persist_node(id, &record, false);
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
                let seq = self.version_store.snapshot_node(node_id.0, old.clone());
                // R-PERSIST-052: Persist version record to data page via WAL
                let vr = version::VersionRecord::Node(old.clone());
                let _ = self.persist_version(node_id.0, seq, &vr);
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
        // R-PERSIST-003: Persist updated node record to data page via WAL
        if let Some(updated) = self.node_store.get_node(node_id).cloned() {
            let _ = self.rewrite_node_on_page(node_id, &updated, false);
        }
        Ok(())
    }

    /// Delete a node and all its connected edges.
    /// REQ-STORE-004: Delete all connected edges first.
    pub fn delete_node(&mut self, node_id: NodeId) -> Result<NodeRecord> {
        // Capture node data for index removal and tombstone before deletion
        let node_data = self.node_store.get_node(node_id).cloned();
        // Delete connected edges first
        self.edge_store
            .delete_edges_for_node(node_id, &mut self.node_store)?;
        let deleted = self.node_store.delete_node(node_id)?;
        // Remove from all applicable indexes
        if let Some(ref node) = node_data {
            for &label_id in &node.labels {
                for (prop_key_id, value) in &node.properties {
                    if let Some(idx) = self.index_manager.find_index_mut(label_id, *prop_key_id) {
                        idx.remove(value, node_id);
                    }
                }
            }
        }
        // R-PERSIST-004: Write tombstone record to data page via WAL
        if let Some(ref node) = node_data {
            let _ = self.rewrite_node_on_page(node_id, node, true);
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
        // R-PERSIST-002: Persist edge record to data page via WAL
        if let Some(record) = self.edge_store.get_edge(id).cloned() {
            let _ = self.persist_edge(id, &record, false);
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
        // R-PERSIST-003: Persist updated edge record to data page via WAL
        if let Some(updated) = self.edge_store.get_edge(edge_id).cloned() {
            let _ = self.rewrite_edge_on_page(edge_id, &updated, false);
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
        // CC-T5: Capture data for index removal and tombstone
        let edge_data = self.edge_store.get_edge(edge_id).cloned();
        let deleted = self.edge_store.delete_edge(edge_id, &mut self.node_store)?;
        // Remove from edge indexes
        if let Some(ref edge) = edge_data {
            for (prop_key_id, value) in &edge.properties {
                if let Some(idx) = self
                    .edge_index_manager
                    .find_index_mut(edge.rel_type_id, *prop_key_id)
                {
                    idx.remove(value, edge_id);
                }
            }
        }
        // R-PERSIST-004: Write tombstone record to data page via WAL
        if let Some(ref edge) = edge_data {
            let _ = self.rewrite_edge_on_page(edge_id, edge, true);
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
        // R-PERSIST-050: Persist subgraph record to data page via WAL
        if let Some(record) = self.subgraph_store.get(id).cloned() {
            let members = self.membership_index.members(id);
            let _ = self.persist_subgraph(id, &record, &members, false);
        }
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
        // R-PERSIST-050: Re-persist subgraph record with updated membership
        if let Some(record) = self.subgraph_store.get(subgraph_id).cloned() {
            let members = self.membership_index.members(subgraph_id);
            let _ = self.persist_subgraph(subgraph_id, &record, &members, false);
        }
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
        // R-PERSIST-051: Persist hyperedge record to data page via WAL
        if let Some(record) = self.hyperedge_store.get(id).cloned() {
            let _ = self.persist_hyperedge(id, &record, false);
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

    // -- Data page persistence (Phase 2: R-PERSIST-001..004) --

    /// Returns the node data root page ID from the database header.
    pub fn node_data_root_page(&self) -> u32 {
        self.page_manager.header().node_data_root_page
    }

    /// Returns the edge data root page ID from the database header.
    pub fn edge_data_root_page(&self) -> u32 {
        self.page_manager.header().edge_data_root_page
    }

    /// Returns the number of version snapshots for a given entity.
    pub fn version_count(&self, entity_id: u64) -> u64 {
        self.version_store.version_count(entity_id)
    }

    /// Returns the version chain for a given entity (oldest to newest).
    pub fn version_chain(&self, entity_id: u64) -> Vec<(u64, &version::VersionRecord)> {
        self.version_store.get_version_chain(entity_id)
    }

    /// Read a data page by page ID.
    ///
    /// Returns the in-memory cached copy if the page is the current active
    /// data page (which may contain WAL-only writes not yet checkpointed).
    /// Otherwise reads from the main database file.
    pub fn read_data_page(&mut self, page_id: u32) -> Result<[u8; PAGE_SIZE]> {
        // Check cached node data page first
        if let Some((cached_pid, ref cached_buf)) = self.current_node_data_page {
            if cached_pid == page_id {
                return Ok(*cached_buf);
            }
        }
        // Check cached edge data page
        if let Some((cached_pid, ref cached_buf)) = self.current_edge_data_page {
            if cached_pid == page_id {
                return Ok(*cached_buf);
            }
        }
        // Check cached subgraph data page
        #[cfg(feature = "subgraph")]
        if let Some((cached_pid, ref cached_buf)) = self.current_subgraph_data_page {
            if cached_pid == page_id {
                return Ok(*cached_buf);
            }
        }
        // Check cached hyperedge data page
        #[cfg(feature = "hypergraph")]
        if let Some((cached_pid, ref cached_buf)) = self.current_hyperedge_data_page {
            if cached_pid == page_id {
                return Ok(*cached_buf);
            }
        }
        // Check cached version data page
        if let Some((cached_pid, ref cached_buf)) = self.current_version_data_page {
            if cached_pid == page_id {
                return Ok(*cached_buf);
            }
        }
        // Fall back to main file (reads checkpointed data)
        self.page_manager.read_page(PageId(page_id))
    }

    /// Returns the number of committed WAL frames.
    pub fn wal_frame_count(&self) -> u64 {
        self.wal_writer.frame_count()
    }

    /// Returns the number of node data pages currently allocated.
    pub fn node_data_page_count(&self) -> usize {
        let root = self.page_manager.header().node_data_root_page;
        if root == 0 {
            return 0;
        }
        // Count unique pages referenced in node_page_map + current page
        let mut pages: std::collections::HashSet<u32> =
            self.node_page_map.values().copied().collect();
        if let Some((pid, _)) = &self.current_node_data_page {
            pages.insert(*pid);
        }
        pages.len()
    }

    /// Load all persisted node records from data pages into the in-memory NodeStore.
    ///
    /// Walks the node data page chain starting from `node_data_root_page` in the
    /// database header. For each page, deserializes all records and inserts
    /// non-tombstone records into the NodeStore. Also rebuilds `node_page_map`
    /// and sets `current_node_data_page` to the last page in the chain.
    ///
    /// R-PERSIST-031: After WAL recovery, all node data pages MUST be read and
    /// deserialized into NodeStore.
    fn load_nodes_from_pages(&mut self) -> Result<()> {
        use page::record_serialization::{
            deserialize_node_record, read_records_from_page, DataPageHeader,
        };

        let root_page = self.page_manager.header().node_data_root_page;
        if root_page == 0 {
            return Ok(()); // No node data persisted
        }

        let mut current_page_id = root_page;
        loop {
            let page_buf = self.page_manager.read_page(PageId(current_page_id))?;
            let header = DataPageHeader::read_from(&page_buf);

            // Read and deserialize all records from this page
            let entries = read_records_from_page(&page_buf);
            for (off, len) in &entries {
                if let Some((record, deleted, _)) =
                    deserialize_node_record(&page_buf[*off..*off + *len])
                {
                    if !deleted {
                        self.node_page_map.insert(record.node_id.0, current_page_id);
                        self.node_store.insert_loaded_record(record);
                    }
                }
            }

            // Follow the page chain or stop
            if header.next_page == 0 {
                // Last page in chain -- cache it for future appends
                self.current_node_data_page = Some((current_page_id, page_buf));
                break;
            }
            current_page_id = header.next_page;
        }

        Ok(())
    }

    /// Load all persisted edge records from data pages into the in-memory EdgeStore.
    ///
    /// Walks the edge data page chain starting from `edge_data_root_page` in the
    /// database header. For each page, deserializes all records and inserts
    /// non-tombstone records into the EdgeStore. Also rebuilds `edge_page_map`
    /// and sets `current_edge_data_page` to the last page in the chain.
    ///
    /// R-PERSIST-032: After WAL recovery, all edge data pages MUST be read and
    /// deserialized into EdgeStore.
    fn load_edges_from_pages(&mut self) -> Result<()> {
        use page::record_serialization::{
            deserialize_edge_record, read_records_from_page, DataPageHeader,
        };

        let root_page = self.page_manager.header().edge_data_root_page;
        if root_page == 0 {
            return Ok(()); // No edge data persisted
        }

        let mut current_page_id = root_page;
        loop {
            let page_buf = self.page_manager.read_page(PageId(current_page_id))?;
            let header = DataPageHeader::read_from(&page_buf);

            let entries = read_records_from_page(&page_buf);
            for (off, len) in &entries {
                if let Some((record, deleted, _)) =
                    deserialize_edge_record(&page_buf[*off..*off + *len])
                {
                    if !deleted {
                        self.edge_page_map.insert(record.edge_id.0, current_page_id);
                        self.edge_store.insert_loaded_record(record);
                    }
                }
            }

            if header.next_page == 0 {
                self.current_edge_data_page = Some((current_page_id, page_buf));
                break;
            }
            current_page_id = header.next_page;
        }

        Ok(())
    }

    /// R-PERSIST-010: Save the catalog to one or more CatalogData pages.
    ///
    /// Serializes the in-memory `Catalog` via `catalog.save()` (bincode) and
    /// writes the resulting bytes across chained CatalogData pages.  The first
    /// page ID is stored in `DatabaseHeader.catalog_page_id`.
    fn save_catalog(&mut self) -> Result<()> {
        use page::record_serialization::DataPageHeader;
        use page::PageType;

        let catalog_bytes = self.catalog.save();
        if catalog_bytes.is_empty() {
            return Ok(());
        }

        let usable_per_page = PAGE_SIZE - DataPageHeader::SIZE;
        let mut first_page_id: Option<u32> = None;
        let mut prev_page: Option<(u32, [u8; PAGE_SIZE])> = None;

        for chunk in catalog_bytes.chunks(usable_per_page) {
            let new_page_id = self.page_manager.allocate_page()?;
            let mut page_buf = [0u8; PAGE_SIZE];
            let mut header = DataPageHeader::new(PageType::CatalogData as u8);
            header.free_offset = (DataPageHeader::SIZE + chunk.len()) as u16;
            header.record_count = 1; // treat as single blob fragment
            header.write_to(&mut page_buf);

            // Write chunk data after header
            page_buf[DataPageHeader::SIZE..DataPageHeader::SIZE + chunk.len()]
                .copy_from_slice(chunk);

            if first_page_id.is_none() {
                first_page_id = Some(new_page_id.0);
            }

            // Chain previous page to this one
            if let Some((prev_id, ref mut prev_buf)) = prev_page {
                let mut prev_header = DataPageHeader::read_from(prev_buf);
                prev_header.next_page = new_page_id.0;
                prev_header.write_to(prev_buf);
                // Write previous page through WAL
                let db_size = self.page_manager.header().page_count;
                self.wal_writer
                    .write_frame(PageId(prev_id), db_size, prev_buf)?;
            }

            prev_page = Some((new_page_id.0, page_buf));
        }

        // Write the last page
        if let Some((last_id, ref last_buf)) = prev_page {
            let db_size = self.page_manager.header().page_count;
            self.wal_writer
                .write_frame(PageId(last_id), db_size, last_buf)?;
            self.wal_writer.commit()?;
        }

        // Update header with catalog root page
        if let Some(root_id) = first_page_id {
            self.page_manager.header_mut().catalog_page_id = root_id;
        }

        Ok(())
    }

    /// R-PERSIST-012: Load catalog from CatalogData pages on database open.
    ///
    /// Reads the chained CatalogData pages starting from
    /// `DatabaseHeader.catalog_page_id`, concatenates the data payloads, and
    /// deserializes via `Catalog::load()`.
    fn load_catalog(&mut self) -> Result<()> {
        use page::record_serialization::DataPageHeader;

        let root_page = self.page_manager.header().catalog_page_id;
        if root_page == 0 {
            return Ok(()); // No catalog data persisted — use default
        }

        let mut catalog_bytes = Vec::new();
        let mut current_page_id = root_page;

        loop {
            let page_buf = self.page_manager.read_page(PageId(current_page_id))?;
            let header = DataPageHeader::read_from(&page_buf);

            // Extract payload between header and free_offset
            let data_start = DataPageHeader::SIZE;
            let data_end = header.free_offset as usize;
            if data_end > data_start && data_end <= PAGE_SIZE {
                catalog_bytes.extend_from_slice(&page_buf[data_start..data_end]);
            }

            if header.next_page == 0 {
                break;
            }
            current_page_id = header.next_page;
        }

        self.catalog = catalog::Catalog::load(&catalog_bytes)?;
        Ok(())
    }

    /// Persist a node record to a data page and write through WAL.
    fn persist_node(&mut self, node_id: NodeId, record: &NodeRecord, deleted: bool) -> Result<()> {
        use page::record_serialization::{
            pack_record_into_page, serialize_node_record, DataPageHeader,
        };
        use page::PageType;

        let record_bytes = serialize_node_record(record, deleted);

        // Try to pack into current node data page
        if let Some((page_id, ref mut page_buf)) = self.current_node_data_page {
            if pack_record_into_page(page_buf, &record_bytes) {
                // Write the page through WAL
                let db_size = self.page_manager.header().page_count;
                self.wal_writer
                    .write_frame(PageId(page_id), db_size, page_buf)?;
                self.wal_writer.commit()?;
                self.node_page_map.insert(node_id.0, page_id);
                return Ok(());
            }
        }

        // Current page is full or doesn't exist -- allocate a new one
        let new_page_id = self.page_manager.allocate_page()?;
        let mut new_page = [0u8; PAGE_SIZE];
        let header = DataPageHeader::new(PageType::NodeData as u8);
        header.write_to(&mut new_page);

        // Chain the new page to the previous one
        if let Some((old_page_id, ref mut old_buf)) = self.current_node_data_page {
            // Update old page's next_page pointer
            let mut old_header = DataPageHeader::read_from(old_buf);
            old_header.next_page = new_page_id.0;
            old_header.write_to(old_buf);
            // Write old page with updated chain pointer
            let db_size = self.page_manager.header().page_count;
            self.wal_writer
                .write_frame(PageId(old_page_id), db_size, old_buf)?;
            self.wal_writer.commit()?;
        }

        // Pack record into new page
        let packed = pack_record_into_page(&mut new_page, &record_bytes);
        debug_assert!(packed, "fresh page should always have space for a record");

        // Write new page through WAL
        let db_size = self.page_manager.header().page_count;
        self.wal_writer
            .write_frame(new_page_id, db_size, &new_page)?;
        self.wal_writer.commit()?;

        // Update header if this is the first node data page
        if self.page_manager.header().node_data_root_page == 0 {
            self.page_manager.header_mut().node_data_root_page = new_page_id.0;
            self.page_manager.flush_header()?;
        }

        self.node_page_map.insert(node_id.0, new_page_id.0);
        self.current_node_data_page = Some((new_page_id.0, new_page));

        Ok(())
    }

    /// Persist an edge record to a data page and write through WAL.
    fn persist_edge(
        &mut self,
        edge_id: EdgeId,
        record: &RelationshipRecord,
        deleted: bool,
    ) -> Result<()> {
        use page::record_serialization::{
            pack_record_into_page, serialize_edge_record, DataPageHeader,
        };
        use page::PageType;

        let record_bytes = serialize_edge_record(record, deleted);

        // Try to pack into current edge data page
        if let Some((page_id, ref mut page_buf)) = self.current_edge_data_page {
            if pack_record_into_page(page_buf, &record_bytes) {
                let db_size = self.page_manager.header().page_count;
                self.wal_writer
                    .write_frame(PageId(page_id), db_size, page_buf)?;
                self.wal_writer.commit()?;
                self.edge_page_map.insert(edge_id.0, page_id);
                return Ok(());
            }
        }

        // Allocate new edge data page
        let new_page_id = self.page_manager.allocate_page()?;
        let mut new_page = [0u8; PAGE_SIZE];
        let header = DataPageHeader::new(PageType::EdgeData as u8);
        header.write_to(&mut new_page);

        // Chain to previous page
        if let Some((old_page_id, ref mut old_buf)) = self.current_edge_data_page {
            let mut old_header = DataPageHeader::read_from(old_buf);
            old_header.next_page = new_page_id.0;
            old_header.write_to(old_buf);
            let db_size = self.page_manager.header().page_count;
            self.wal_writer
                .write_frame(PageId(old_page_id), db_size, old_buf)?;
            self.wal_writer.commit()?;
        }

        let packed = pack_record_into_page(&mut new_page, &record_bytes);
        debug_assert!(packed, "fresh page should always have space for a record");

        let db_size = self.page_manager.header().page_count;
        self.wal_writer
            .write_frame(new_page_id, db_size, &new_page)?;
        self.wal_writer.commit()?;

        if self.page_manager.header().edge_data_root_page == 0 {
            self.page_manager.header_mut().edge_data_root_page = new_page_id.0;
            self.page_manager.flush_header()?;
        }

        self.edge_page_map.insert(edge_id.0, new_page_id.0);
        self.current_edge_data_page = Some((new_page_id.0, new_page));

        Ok(())
    }

    /// Rewrite a node record on its existing data page (for update/delete).
    /// This rewrites the entire page with the updated record replacing the old one.
    fn rewrite_node_on_page(
        &mut self,
        node_id: NodeId,
        record: &NodeRecord,
        deleted: bool,
    ) -> Result<()> {
        use page::record_serialization::{
            deserialize_node_record, pack_record_into_page, read_records_from_page,
            serialize_node_record, DataPageHeader,
        };

        let page_id = match self.node_page_map.get(&node_id.0) {
            Some(&pid) => pid,
            None => {
                // Node was never persisted; write as new record
                return self.persist_node(node_id, record, deleted);
            }
        };

        // Read the current page (from cache or main file)
        let old_page = self.read_data_page(page_id)?;
        let old_header = DataPageHeader::read_from(&old_page);

        // Rebuild the page: copy all records except the one being updated
        let mut new_page = [0u8; PAGE_SIZE];
        let mut new_header = DataPageHeader::new(old_header.page_type);
        new_header.next_page = old_header.next_page;
        new_header.write_to(&mut new_page);

        let entries = read_records_from_page(&old_page);
        for (off, len) in &entries {
            let slice = &old_page[*off..*off + *len];
            if let Some((rec, _del, _)) = deserialize_node_record(slice) {
                if rec.node_id == node_id {
                    // Replace with updated record
                    let updated_bytes = serialize_node_record(record, deleted);
                    pack_record_into_page(&mut new_page, &updated_bytes);
                } else {
                    // Copy existing record as-is
                    pack_record_into_page(&mut new_page, slice);
                }
            }
        }

        // Write updated page through WAL
        let db_size = self.page_manager.header().page_count;
        self.wal_writer
            .write_frame(PageId(page_id), db_size, &new_page)?;
        self.wal_writer.commit()?;

        // Update cached page if it matches
        if let Some((cached_pid, ref mut cached_buf)) = self.current_node_data_page {
            if cached_pid == page_id {
                *cached_buf = new_page;
            }
        }

        Ok(())
    }

    /// Rewrite an edge record on its existing data page (for update/delete).
    fn rewrite_edge_on_page(
        &mut self,
        edge_id: EdgeId,
        record: &RelationshipRecord,
        deleted: bool,
    ) -> Result<()> {
        use page::record_serialization::{
            deserialize_edge_record, pack_record_into_page, read_records_from_page,
            serialize_edge_record, DataPageHeader,
        };

        let page_id = match self.edge_page_map.get(&edge_id.0) {
            Some(&pid) => pid,
            None => {
                return self.persist_edge(edge_id, record, deleted);
            }
        };

        // Read the current page (from cache or main file)
        let old_page = self.read_data_page(page_id)?;
        let old_header = DataPageHeader::read_from(&old_page);

        let mut new_page = [0u8; PAGE_SIZE];
        let mut new_header = DataPageHeader::new(old_header.page_type);
        new_header.next_page = old_header.next_page;
        new_header.write_to(&mut new_page);

        let entries = read_records_from_page(&old_page);
        for (off, len) in &entries {
            let slice = &old_page[*off..*off + *len];
            if let Some((rec, _del, _)) = deserialize_edge_record(slice) {
                if rec.edge_id == edge_id {
                    let updated_bytes = serialize_edge_record(record, deleted);
                    pack_record_into_page(&mut new_page, &updated_bytes);
                } else {
                    pack_record_into_page(&mut new_page, slice);
                }
            }
        }

        let db_size = self.page_manager.header().page_count;
        self.wal_writer
            .write_frame(PageId(page_id), db_size, &new_page)?;
        self.wal_writer.commit()?;

        if let Some((cached_pid, ref mut cached_buf)) = self.current_edge_data_page {
            if cached_pid == page_id {
                *cached_buf = new_page;
            }
        }

        Ok(())
    }

    // ================================================================
    // PERSIST-001 Phase 5: SubgraphStore persistence
    // ================================================================

    /// Load all persisted subgraph records from data pages into the in-memory
    /// SubgraphStore and MembershipIndex.
    #[cfg(feature = "subgraph")]
    fn load_subgraphs_from_pages(&mut self) -> Result<()> {
        use page::record_serialization::{
            deserialize_subgraph_record, read_records_from_page, DataPageHeader,
        };

        let root_page = self.page_manager.header().subgraph_data_root_page;
        if root_page == 0 {
            return Ok(());
        }

        let mut current_page_id = root_page;
        loop {
            let page_buf = self.page_manager.read_page(PageId(current_page_id))?;
            let header = DataPageHeader::read_from(&page_buf);

            let entries = read_records_from_page(&page_buf);
            for (off, len) in &entries {
                if let Some((record, members, deleted, _)) =
                    deserialize_subgraph_record(&page_buf[*off..*off + *len])
                {
                    if !deleted {
                        let sg_id = record.subgraph_id;
                        self.subgraph_store.insert_loaded_record(record);
                        // Rebuild membership index from persisted member lists
                        for node_id in members {
                            self.membership_index.add(sg_id, node_id);
                        }
                    }
                }
            }

            if header.next_page == 0 {
                self.current_subgraph_data_page = Some((current_page_id, page_buf));
                break;
            }
            current_page_id = header.next_page;
        }

        Ok(())
    }

    /// Persist a subgraph record (with membership data) to a data page via WAL.
    #[cfg(feature = "subgraph")]
    fn persist_subgraph(
        &mut self,
        _id: SubgraphId,
        record: &SubgraphRecord,
        members: &[NodeId],
        deleted: bool,
    ) -> Result<()> {
        use page::record_serialization::{
            pack_record_into_page, serialize_subgraph_record, DataPageHeader,
        };
        use page::PageType;

        let record_bytes = serialize_subgraph_record(record, members, deleted);

        // Try to pack into current subgraph data page
        if let Some((page_id, ref mut page_buf)) = self.current_subgraph_data_page {
            if pack_record_into_page(page_buf, &record_bytes) {
                let db_size = self.page_manager.header().page_count;
                self.wal_writer
                    .write_frame(PageId(page_id), db_size, page_buf)?;
                self.wal_writer.commit()?;
                return Ok(());
            }
        }

        // Allocate new subgraph data page
        let new_page_id = self.page_manager.allocate_page()?;
        let mut new_page = [0u8; PAGE_SIZE];
        let header = DataPageHeader::new(PageType::SubgraphData as u8);
        header.write_to(&mut new_page);

        // Chain to previous page
        if let Some((old_page_id, ref mut old_buf)) = self.current_subgraph_data_page {
            let mut old_header = DataPageHeader::read_from(old_buf);
            old_header.next_page = new_page_id.0;
            old_header.write_to(old_buf);
            let db_size = self.page_manager.header().page_count;
            self.wal_writer
                .write_frame(PageId(old_page_id), db_size, old_buf)?;
            self.wal_writer.commit()?;
        }

        let packed = pack_record_into_page(&mut new_page, &record_bytes);
        debug_assert!(packed, "fresh page should always have space for a record");

        let db_size = self.page_manager.header().page_count;
        self.wal_writer
            .write_frame(new_page_id, db_size, &new_page)?;
        self.wal_writer.commit()?;

        if self.page_manager.header().subgraph_data_root_page == 0 {
            self.page_manager.header_mut().subgraph_data_root_page = new_page_id.0;
            self.page_manager.flush_header()?;
        }

        self.current_subgraph_data_page = Some((new_page_id.0, new_page));

        Ok(())
    }

    // ================================================================
    // PERSIST-001 Phase 5: HyperEdgeStore persistence
    // ================================================================

    /// Load all persisted hyperedge records from data pages into the in-memory
    /// HyperEdgeStore and HyperEdgeReverseIndex.
    #[cfg(feature = "hypergraph")]
    fn load_hyperedges_from_pages(&mut self) -> Result<()> {
        use page::record_serialization::{
            deserialize_hyperedge_record, read_records_from_page, DataPageHeader,
        };

        let root_page = self.page_manager.header().hyperedge_data_root_page;
        if root_page == 0 {
            return Ok(());
        }

        let mut current_page_id = root_page;
        loop {
            let page_buf = self.page_manager.read_page(PageId(current_page_id))?;
            let header = DataPageHeader::read_from(&page_buf);

            let entries = read_records_from_page(&page_buf);
            for (off, len) in &entries {
                if let Some((record, deleted, _)) =
                    deserialize_hyperedge_record(&page_buf[*off..*off + *len])
                {
                    if !deleted {
                        let he_id = record.id;
                        // Rebuild reverse index
                        for entity in record.sources.iter().chain(record.targets.iter()) {
                            let raw_id = match entity {
                                cypherlite_core::GraphEntity::Node(nid) => nid.0,
                                cypherlite_core::GraphEntity::Subgraph(sid) => sid.0,
                                cypherlite_core::GraphEntity::HyperEdge(hid) => hid.0,
                                cypherlite_core::GraphEntity::TemporalRef(nid, _) => nid.0,
                            };
                            self.hyperedge_reverse_index.add(he_id.0, raw_id);
                        }
                        self.hyperedge_store.insert_loaded_record(record);
                    }
                }
            }

            if header.next_page == 0 {
                self.current_hyperedge_data_page = Some((current_page_id, page_buf));
                break;
            }
            current_page_id = header.next_page;
        }

        Ok(())
    }

    /// Persist a hyperedge record to a data page via WAL.
    #[cfg(feature = "hypergraph")]
    fn persist_hyperedge(
        &mut self,
        _id: HyperEdgeId,
        record: &HyperEdgeRecord,
        deleted: bool,
    ) -> Result<()> {
        use page::record_serialization::{
            pack_record_into_page, serialize_hyperedge_record, DataPageHeader,
        };
        use page::PageType;

        let record_bytes = serialize_hyperedge_record(record, deleted);

        // Try to pack into current hyperedge data page
        if let Some((page_id, ref mut page_buf)) = self.current_hyperedge_data_page {
            if pack_record_into_page(page_buf, &record_bytes) {
                let db_size = self.page_manager.header().page_count;
                self.wal_writer
                    .write_frame(PageId(page_id), db_size, page_buf)?;
                self.wal_writer.commit()?;
                return Ok(());
            }
        }

        // Allocate new hyperedge data page
        let new_page_id = self.page_manager.allocate_page()?;
        let mut new_page = [0u8; PAGE_SIZE];
        let header = DataPageHeader::new(PageType::HyperEdgeData as u8);
        header.write_to(&mut new_page);

        // Chain to previous page
        if let Some((old_page_id, ref mut old_buf)) = self.current_hyperedge_data_page {
            let mut old_header = DataPageHeader::read_from(old_buf);
            old_header.next_page = new_page_id.0;
            old_header.write_to(old_buf);
            let db_size = self.page_manager.header().page_count;
            self.wal_writer
                .write_frame(PageId(old_page_id), db_size, old_buf)?;
            self.wal_writer.commit()?;
        }

        let packed = pack_record_into_page(&mut new_page, &record_bytes);
        debug_assert!(packed, "fresh page should always have space for a record");

        let db_size = self.page_manager.header().page_count;
        self.wal_writer
            .write_frame(new_page_id, db_size, &new_page)?;
        self.wal_writer.commit()?;

        if self.page_manager.header().hyperedge_data_root_page == 0 {
            self.page_manager.header_mut().hyperedge_data_root_page = new_page_id.0;
            self.page_manager.flush_header()?;
        }

        self.current_hyperedge_data_page = Some((new_page_id.0, new_page));

        Ok(())
    }

    // ================================================================
    // PERSIST-001 Phase 5: VersionStore persistence
    // ================================================================

    /// Load all persisted version records from data pages into the in-memory
    /// VersionStore.
    fn load_versions_from_pages(&mut self) -> Result<()> {
        use page::record_serialization::{
            deserialize_version_record, read_records_from_page, DataPageHeader,
        };

        let root_page = self.page_manager.header().version_data_root_page;
        if root_page == 0 {
            return Ok(());
        }

        let mut current_page_id = root_page;
        loop {
            let page_buf = self.page_manager.read_page(PageId(current_page_id))?;
            let header = DataPageHeader::read_from(&page_buf);

            let entries = read_records_from_page(&page_buf);
            for (off, len) in &entries {
                if let Some((entity_id, version_seq, record, _)) =
                    deserialize_version_record(&page_buf[*off..*off + *len])
                {
                    self.version_store
                        .insert_loaded_record(entity_id, version_seq, record);
                }
            }

            if header.next_page == 0 {
                self.current_version_data_page = Some((current_page_id, page_buf));
                break;
            }
            current_page_id = header.next_page;
        }

        Ok(())
    }

    /// Persist a version record to a data page via WAL.
    fn persist_version(
        &mut self,
        entity_id: u64,
        version_seq: u64,
        record: &version::VersionRecord,
    ) -> Result<()> {
        use page::record_serialization::{
            pack_record_into_page, serialize_version_record, DataPageHeader,
        };
        use page::PageType;

        let record_bytes = serialize_version_record(entity_id, version_seq, record);

        // Try to pack into current version data page
        if let Some((page_id, ref mut page_buf)) = self.current_version_data_page {
            if pack_record_into_page(page_buf, &record_bytes) {
                let db_size = self.page_manager.header().page_count;
                self.wal_writer
                    .write_frame(PageId(page_id), db_size, page_buf)?;
                self.wal_writer.commit()?;
                return Ok(());
            }
        }

        // Allocate new version data page
        let new_page_id = self.page_manager.allocate_page()?;
        let mut new_page = [0u8; PAGE_SIZE];
        let header = DataPageHeader::new(PageType::VersionData as u8);
        header.write_to(&mut new_page);

        // Chain to previous page
        if let Some((old_page_id, ref mut old_buf)) = self.current_version_data_page {
            let mut old_header = DataPageHeader::read_from(old_buf);
            old_header.next_page = new_page_id.0;
            old_header.write_to(old_buf);
            let db_size = self.page_manager.header().page_count;
            self.wal_writer
                .write_frame(PageId(old_page_id), db_size, old_buf)?;
            self.wal_writer.commit()?;
        }

        let packed = pack_record_into_page(&mut new_page, &record_bytes);
        debug_assert!(packed, "fresh page should always have space for a record");

        let db_size = self.page_manager.header().page_count;
        self.wal_writer
            .write_frame(new_page_id, db_size, &new_page)?;
        self.wal_writer.commit()?;

        if self.page_manager.header().version_data_root_page == 0 {
            self.page_manager.header_mut().version_data_root_page = new_page_id.0;
            self.page_manager.flush_header()?;
        }

        self.current_version_data_page = Some((new_page_id.0, new_page));

        Ok(())
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
        // R-PERSIST-010: Persist catalog before closing.
        let _ = self.save_catalog();
        // R-PERSIST-037: On Drop, checkpoint executes, WAL deleted, then file lock released.
        // R-PERSIST-005: Flush header to persist next_node_id/next_edge_id before checkpoint.
        let _ = self.page_manager.flush_header();
        // Flush WAL to main database file, then delete WAL only if successful.
        // If checkpoint fails, WAL is preserved for crash recovery on next open.
        if self.checkpoint().is_ok() {
            let _ = std::fs::remove_file(self.config.wal_path());
        }
        // Lock file is released automatically when self.lock_file is dropped.
        // Clean up the .cyl-lock sidecar file.
        let _ = std::fs::remove_file(self.config.path.with_extension("cyl-lock"));
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
        }

        // Reopen - data pages loaded back into memory (R-PERSIST-005)
        {
            let engine = StorageEngine::open(config).expect("reopen");
            assert_eq!(engine.node_count(), 1);
            let node = engine.get_node(NodeId(1)).expect("node should be loaded");
            assert_eq!(node.labels, vec![1]);
            assert_eq!(
                node.properties[0],
                (1, PropertyValue::String("Alice".into()))
            );
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

    // ======================================================================
    // R-PERSIST-035..039: File locking tests
    // ======================================================================

    // R-PERSIST-038: Two StorageEngine instances MUST NOT open same .cyl file simultaneously
    #[test]
    fn test_second_open_returns_database_locked() {
        let dir = tempdir().expect("tempdir");
        let db_path = dir.path().join("lock_test.cyl");
        let config1 = DatabaseConfig {
            path: db_path.clone(),
            wal_sync_mode: SyncMode::Normal,
            ..Default::default()
        };
        let _engine1 = StorageEngine::open(config1).expect("first open should succeed");

        let config2 = DatabaseConfig {
            path: db_path.clone(),
            wal_sync_mode: SyncMode::Normal,
            ..Default::default()
        };
        let result = StorageEngine::open(config2);
        match result {
            Err(CypherLiteError::DatabaseLocked(ref msg)) => {
                assert!(
                    msg.contains("lock_test.cyl"),
                    "error should contain file path: {msg}"
                );
            }
            Err(other) => panic!("expected DatabaseLocked, got: {other}"),
            Ok(_) => panic!("expected DatabaseLocked error, but open succeeded"),
        }
    }

    // R-PERSIST-036: File lock held for entire StorageEngine lifetime, released only on Drop
    // R-PERSIST-037: On Drop, file lock released
    #[test]
    fn test_drop_releases_lock_then_reopen_succeeds() {
        let dir = tempdir().expect("tempdir");
        let db_path = dir.path().join("drop_test.cyl");
        let config = DatabaseConfig {
            path: db_path.clone(),
            wal_sync_mode: SyncMode::Normal,
            ..Default::default()
        };

        // Open and immediately drop
        {
            let _engine = StorageEngine::open(config.clone()).expect("first open");
        }
        // Should succeed after drop
        let _engine2 = StorageEngine::open(config).expect("reopen after drop should succeed");
    }

    // ======================================================================
    // TASK-019/020: R-PERSIST-001 create_node writes to WAL via data pages
    // ======================================================================

    #[test]
    fn test_create_node_sets_node_data_root_page() {
        let dir = tempdir().expect("tempdir");
        let mut engine = test_engine(dir.path());
        // Before any node creation, root page should be 0
        assert_eq!(engine.node_data_root_page(), 0);
        engine.create_node(vec![1], vec![(1, PropertyValue::String("Alice".into()))]);
        // After creation, a node data page should have been allocated
        assert_ne!(engine.node_data_root_page(), 0);
    }

    #[test]
    fn test_create_node_data_page_contains_record() {
        let dir = tempdir().expect("tempdir");
        let mut engine = test_engine(dir.path());
        let id = engine.create_node(vec![1, 2], vec![(1, PropertyValue::String("Alice".into()))]);
        // Read back the data page and verify the node is in it
        let page_id = engine.node_data_root_page();
        assert_ne!(page_id, 0);
        let page = engine.read_data_page(page_id).expect("read page");
        let entries = page::record_serialization::read_records_from_page(&page);
        assert_eq!(entries.len(), 1);
        // Deserialize and verify
        let (off, len) = entries[0];
        let (record, deleted, _) =
            page::record_serialization::deserialize_node_record(&page[off..off + len])
                .expect("deserialize");
        assert_eq!(record.node_id, id);
        assert_eq!(record.labels, vec![1, 2]);
        assert!(!deleted);
    }

    #[test]
    fn test_create_multiple_nodes_all_persisted() {
        let dir = tempdir().expect("tempdir");
        let mut engine = test_engine(dir.path());
        let mut ids = vec![];
        for i in 0..5u64 {
            let id = engine.create_node(vec![1], vec![(1, PropertyValue::Int64(i as i64))]);
            ids.push(id);
        }
        // All nodes should be in data pages
        let page_id = engine.node_data_root_page();
        let page = engine.read_data_page(page_id).expect("read page");
        let entries = page::record_serialization::read_records_from_page(&page);
        assert_eq!(entries.len(), 5);
    }

    // ======================================================================
    // TASK-021/022: R-PERSIST-002 create_edge writes to WAL via data pages
    // ======================================================================

    #[test]
    fn test_create_edge_sets_edge_data_root_page() {
        let dir = tempdir().expect("tempdir");
        let mut engine = test_engine(dir.path());
        assert_eq!(engine.edge_data_root_page(), 0);
        let n1 = engine.create_node(vec![], vec![]);
        let n2 = engine.create_node(vec![], vec![]);
        engine.create_edge(n1, n2, 1, vec![]).expect("edge");
        assert_ne!(engine.edge_data_root_page(), 0);
    }

    #[test]
    fn test_create_edge_data_page_contains_record() {
        let dir = tempdir().expect("tempdir");
        let mut engine = test_engine(dir.path());
        let n1 = engine.create_node(vec![], vec![]);
        let n2 = engine.create_node(vec![], vec![]);
        let eid = engine
            .create_edge(n1, n2, 5, vec![(1, PropertyValue::Int64(42))])
            .expect("edge");
        let page_id = engine.edge_data_root_page();
        let page = engine.read_data_page(page_id).expect("read page");
        let entries = page::record_serialization::read_records_from_page(&page);
        assert_eq!(entries.len(), 1);
        let (off, len) = entries[0];
        let (record, deleted, _) =
            page::record_serialization::deserialize_edge_record(&page[off..off + len])
                .expect("deserialize");
        assert_eq!(record.edge_id, eid);
        assert_eq!(record.start_node, n1);
        assert_eq!(record.end_node, n2);
        assert_eq!(record.rel_type_id, 5);
        assert!(!deleted);
    }

    // ======================================================================
    // TASK-023/024: R-PERSIST-003 update_node writes to WAL
    // ======================================================================

    #[test]
    fn test_update_node_rewrites_data_page() {
        let dir = tempdir().expect("tempdir");
        let mut engine = test_engine(dir.path());
        let id = engine.create_node(vec![1], vec![(1, PropertyValue::Int64(10))]);
        engine
            .update_node(id, vec![(1, PropertyValue::Int64(20))])
            .expect("update");
        // Find the node in data pages -- should have the updated value
        let page_id = engine.node_data_root_page();
        let page = engine.read_data_page(page_id).expect("read page");
        let entries = page::record_serialization::read_records_from_page(&page);
        // Find the entry for this node (may be the latest non-deleted version)
        let mut found = false;
        for (off, len) in &entries {
            let (record, deleted, _) =
                page::record_serialization::deserialize_node_record(&page[*off..*off + *len])
                    .expect("deserialize");
            if record.node_id == id && !deleted {
                assert_eq!(record.properties[0].1, PropertyValue::Int64(20));
                found = true;
            }
        }
        assert!(found, "updated node record should be in data page");
    }

    // ======================================================================
    // TASK-025/026: R-PERSIST-004 delete_node/edge writes tombstone
    // ======================================================================

    #[test]
    fn test_delete_node_writes_tombstone() {
        let dir = tempdir().expect("tempdir");
        let mut engine = test_engine(dir.path());
        let id = engine.create_node(vec![1], vec![]);
        engine.delete_node(id).expect("delete");
        // Check that data page contains a tombstone record
        let page_id = engine.node_data_root_page();
        let page = engine.read_data_page(page_id).expect("read page");
        let entries = page::record_serialization::read_records_from_page(&page);
        let mut tombstone_found = false;
        for (off, len) in &entries {
            let (record, deleted, _) =
                page::record_serialization::deserialize_node_record(&page[*off..*off + *len])
                    .expect("deserialize");
            if record.node_id == id && deleted {
                tombstone_found = true;
            }
        }
        assert!(
            tombstone_found,
            "deleted node should have a tombstone record"
        );
    }

    #[test]
    fn test_delete_edge_writes_tombstone() {
        let dir = tempdir().expect("tempdir");
        let mut engine = test_engine(dir.path());
        let n1 = engine.create_node(vec![], vec![]);
        let n2 = engine.create_node(vec![], vec![]);
        let eid = engine.create_edge(n1, n2, 1, vec![]).expect("edge");
        engine.delete_edge(eid).expect("delete");
        let page_id = engine.edge_data_root_page();
        let page = engine.read_data_page(page_id).expect("read page");
        let entries = page::record_serialization::read_records_from_page(&page);
        let mut tombstone_found = false;
        for (off, len) in &entries {
            let (record, deleted, _) =
                page::record_serialization::deserialize_edge_record(&page[*off..*off + *len])
                    .expect("deserialize");
            if record.edge_id == eid && deleted {
                tombstone_found = true;
            }
        }
        assert!(
            tombstone_found,
            "deleted edge should have a tombstone record"
        );
    }

    // ======================================================================
    // TASK-029: WAL commit verification
    // ======================================================================

    #[test]
    fn test_create_node_wal_has_committed_frames() {
        let dir = tempdir().expect("tempdir");
        let mut engine = test_engine(dir.path());
        engine.create_node(vec![1], vec![(1, PropertyValue::Int64(42))]);
        // WAL should have at least 1 committed frame (the data page write)
        assert!(
            engine.wal_frame_count() > 0,
            "WAL should have committed frames"
        );
    }

    #[test]
    fn test_page_overflow_allocates_new_page() {
        let dir = tempdir().expect("tempdir");
        let mut engine = test_engine(dir.path());
        // Create many nodes with large properties to fill a page
        for i in 0..200u64 {
            let big_str = "x".repeat(100);
            engine.create_node(
                vec![1, 2, 3],
                vec![
                    (1, PropertyValue::String(big_str)),
                    (2, PropertyValue::Int64(i as i64)),
                ],
            );
        }
        // Should have allocated more than one data page
        assert!(
            engine.node_data_page_count() > 1,
            "should use multiple data pages"
        );
    }

    // ======================================================================
    // TASK-030/031: R-PERSIST-005 close/reopen preserves nodes
    // ======================================================================

    #[test]
    fn test_close_reopen_preserves_nodes() {
        let dir = tempdir().expect("tempdir");
        let db_path = dir.path().join("persist_nodes.cyl");
        let config = DatabaseConfig {
            path: db_path.clone(),
            wal_sync_mode: SyncMode::Normal,
            ..Default::default()
        };

        // Phase 1: Create nodes, then close (drop triggers checkpoint)
        {
            let mut engine = StorageEngine::open(config.clone()).expect("open");
            engine.create_node(vec![1, 2], vec![(1, PropertyValue::String("Alice".into()))]);
            engine.create_node(vec![3], vec![(2, PropertyValue::Int64(42))]);
            engine.create_node(
                vec![1],
                vec![
                    (1, PropertyValue::String("Charlie".into())),
                    (3, PropertyValue::Bool(true)),
                ],
            );
            assert_eq!(engine.node_count(), 3);
            // Drop: checkpoint flushes WAL -> main file
        }

        // Phase 2: Reopen and verify all nodes are present
        {
            let engine = StorageEngine::open(config).expect("reopen");
            assert_eq!(
                engine.node_count(),
                3,
                "all nodes should be loaded from disk"
            );

            // Verify node 1 (NodeId(1))
            let n1 = engine.get_node(NodeId(1)).expect("node 1 should exist");
            assert_eq!(n1.labels, vec![1, 2]);
            assert_eq!(n1.properties.len(), 1);
            assert_eq!(n1.properties[0], (1, PropertyValue::String("Alice".into())));

            // Verify node 2 (NodeId(2))
            let n2 = engine.get_node(NodeId(2)).expect("node 2 should exist");
            assert_eq!(n2.labels, vec![3]);
            assert_eq!(n2.properties[0], (2, PropertyValue::Int64(42)));

            // Verify node 3 (NodeId(3))
            let n3 = engine.get_node(NodeId(3)).expect("node 3 should exist");
            assert_eq!(n3.labels, vec![1]);
            assert_eq!(n3.properties.len(), 2);
        }
    }

    // ======================================================================
    // TASK-032/033: R-PERSIST-005 close/reopen preserves edges
    // ======================================================================

    #[test]
    fn test_close_reopen_preserves_edges() {
        let dir = tempdir().expect("tempdir");
        let db_path = dir.path().join("persist_edges.cyl");
        let config = DatabaseConfig {
            path: db_path.clone(),
            wal_sync_mode: SyncMode::Normal,
            ..Default::default()
        };

        // Phase 1: Create nodes and edges
        {
            let mut engine = StorageEngine::open(config.clone()).expect("open");
            let n1 = engine.create_node(vec![1], vec![]);
            let n2 = engine.create_node(vec![2], vec![]);
            let n3 = engine.create_node(vec![3], vec![]);
            engine
                .create_edge(n1, n2, 10, vec![(1, PropertyValue::String("since".into()))])
                .expect("edge1");
            engine.create_edge(n2, n3, 20, vec![]).expect("edge2");
            assert_eq!(engine.node_count(), 3);
            assert_eq!(engine.edge_count(), 2);
        }

        // Phase 2: Reopen and verify
        {
            let engine = StorageEngine::open(config).expect("reopen");
            assert_eq!(engine.node_count(), 3, "nodes should persist");
            assert_eq!(engine.edge_count(), 2, "edges should persist");

            // Verify edge 1
            let e1 = engine.get_edge(EdgeId(1)).expect("edge 1 should exist");
            assert_eq!(e1.start_node, NodeId(1));
            assert_eq!(e1.end_node, NodeId(2));
            assert_eq!(e1.rel_type_id, 10);
            assert_eq!(e1.properties.len(), 1);
            assert_eq!(e1.properties[0], (1, PropertyValue::String("since".into())));

            // Verify edge 2
            let e2 = engine.get_edge(EdgeId(2)).expect("edge 2 should exist");
            assert_eq!(e2.start_node, NodeId(2));
            assert_eq!(e2.end_node, NodeId(3));
            assert_eq!(e2.rel_type_id, 20);
            assert!(e2.properties.is_empty());
        }
    }

    // ======================================================================
    // TASK-034/035: Large dataset close/reopen
    // ======================================================================

    #[test]
    fn test_close_reopen_large_dataset() {
        let dir = tempdir().expect("tempdir");
        let db_path = dir.path().join("persist_large.cyl");
        let config = DatabaseConfig {
            path: db_path.clone(),
            wal_sync_mode: SyncMode::Normal,
            ..Default::default()
        };

        let node_count = 1000;
        let edge_count = 500;

        // Phase 1: Create large dataset
        {
            let mut engine = StorageEngine::open(config.clone()).expect("open");
            for i in 0..node_count {
                engine.create_node(
                    vec![(i % 5) as u32],
                    vec![(1, PropertyValue::Int64(i as i64))],
                );
            }
            // Create edges between consecutive nodes
            for i in 0..edge_count {
                let src = NodeId((i + 1) as u64);
                let dst = NodeId((i + 2) as u64);
                engine
                    .create_edge(src, dst, 1, vec![(1, PropertyValue::Int64(i as i64))])
                    .expect("edge");
            }
            assert_eq!(engine.node_count(), node_count);
            assert_eq!(engine.edge_count(), edge_count);
        }

        // Phase 2: Reopen and verify all data
        {
            let engine = StorageEngine::open(config).expect("reopen");
            assert_eq!(
                engine.node_count(),
                node_count,
                "all {} nodes should be loaded",
                node_count
            );
            assert_eq!(
                engine.edge_count(),
                edge_count,
                "all {} edges should be loaded",
                edge_count
            );

            // Spot-check some nodes
            let first = engine.get_node(NodeId(1)).expect("first node");
            assert_eq!(first.properties[0], (1, PropertyValue::Int64(0)));
            let last = engine
                .get_node(NodeId(node_count as u64))
                .expect("last node");
            assert_eq!(
                last.properties[0],
                (1, PropertyValue::Int64((node_count - 1) as i64))
            );

            // Spot-check some edges
            let first_edge = engine.get_edge(EdgeId(1)).expect("first edge");
            assert_eq!(first_edge.start_node, NodeId(1));
            assert_eq!(first_edge.end_node, NodeId(2));
        }
    }

    // ======================================================================
    // TASK-036: Close/reopen empty database
    // ======================================================================

    #[test]
    fn test_close_reopen_empty_database() {
        let dir = tempdir().expect("tempdir");
        let db_path = dir.path().join("persist_empty.cyl");
        let config = DatabaseConfig {
            path: db_path.clone(),
            wal_sync_mode: SyncMode::Normal,
            ..Default::default()
        };

        // Phase 1: Open empty, close
        {
            let _engine = StorageEngine::open(config.clone()).expect("open");
        }

        // Phase 2: Reopen empty database - should not crash
        {
            let engine = StorageEngine::open(config).expect("reopen");
            assert_eq!(engine.node_count(), 0);
            assert_eq!(engine.edge_count(), 0);
        }
    }

    // ======================================================================
    // TASK-036: New node IDs continue from where they left off
    // ======================================================================

    #[test]
    fn test_close_reopen_id_continuity() {
        let dir = tempdir().expect("tempdir");
        let db_path = dir.path().join("persist_ids.cyl");
        let config = DatabaseConfig {
            path: db_path.clone(),
            wal_sync_mode: SyncMode::Normal,
            ..Default::default()
        };

        // Phase 1: Create 3 nodes
        {
            let mut engine = StorageEngine::open(config.clone()).expect("open");
            engine.create_node(vec![], vec![]);
            engine.create_node(vec![], vec![]);
            engine.create_node(vec![], vec![]);
        }

        // Phase 2: Reopen, create another node - should get NodeId(4)
        {
            let mut engine = StorageEngine::open(config).expect("reopen");
            assert_eq!(engine.node_count(), 3);
            let new_id = engine.create_node(vec![99], vec![]);
            assert_eq!(new_id, NodeId(4), "new node should get next sequential ID");
            assert_eq!(engine.node_count(), 4);
        }
    }

    // ======================================================================
    // TASK-037: R-PERSIST-010 close/reopen preserves catalog labels
    // ======================================================================

    #[test]
    fn test_close_reopen_preserves_catalog_labels() {
        let dir = tempdir().expect("tempdir");
        let db_path = dir.path().join("persist_catalog.cyl");
        let config = DatabaseConfig {
            path: db_path.clone(),
            wal_sync_mode: SyncMode::Normal,
            ..Default::default()
        };

        // Phase 1: Register labels and create a node using them
        let person_id;
        let company_id;
        {
            let mut engine = StorageEngine::open(config.clone()).expect("open");
            person_id = engine.get_or_create_label("Person");
            company_id = engine.get_or_create_label("Company");
            // Create a node so there is something to persist
            engine.create_node(vec![person_id], vec![]);
        }

        // Phase 2: Reopen and verify catalog labels survived
        {
            let engine = StorageEngine::open(config).expect("reopen");
            assert_eq!(
                engine.label_id("Person"),
                Some(person_id),
                "Person label should persist across close/reopen"
            );
            assert_eq!(
                engine.label_id("Company"),
                Some(company_id),
                "Company label should persist across close/reopen"
            );
            assert_eq!(
                engine.label_name(person_id),
                Some("Person"),
                "Reverse lookup should work after reopen"
            );
        }
    }

    // ======================================================================
    // TASK-039: R-PERSIST-010 close/reopen preserves all catalog entries
    // ======================================================================

    #[test]
    fn test_close_reopen_preserves_all_catalog_entries() {
        let dir = tempdir().expect("tempdir");
        let db_path = dir.path().join("persist_catalog_all.cyl");
        let config = DatabaseConfig {
            path: db_path.clone(),
            wal_sync_mode: SyncMode::Normal,
            ..Default::default()
        };

        // Phase 1: Register labels, property keys, and relationship types
        let label_person;
        let label_company;
        let prop_name;
        let prop_age;
        let rel_knows;
        let rel_works_at;
        {
            let mut engine = StorageEngine::open(config.clone()).expect("open");
            label_person = engine.get_or_create_label("Person");
            label_company = engine.get_or_create_label("Company");
            prop_name = engine.get_or_create_prop_key("name");
            prop_age = engine.get_or_create_prop_key("age");
            rel_knows = engine.get_or_create_rel_type("KNOWS");
            rel_works_at = engine.get_or_create_rel_type("WORKS_AT");
            // Create some data so engine has something to persist
            let n1 = engine.create_node(
                vec![label_person],
                vec![(prop_name, PropertyValue::String("Alice".into()))],
            );
            let n2 = engine.create_node(
                vec![label_company],
                vec![(prop_name, PropertyValue::String("Acme".into()))],
            );
            engine
                .create_edge(n1, n2, rel_works_at, vec![])
                .expect("edge");
        }

        // Phase 2: Reopen and verify ALL catalog entries survived
        {
            let engine = StorageEngine::open(config).expect("reopen");

            // Labels
            assert_eq!(engine.label_id("Person"), Some(label_person));
            assert_eq!(engine.label_id("Company"), Some(label_company));
            assert_eq!(engine.label_name(label_person), Some("Person"));
            assert_eq!(engine.label_name(label_company), Some("Company"));

            // Property keys
            assert_eq!(engine.prop_key_id("name"), Some(prop_name));
            assert_eq!(engine.prop_key_id("age"), Some(prop_age));
            assert_eq!(engine.prop_key_name(prop_name), Some("name"));
            assert_eq!(engine.prop_key_name(prop_age), Some("age"));

            // Relationship types
            assert_eq!(engine.rel_type_id("KNOWS"), Some(rel_knows));
            assert_eq!(engine.rel_type_id("WORKS_AT"), Some(rel_works_at));
            assert_eq!(engine.rel_type_name(rel_knows), Some("KNOWS"));
            assert_eq!(engine.rel_type_name(rel_works_at), Some("WORKS_AT"));
        }
    }

    // ======================================================================
    // TASK-039b: Catalog ID sequence continues after reopen
    // ======================================================================

    #[test]
    fn test_close_reopen_catalog_id_continuity() {
        let dir = tempdir().expect("tempdir");
        let db_path = dir.path().join("persist_catalog_ids.cyl");
        let config = DatabaseConfig {
            path: db_path.clone(),
            wal_sync_mode: SyncMode::Normal,
            ..Default::default()
        };

        // Phase 1: Register 2 labels (ids 0, 1)
        {
            let mut engine = StorageEngine::open(config.clone()).expect("open");
            engine.get_or_create_label("Person"); // id=0
            engine.get_or_create_label("Company"); // id=1
        }

        // Phase 2: Reopen and register a new label - should get id=2
        {
            let mut engine = StorageEngine::open(config).expect("reopen");
            let new_id = engine.get_or_create_label("City");
            assert_eq!(
                new_id, 2,
                "new label after reopen should continue ID sequence"
            );
            // Existing labels still accessible
            assert_eq!(engine.label_id("Person"), Some(0));
            assert_eq!(engine.label_id("Company"), Some(1));
            assert_eq!(engine.label_id("City"), Some(2));
        }
    }

    // ======================================================================
    // TASK-039c: Empty catalog persistence (no labels registered)
    // ======================================================================

    #[test]
    fn test_close_reopen_empty_catalog() {
        let dir = tempdir().expect("tempdir");
        let db_path = dir.path().join("persist_catalog_empty.cyl");
        let config = DatabaseConfig {
            path: db_path.clone(),
            wal_sync_mode: SyncMode::Normal,
            ..Default::default()
        };

        // Phase 1: Open and close without registering any catalog entries
        {
            let _engine = StorageEngine::open(config.clone()).expect("open");
        }

        // Phase 2: Reopen - should not crash
        {
            let engine = StorageEngine::open(config).expect("reopen");
            assert_eq!(engine.label_id("anything"), None);
            assert_eq!(engine.node_count(), 0);
        }
    }

    // ======================================================================
    // TASK-042/043: R-PERSIST-050 SubgraphStore persistence
    // ======================================================================

    #[cfg(feature = "subgraph")]
    #[test]
    fn test_close_reopen_preserves_subgraphs() {
        use cypherlite_core::SubgraphId;

        let dir = tempdir().expect("tempdir");
        let db_path = dir.path().join("persist_subgraphs.cyl");
        let config = DatabaseConfig {
            path: db_path.clone(),
            wal_sync_mode: SyncMode::Normal,
            ..Default::default()
        };

        // Phase 1: Create subgraphs, then close
        {
            let mut engine = StorageEngine::open(config.clone()).expect("open");
            // Create subgraph with properties and temporal anchor
            let sg1 = engine.create_subgraph(
                vec![(1, PropertyValue::String("graph-A".into()))],
                Some(1_700_000_000_000),
            );
            // Create empty subgraph
            let sg2 = engine.create_subgraph(vec![], None);
            // Create subgraph with multiple properties
            let sg3 = engine.create_subgraph(
                vec![
                    (2, PropertyValue::Int64(42)),
                    (3, PropertyValue::Bool(true)),
                ],
                Some(1_700_000_001_000),
            );
            assert_eq!(sg1, SubgraphId(1));
            assert_eq!(sg2, SubgraphId(2));
            assert_eq!(sg3, SubgraphId(3));
        }

        // Phase 2: Reopen and verify all subgraphs are present
        {
            let engine = StorageEngine::open(config).expect("reopen");
            // Verify subgraph 1
            let s1 = engine.get_subgraph(SubgraphId(1)).expect("subgraph 1");
            assert_eq!(s1.subgraph_id, SubgraphId(1));
            assert_eq!(
                s1.properties,
                vec![(1, PropertyValue::String("graph-A".into()))]
            );
            assert_eq!(s1.temporal_anchor, Some(1_700_000_000_000));

            // Verify subgraph 2
            let s2 = engine.get_subgraph(SubgraphId(2)).expect("subgraph 2");
            assert!(s2.properties.is_empty());
            assert_eq!(s2.temporal_anchor, None);

            // Verify subgraph 3
            let s3 = engine.get_subgraph(SubgraphId(3)).expect("subgraph 3");
            assert_eq!(s3.properties.len(), 2);
            assert_eq!(s3.temporal_anchor, Some(1_700_000_001_000));

            // Verify next_subgraph_id is preserved (should be 4)
            let sg4 = engine.get_subgraph(SubgraphId(4));
            assert!(sg4.is_none());
        }
    }

    #[cfg(feature = "subgraph")]
    #[test]
    fn test_close_reopen_preserves_memberships() {
        use cypherlite_core::SubgraphId;

        let dir = tempdir().expect("tempdir");
        let db_path = dir.path().join("persist_memberships.cyl");
        let config = DatabaseConfig {
            path: db_path.clone(),
            wal_sync_mode: SyncMode::Normal,
            ..Default::default()
        };

        // Phase 1: Create subgraph with members
        {
            let mut engine = StorageEngine::open(config.clone()).expect("open");
            let sg = engine.create_subgraph(vec![], None);
            let n1 = engine.create_node(vec![1], vec![]);
            let n2 = engine.create_node(vec![2], vec![]);
            engine.add_member(sg, n1).expect("add n1");
            engine.add_member(sg, n2).expect("add n2");
            assert_eq!(engine.list_members(sg).len(), 2);
        }

        // Phase 2: Reopen and verify memberships
        {
            let engine = StorageEngine::open(config).expect("reopen");
            let members = engine.list_members(SubgraphId(1));
            assert_eq!(members.len(), 2);
            assert!(members.contains(&NodeId(1)));
            assert!(members.contains(&NodeId(2)));
        }
    }

    // ======================================================================
    // TASK-044/045: R-PERSIST-051 HyperEdgeStore persistence
    // ======================================================================

    #[cfg(feature = "hypergraph")]
    #[test]
    fn test_close_reopen_preserves_hyperedges() {
        use cypherlite_core::{GraphEntity, HyperEdgeId};

        let dir = tempdir().expect("tempdir");
        let db_path = dir.path().join("persist_hyperedges.cyl");
        let config = DatabaseConfig {
            path: db_path.clone(),
            wal_sync_mode: SyncMode::Normal,
            ..Default::default()
        };

        // Phase 1: Create hyperedges, then close
        {
            let mut engine = StorageEngine::open(config.clone()).expect("open");
            let n1 = engine.create_node(vec![1], vec![]);
            let n2 = engine.create_node(vec![2], vec![]);
            let n3 = engine.create_node(vec![3], vec![]);

            // Create hyperedge with properties
            let he1 = engine.create_hyperedge(
                10,
                vec![GraphEntity::Node(n1)],
                vec![GraphEntity::Node(n2), GraphEntity::Node(n3)],
                vec![(1, PropertyValue::String("rel-A".into()))],
            );
            // Create empty hyperedge
            let he2 = engine.create_hyperedge(20, vec![], vec![], vec![]);
            assert_eq!(he1, HyperEdgeId(1));
            assert_eq!(he2, HyperEdgeId(2));
        }

        // Phase 2: Reopen and verify all hyperedges are present
        {
            let engine = StorageEngine::open(config).expect("reopen");
            // Verify hyperedge 1
            let h1 = engine.get_hyperedge(HyperEdgeId(1)).expect("hyperedge 1");
            assert_eq!(h1.id, HyperEdgeId(1));
            assert_eq!(h1.rel_type_id, 10);
            assert_eq!(h1.sources.len(), 1);
            assert_eq!(h1.targets.len(), 2);
            assert_eq!(
                h1.properties,
                vec![(1, PropertyValue::String("rel-A".into()))]
            );

            // Verify hyperedge 2
            let h2 = engine.get_hyperedge(HyperEdgeId(2)).expect("hyperedge 2");
            assert_eq!(h2.id, HyperEdgeId(2));
            assert_eq!(h2.rel_type_id, 20);
            assert!(h2.sources.is_empty());
            assert!(h2.targets.is_empty());
            assert!(h2.properties.is_empty());

            // Verify nodes also persisted
            assert_eq!(engine.node_count(), 3);
        }
    }

    // ======================================================================
    // TASK-046/047: R-PERSIST-052 VersionStore persistence
    // ======================================================================

    #[test]
    fn test_close_reopen_preserves_version_store() {
        let dir = tempdir().expect("tempdir");
        let db_path = dir.path().join("persist_versions.cyl");
        let config = DatabaseConfig {
            path: db_path.clone(),
            wal_sync_mode: SyncMode::Normal,
            version_storage_enabled: true,
            ..Default::default()
        };

        // Phase 1: Create nodes, update them to create version snapshots
        {
            let mut engine = StorageEngine::open(config.clone()).expect("open");
            let n1 =
                engine.create_node(vec![1], vec![(1, PropertyValue::String("Alice-v1".into()))]);
            // Update node to create a version snapshot
            engine
                .update_node(n1, vec![(1, PropertyValue::String("Alice-v2".into()))])
                .expect("update n1");
            // Verify version was created before close
            assert_eq!(engine.version_count(n1.0), 1);
        }

        // Phase 2: Reopen and verify version history is present
        {
            let engine = StorageEngine::open(config).expect("reopen");
            // Node should be present with latest state
            let n = engine.get_node(NodeId(1)).expect("node 1");
            assert_eq!(
                n.properties[0],
                (1, PropertyValue::String("Alice-v2".into()))
            );
            // Version history should be preserved
            assert_eq!(engine.version_count(1), 1);
            let chain = engine.version_chain(1);
            assert_eq!(chain.len(), 1);
        }
    }
}
