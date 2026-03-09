// Multi-thread concurrency safety tests

use cypherlite_core::{CypherLiteError, TransactionView};
use cypherlite_storage::transaction::TransactionManager;
use std::sync::Arc;

// REQ-TX-009: Single writer via exclusive lock (thread safety)
#[test]
fn test_concurrent_write_exclusion() {
    let tm = Arc::new(TransactionManager::new());

    let tm1 = tm.clone();
    let tm2 = tm.clone();

    // First writer acquires lock
    let w1 = tm1.begin_write().expect("first write");

    // Second writer should get TransactionConflict
    let result = tm2.begin_write();
    assert!(matches!(result, Err(CypherLiteError::TransactionConflict)));

    drop(w1);

    // Now second writer should succeed
    let _w2 = tm2.begin_write().expect("second write after drop");
}

// REQ-TX-006: Multiple concurrent readers
#[test]
fn test_concurrent_readers() {
    let tm = Arc::new(TransactionManager::new());
    tm.update_current_frame(42);

    let mut handles = vec![];
    for _ in 0..10 {
        let tm_clone = tm.clone();
        let handle = std::thread::spawn(move || {
            let r = tm_clone.begin_read();
            assert_eq!(r.snapshot_frame(), 42);
        });
        handles.push(handle);
    }

    for h in handles {
        h.join().expect("thread");
    }
}

// REQ-TX-006: Readers not blocked by writer
#[test]
fn test_readers_not_blocked_by_writer() {
    let tm = Arc::new(TransactionManager::new());
    tm.update_current_frame(10);

    let _w = tm.begin_write().expect("write");

    // Readers should still work
    let tm_clone = tm.clone();
    let handle = std::thread::spawn(move || {
        let r = tm_clone.begin_read();
        assert_eq!(r.snapshot_frame(), 10);
    });
    handle.join().expect("reader thread");
}

// REQ-TX-006: Snapshot isolation across threads
#[test]
fn test_snapshot_isolation_across_threads() {
    let tm = Arc::new(TransactionManager::new());
    tm.update_current_frame(5);

    // Reader takes snapshot at frame 5
    let r = tm.begin_read();
    assert_eq!(r.snapshot_frame(), 5);

    // Writer commits and advances frame
    let tm_clone = tm.clone();
    let handle = std::thread::spawn(move || {
        let mut w = tm_clone.begin_write().expect("write");
        tm_clone.update_current_frame(10);
        w.commit(10);
    });
    handle.join().expect("writer");

    // Original reader still sees frame 5
    assert_eq!(r.snapshot_frame(), 5);

    // New reader sees frame 10
    let r2 = tm.begin_read();
    assert_eq!(r2.snapshot_frame(), 10);
}

// TransactionManager is Send + Sync
#[test]
fn test_transaction_manager_is_send_sync() {
    fn assert_send<T: Send>() {}
    fn assert_sync<T: Sync>() {}
    assert_send::<TransactionManager>();
    assert_sync::<TransactionManager>();
}

// REQ-TX-003: Rollback releases lock for next writer
#[test]
fn test_rollback_releases_lock_across_threads() {
    let tm = Arc::new(TransactionManager::new());

    let tm1 = tm.clone();
    let handle = std::thread::spawn(move || {
        let w = tm1.begin_write().expect("write");
        w.rollback(); // Release without committing
    });
    handle.join().expect("thread");

    // Should be able to acquire write lock now
    let _w = tm.begin_write().expect("write after rollback");
}
