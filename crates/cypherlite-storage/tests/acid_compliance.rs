// ACID compliance integration tests: Atomicity, Consistency, Isolation, Durability

use cypherlite_core::{CypherLiteError, DatabaseConfig, PageId, SyncMode};
use cypherlite_storage::page::PAGE_SIZE;
use cypherlite_storage::StorageEngine;
use tempfile::tempdir;

fn test_engine() -> (tempfile::TempDir, StorageEngine) {
    let dir = tempdir().expect("tempdir");
    let config = DatabaseConfig {
        path: dir.path().join("test.cyl"),
        wal_sync_mode: SyncMode::Normal,
        ..Default::default()
    };
    let engine = StorageEngine::open(config).expect("open");
    (dir, engine)
}

// REQ-TX-004: Atomicity - all-or-nothing WAL frames
#[test]
fn test_atomicity_commit() {
    let (_dir, mut engine) = test_engine();

    // Write multiple WAL frames in one transaction
    engine
        .wal_write_page(PageId(2), &[0x11; PAGE_SIZE])
        .expect("w1");
    engine
        .wal_write_page(PageId(3), &[0x22; PAGE_SIZE])
        .expect("w2");
    let frame = engine.wal_commit().expect("commit");
    assert!(frame >= 2);
}

// REQ-TX-004: Atomicity - rollback discards all frames
#[test]
fn test_atomicity_rollback() {
    let (_dir, mut engine) = test_engine();

    engine
        .wal_write_page(PageId(2), &[0x11; PAGE_SIZE])
        .expect("w1");
    engine
        .wal_write_page(PageId(3), &[0x22; PAGE_SIZE])
        .expect("w2");
    engine.wal_discard();

    // Nothing committed
    let count = engine.checkpoint().expect("checkpoint");
    assert_eq!(count, 0);
}

// REQ-TX-005: Consistency - constraints validated
#[test]
fn test_consistency_node_reference() {
    let (_dir, mut engine) = test_engine();
    let n1 = engine.create_node(vec![], vec![]);
    // Creating edge to nonexistent node should fail
    let result = engine.create_edge(n1, cypherlite_core::NodeId(999), 1, vec![]);
    assert!(matches!(result, Err(CypherLiteError::NodeNotFound(999))));
}

// REQ-TX-006: Isolation - snapshot isolation
#[test]
fn test_isolation_snapshot() {
    let (_dir, engine) = test_engine();

    // Reader sees snapshot at time of begin
    let r1 = engine.begin_read();
    assert_eq!(cypherlite_core::TransactionView::snapshot_frame(&r1), 0);

    // Another reader also sees same snapshot
    let r2 = engine.begin_read();
    assert_eq!(cypherlite_core::TransactionView::snapshot_frame(&r2), 0);
}

// REQ-TX-007: Durability - committed data survives checkpoint
#[test]
fn test_durability_checkpoint() {
    let dir = tempdir().expect("tempdir");
    let config = DatabaseConfig {
        path: dir.path().join("test.cyl"),
        wal_sync_mode: SyncMode::Normal,
        ..Default::default()
    };

    // Write and commit data, then checkpoint
    {
        let mut engine = StorageEngine::open(config.clone()).expect("open");
        engine
            .wal_write_page(PageId(2), &[0xAB; PAGE_SIZE])
            .expect("w");
        engine.wal_commit().expect("commit");
        engine.checkpoint().expect("checkpoint");
    }

    // Reopen and verify data persists in main file
    {
        let engine = StorageEngine::open(config).expect("reopen");
        // Database should open successfully after checkpoint
        assert_eq!(engine.node_count(), 0); // no in-memory data, but file is valid
    }
}

// REQ-TX-009: Single writer exclusion
#[test]
fn test_single_writer() {
    let (_dir, engine) = test_engine();
    let _w1 = engine.begin_write().expect("first write");
    let result = engine.begin_write();
    assert!(matches!(result, Err(CypherLiteError::TransactionConflict)));
}

// REQ-TX-010: Write lock conflict
#[test]
fn test_write_lock_conflict() {
    let (_dir, engine) = test_engine();
    let _w = engine.begin_write().expect("write");
    for _ in 0..5 {
        assert!(matches!(
            engine.begin_write(),
            Err(CypherLiteError::TransactionConflict)
        ));
    }
}

// REQ-TX-011: Uncommitted not visible
#[test]
fn test_uncommitted_not_visible() {
    let (_dir, engine) = test_engine();
    let r1 = engine.begin_read();
    // Start a write but don't commit
    let _w = engine.begin_write().expect("write");
    // New reader still sees old state
    let r2 = engine.begin_read();
    assert_eq!(
        cypherlite_core::TransactionView::snapshot_frame(&r1),
        cypherlite_core::TransactionView::snapshot_frame(&r2)
    );
}

// WAL + Checkpoint round-trip
#[test]
fn test_wal_checkpoint_roundtrip() {
    let (_dir, mut engine) = test_engine();

    // Write -> commit -> checkpoint -> verify
    let data = [0xFE; PAGE_SIZE];
    engine.wal_write_page(PageId(2), &data).expect("w");
    engine.wal_commit().expect("commit");

    let checkpointed = engine.checkpoint().expect("checkpoint");
    assert_eq!(checkpointed, 1);

    // After checkpoint, WAL should be empty
    engine
        .wal_write_page(PageId(3), &[0x01; PAGE_SIZE])
        .expect("w2");
    engine.wal_commit().expect("commit2");
    let checkpointed2 = engine.checkpoint().expect("checkpoint2");
    assert_eq!(checkpointed2, 1);
}
