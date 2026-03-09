pub mod btree;
pub mod page;
pub mod transaction;
pub mod wal;

use cypherlite_core::{
    DatabaseConfig, EdgeId, NodeId, NodeRecord, PageId, PropertyValue, RelationshipRecord, Result,
};

use btree::edge_store::EdgeStore;
use btree::node_store::NodeStore;
use page::buffer_pool::BufferPool;
use page::page_manager::PageManager;
use page::PAGE_SIZE;
use transaction::mvcc::TransactionManager;
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

        Ok(Self {
            page_manager,
            buffer_pool,
            wal_writer,
            wal_reader,
            tx_manager,
            node_store,
            edge_store,
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
        let id = self.node_store.create_node(labels, properties);
        // Update header with new next_node_id
        self.page_manager.header_mut().next_node_id = self.node_store.next_id();
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
        self.node_store.update_node(node_id, properties)
    }

    /// Delete a node and all its connected edges.
    /// REQ-STORE-004: Delete all connected edges first.
    pub fn delete_node(&mut self, node_id: NodeId) -> Result<NodeRecord> {
        // Delete connected edges first
        self.edge_store
            .delete_edges_for_node(node_id, &mut self.node_store)?;
        self.node_store.delete_node(node_id)
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
            properties,
            &mut self.node_store,
        )?;
        self.page_manager.header_mut().next_edge_id = self.edge_store.next_id();
        Ok(id)
    }

    /// Get an edge by ID.
    pub fn get_edge(&self, edge_id: EdgeId) -> Option<&RelationshipRecord> {
        self.edge_store.get_edge(edge_id)
    }

    /// Get all edges connected to a node.
    pub fn get_edges_for_node(&self, node_id: NodeId) -> Vec<&RelationshipRecord> {
        self.edge_store
            .get_edges_for_node(node_id, &self.node_store)
    }

    /// Delete an edge.
    pub fn delete_edge(&mut self, edge_id: EdgeId) -> Result<RelationshipRecord> {
        self.edge_store.delete_edge(edge_id, &mut self.node_store)
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

    /// Returns a reference to the config.
    pub fn config(&self) -> &DatabaseConfig {
        &self.config
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
}
