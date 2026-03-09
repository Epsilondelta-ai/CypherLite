// TransactionManager, ReadTransaction, WriteTransaction, MVCC snapshot isolation

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use parking_lot::{Mutex, MutexGuard};

use cypherlite_core::{CypherLiteError, Result, TransactionView};

/// Manages transaction lifecycle and concurrency control.
///
/// REQ-TX-009: Exclusive lock for write transactions (single writer).
/// REQ-TX-006: Snapshot Isolation for read transactions.
pub struct TransactionManager {
    /// Current committed WAL frame number.
    current_frame: Arc<AtomicU64>,
    /// Write lock: only one write transaction at a time.
    write_lock: Arc<Mutex<()>>,
    /// Next transaction ID.
    next_tx_id: AtomicU64,
}

impl TransactionManager {
    /// Create a new transaction manager.
    pub fn new() -> Self {
        Self {
            current_frame: Arc::new(AtomicU64::new(0)),
            write_lock: Arc::new(Mutex::new(())),
            next_tx_id: AtomicU64::new(1),
        }
    }

    /// Begin a read transaction.
    /// REQ-TX-001: Snapshot current WAL frame index as read consistency point.
    /// REQ-TX-006: Read transactions see consistent snapshot, not blocked by writes.
    pub fn begin_read(&self) -> ReadTransaction {
        let snapshot = self.current_frame.load(Ordering::Acquire);
        let tx_id = self.next_tx_id.fetch_add(1, Ordering::Relaxed);
        ReadTransaction {
            tx_id,
            snapshot_frame: snapshot,
        }
    }

    /// Begin a write transaction.
    /// REQ-TX-009: Exclusive lock for write transactions.
    /// REQ-TX-010: If write lock unavailable, return TransactionConflict.
    // @MX:WARN: [AUTO] Uses unsafe transmute to extend MutexGuard lifetime to 'static.
    // @MX:REASON: Arc<Mutex> kept alive by _write_lock_arc field; Rust field-drop order
    //   guarantees _guard drops before _write_lock_arc (declaration order). Invariant:
    //   WriteTransaction struct fields must NOT be reordered without updating this safety proof.
    // @MX:SPEC: SPEC-DB-001 REQ-TX-009
    pub fn begin_write(&self) -> Result<WriteTransaction> {
        let guard = self.write_lock.try_lock();

        if guard.is_none() {
            return Err(CypherLiteError::TransactionConflict);
        }

        let snapshot = self.current_frame.load(Ordering::Acquire);
        let tx_id = self.next_tx_id.fetch_add(1, Ordering::Relaxed);

        // Store the guard in the WriteTransaction so the lock is held
        // for the lifetime of the transaction.
        //
        // SAFETY: The MutexGuard borrows from the Arc<Mutex>, which is kept alive
        // by _write_lock_arc. We transmute the lifetime to 'static because
        // we guarantee the Arc outlives the guard via struct field ordering
        // (Rust drops fields in declaration order).
        let guard = guard.expect("checked above");
        let guard: MutexGuard<'static, ()> = unsafe { std::mem::transmute(guard) };

        Ok(WriteTransaction {
            tx_id,
            snapshot_frame: snapshot,
            committed: false,
            _guard: Some(guard),
            _write_lock_arc: self.write_lock.clone(),
            current_frame: self.current_frame.clone(),
        })
    }

    /// Update the current frame number (called after WAL commit).
    pub fn update_current_frame(&self, frame: u64) {
        self.current_frame.store(frame, Ordering::Release);
    }

    /// Get the current committed frame number.
    pub fn current_frame(&self) -> u64 {
        self.current_frame.load(Ordering::Acquire)
    }
}

impl Default for TransactionManager {
    fn default() -> Self {
        Self::new()
    }
}

/// A read-only transaction with a snapshot point.
///
/// REQ-TX-006: Provides Snapshot Isolation.
/// REQ-TX-011: Uncommitted changes not visible.
pub struct ReadTransaction {
    tx_id: u64,
    snapshot_frame: u64,
}

impl ReadTransaction {
    /// Returns the transaction ID.
    pub fn tx_id(&self) -> u64 {
        self.tx_id
    }
}

impl TransactionView for ReadTransaction {
    fn snapshot_frame(&self) -> u64 {
        self.snapshot_frame
    }
}

/// A read-write transaction with exclusive write access.
///
/// REQ-TX-004: All changes atomically (all-or-nothing WAL frames).
/// REQ-TX-009: Single writer via exclusive lock.
///
/// Field ordering matters: `_guard` must be declared before `_write_lock_arc`
/// so the guard is dropped first (Rust drops fields in declaration order).
pub struct WriteTransaction {
    tx_id: u64,
    snapshot_frame: u64,
    committed: bool,
    // Guard must drop before the Arc to avoid use-after-free.
    _guard: Option<MutexGuard<'static, ()>>,
    _write_lock_arc: Arc<Mutex<()>>,
    current_frame: Arc<AtomicU64>,
}

impl WriteTransaction {
    /// Returns the transaction ID.
    pub fn tx_id(&self) -> u64 {
        self.tx_id
    }

    /// Mark this transaction as committed and update the global frame.
    /// REQ-TX-002: Commit updates WAL index atomically.
    pub fn commit(&mut self, new_frame: u64) {
        self.committed = true;
        self.current_frame.store(new_frame, Ordering::Release);
    }

    /// Check if this transaction has been committed.
    pub fn is_committed(&self) -> bool {
        self.committed
    }

    /// Rollback: release the write lock without committing.
    /// REQ-TX-003: Discard uncommitted WAL frames.
    pub fn rollback(mut self) {
        // Drop the guard to release the write lock.
        self._guard.take();
    }
}

impl TransactionView for WriteTransaction {
    fn snapshot_frame(&self) -> u64 {
        self.snapshot_frame
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // REQ-TX-001: Begin read captures snapshot
    #[test]
    fn test_begin_read_captures_snapshot() {
        let tm = TransactionManager::new();
        tm.update_current_frame(42);
        let tx = tm.begin_read();
        assert_eq!(tx.snapshot_frame(), 42);
    }

    // REQ-TX-006: Multiple readers don't block
    #[test]
    fn test_multiple_readers() {
        let tm = TransactionManager::new();
        let r1 = tm.begin_read();
        let r2 = tm.begin_read();
        assert_ne!(r1.tx_id(), r2.tx_id());
    }

    // REQ-TX-009: Single writer (exclusive lock)
    // REQ-TX-010: Second write returns TransactionConflict
    #[test]
    fn test_begin_write_exclusive() {
        let tm = TransactionManager::new();
        let _w1 = tm.begin_write().expect("first write");
        let result = tm.begin_write();
        assert!(matches!(result, Err(CypherLiteError::TransactionConflict)));
    }

    // REQ-TX-009: After first write drops, second write succeeds
    #[test]
    fn test_write_lock_released_on_drop() {
        let tm = TransactionManager::new();
        {
            let _w1 = tm.begin_write().expect("first write");
        } // _w1 drops here, releasing lock
        let _w2 = tm.begin_write().expect("second write should succeed");
    }

    // REQ-TX-002: Commit updates frame
    #[test]
    fn test_commit_updates_frame() {
        let tm = TransactionManager::new();
        let mut w = tm.begin_write().expect("write");
        assert!(!w.is_committed());
        w.commit(10);
        assert!(w.is_committed());
        assert_eq!(tm.current_frame(), 10);
    }

    // REQ-TX-003: Rollback releases lock
    #[test]
    fn test_rollback_releases_lock() {
        let tm = TransactionManager::new();
        let w = tm.begin_write().expect("write");
        w.rollback();
        let _w2 = tm.begin_write().expect("should succeed after rollback");
    }

    // REQ-TX-006: Read sees snapshot, not later writes
    #[test]
    fn test_snapshot_isolation() {
        let tm = TransactionManager::new();
        tm.update_current_frame(5);
        let r = tm.begin_read();
        assert_eq!(r.snapshot_frame(), 5);

        tm.update_current_frame(10);
        assert_eq!(r.snapshot_frame(), 5); // unchanged
    }

    #[test]
    fn test_transaction_ids_are_unique() {
        let tm = TransactionManager::new();
        let t1 = tm.begin_read();
        let t2 = tm.begin_read();
        let t3 = tm.begin_write().expect("w");
        assert_ne!(t1.tx_id(), t2.tx_id());
        assert_ne!(t2.tx_id(), t3.tx_id());
    }

    #[test]
    fn test_initial_frame_is_zero() {
        let tm = TransactionManager::new();
        assert_eq!(tm.current_frame(), 0);
    }

    // REQ-TX-011: Uncommitted changes not visible
    #[test]
    fn test_uncommitted_not_visible() {
        let tm = TransactionManager::new();
        let r = tm.begin_read();
        assert_eq!(r.snapshot_frame(), 0);
        let _w = tm.begin_write().expect("w");
        let r2 = tm.begin_read();
        assert_eq!(r2.snapshot_frame(), 0);
    }

    // REQ-TX-006: Reader not blocked by writer
    #[test]
    fn test_reader_not_blocked_by_writer() {
        let tm = TransactionManager::new();
        let _w = tm.begin_write().expect("write");
        // Reading should still work even with active write transaction
        let r = tm.begin_read();
        assert_eq!(r.snapshot_frame(), 0);
    }
}
