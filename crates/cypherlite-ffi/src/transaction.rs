// CylTx: FFI transaction handle.
//
// A CylTx holds a raw pointer back to its parent CylDb. While a transaction
// is active the `CylDb.in_transaction` flag prevents non-transactional
// access via `cyl_db_execute`.
//
// Note: The underlying Rust Transaction is a thin wrapper -- `commit()` sets
// a flag, `rollback()` is a no-op (Phase 2 -- full WAL rollback pending).
// For FFI we lock the Mutex for each execute call within the transaction and
// use the in_transaction flag for exclusion.

use crate::db::CylDb;
use crate::error::{c_str_to_str, set_error, set_error_code, set_ok, CylError};
use crate::query::CylResult;
use crate::value::CylValue;
use std::collections::HashMap;
use std::sync::atomic::Ordering;

/// Opaque FFI transaction handle.
pub struct CylTx {
    /// Raw pointer to the parent database (not owned).
    db: *mut CylDb,
    /// Whether this transaction has been committed or rolled back.
    finished: bool,
}

// ---------------------------------------------------------------------------
// cyl_tx_begin
// ---------------------------------------------------------------------------

/// Begin a transaction on the database.
///
/// Returns a heap-allocated `CylTx` on success, or `NULL` if the database
/// already has an active transaction or `db` is null.
///
/// While a transaction is active, `cyl_db_execute()` will return
/// `CYL_ERR_TRANSACTION_CONFLICT`. Use `cyl_tx_execute()` instead.
///
/// # Safety
///
/// - `db` must be a valid `CylDb` pointer.
/// - `error_out` must point to a valid `CylError` or be `NULL`.
#[no_mangle]
pub unsafe extern "C" fn cyl_tx_begin(db: *mut CylDb, error_out: *mut CylError) -> *mut CylTx {
    if db.is_null() {
        set_error_code(CylError::CylErrNullPointer, "db is null", error_out);
        return std::ptr::null_mut();
    }

    // SAFETY: caller guarantees db is valid.
    let db_ref = &*db;

    // Atomically set in_transaction. If it was already true, another
    // transaction is active.
    if db_ref
        .in_transaction
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        set_error_code(
            CylError::CylErrTransactionConflict,
            "another transaction is already active",
            error_out,
        );
        return std::ptr::null_mut();
    }

    set_ok(error_out);
    Box::into_raw(Box::new(CylTx {
        db,
        finished: false,
    }))
}

// ---------------------------------------------------------------------------
// cyl_tx_execute
// ---------------------------------------------------------------------------

/// Execute a Cypher query within the transaction.
///
/// # Safety
///
/// - `tx` must be a valid `CylTx` pointer.
/// - `query` must be a valid, null-terminated C string.
/// - `error_out` must point to a valid `CylError` or be `NULL`.
#[no_mangle]
pub unsafe extern "C" fn cyl_tx_execute(
    tx: *mut CylTx,
    query: *const libc::c_char,
    error_out: *mut CylError,
) -> *mut CylResult {
    let Some(tx_ref) = validate_tx(tx, error_out) else {
        return std::ptr::null_mut();
    };
    let Some(query_str) = c_str_to_str(query, "query", error_out) else {
        return std::ptr::null_mut();
    };

    // SAFETY: tx_ref.db was validated during cyl_tx_begin.
    let db_ref = &*tx_ref.db;
    let mut guard = match db_ref.inner.lock() {
        Ok(g) => g,
        Err(poisoned) => poisoned.into_inner(),
    };

    match guard.execute(query_str) {
        Ok(qr) => {
            set_ok(error_out);
            Box::into_raw(Box::new(CylResult::new(qr)))
        }
        Err(e) => {
            set_error(&e, error_out);
            std::ptr::null_mut()
        }
    }
}

/// Execute a Cypher query with parameters within the transaction.
///
/// # Safety
///
/// - `tx` must be a valid `CylTx` pointer.
/// - `query` must be a valid, null-terminated C string.
/// - `param_keys` must point to an array of `param_count` valid C strings.
/// - `param_values` must point to an array of `param_count` `CylValue`s.
/// - `error_out` must point to a valid `CylError` or be `NULL`.
#[no_mangle]
pub unsafe extern "C" fn cyl_tx_execute_with_params(
    tx: *mut CylTx,
    query: *const libc::c_char,
    param_keys: *const *const libc::c_char,
    param_values: *const CylValue,
    param_count: u32,
    error_out: *mut CylError,
) -> *mut CylResult {
    let Some(tx_ref) = validate_tx(tx, error_out) else {
        return std::ptr::null_mut();
    };
    let Some(query_str) = c_str_to_str(query, "query", error_out) else {
        return std::ptr::null_mut();
    };

    // Build parameter map.
    let mut params: HashMap<String, cypherlite_query::Value> = HashMap::new();
    if param_count > 0 {
        if param_keys.is_null() || param_values.is_null() {
            set_error_code(
                CylError::CylErrNullPointer,
                "param_keys or param_values is null",
                error_out,
            );
            return std::ptr::null_mut();
        }
        for i in 0..param_count as usize {
            // SAFETY: caller guarantees arrays are valid for param_count elements.
            let key_ptr = *param_keys.add(i);
            let Some(key) = c_str_to_str(key_ptr, "param_key", error_out) else {
                return std::ptr::null_mut();
            };
            let cyl_val = &*param_values.add(i);
            let value = crate::value::cyl_value_to_rust(cyl_val);
            params.insert(key.to_string(), value);
        }
    }

    // SAFETY: tx_ref.db was validated during cyl_tx_begin.
    let db_ref = &*tx_ref.db;
    let mut guard = match db_ref.inner.lock() {
        Ok(g) => g,
        Err(poisoned) => poisoned.into_inner(),
    };

    match guard.execute_with_params(query_str, params) {
        Ok(qr) => {
            set_ok(error_out);
            Box::into_raw(Box::new(CylResult::new(qr)))
        }
        Err(e) => {
            set_error(&e, error_out);
            std::ptr::null_mut()
        }
    }
}

// ---------------------------------------------------------------------------
// cyl_tx_commit / rollback / free
// ---------------------------------------------------------------------------

/// Commit the transaction and free the CylTx handle.
///
/// After this call the `tx` pointer is invalid. The database handle becomes
/// available for non-transactional queries again.
///
/// # Safety
///
/// - `tx` must be a valid `CylTx` pointer.
/// - `error_out` must point to a valid `CylError` or be `NULL`.
#[no_mangle]
pub unsafe extern "C" fn cyl_tx_commit(tx: *mut CylTx, error_out: *mut CylError) {
    if tx.is_null() {
        set_error_code(CylError::CylErrNullPointer, "tx is null", error_out);
        return;
    }

    // SAFETY: tx was allocated by cyl_tx_begin.
    let mut tx_box = Box::from_raw(tx);
    if tx_box.finished {
        set_error_code(
            CylError::CylErrTransactionConflict,
            "transaction already finished",
            error_out,
        );
        // Intentionally leak the box back -- it was already finished.
        // Actually, the caller should not use it after commit/rollback.
        // We drop it anyway to avoid a double-free scenario.
        return;
    }

    tx_box.finished = true;

    // Release the in_transaction flag on the parent database.
    // SAFETY: tx_box.db was validated during cyl_tx_begin.
    let db_ref = &*tx_box.db;
    db_ref.in_transaction.store(false, Ordering::Release);

    set_ok(error_out);
    // tx_box is dropped here, freeing the CylTx.
}

/// Rollback the transaction and free the CylTx handle.
///
/// Note: In the current Phase 2 implementation rollback is a no-op at the
/// storage level (changes already applied are not undone). Full WAL rollback
/// will be added in a future phase.
///
/// # Safety
///
/// - `tx` must be a valid `CylTx` pointer (or NULL for no-op).
#[no_mangle]
pub unsafe extern "C" fn cyl_tx_rollback(tx: *mut CylTx) {
    if tx.is_null() {
        return;
    }

    // SAFETY: tx was allocated by cyl_tx_begin.
    let mut tx_box = Box::from_raw(tx);
    if tx_box.finished {
        return;
    }

    tx_box.finished = true;

    // Release the in_transaction flag.
    // SAFETY: tx_box.db was validated during cyl_tx_begin.
    let db_ref = &*tx_box.db;
    db_ref.in_transaction.store(false, Ordering::Release);
    // tx_box is dropped here.
}

/// Free a CylTx handle. If the transaction was not committed or rolled back,
/// it is automatically rolled back.
///
/// No-op if `tx` is NULL.
///
/// # Safety
///
/// - `tx` must be a pointer returned by `cyl_tx_begin` (or NULL).
#[no_mangle]
pub unsafe extern "C" fn cyl_tx_free(tx: *mut CylTx) {
    if tx.is_null() {
        return;
    }

    // SAFETY: tx was allocated by cyl_tx_begin.
    let mut tx_box = Box::from_raw(tx);
    if !tx_box.finished {
        tx_box.finished = true;
        // Auto-rollback: release the flag.
        // SAFETY: tx_box.db was validated during cyl_tx_begin.
        let db_ref = &*tx_box.db;
        db_ref.in_transaction.store(false, Ordering::Release);
    }
    // tx_box is dropped here.
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Validate a CylTx pointer. Returns `None` (with error) if null or finished.
unsafe fn validate_tx<'a>(tx: *mut CylTx, error_out: *mut CylError) -> Option<&'a CylTx> {
    if tx.is_null() {
        set_error_code(CylError::CylErrNullPointer, "tx is null", error_out);
        return None;
    }
    let tx_ref = &*tx;
    if tx_ref.finished {
        set_error_code(
            CylError::CylErrTransactionConflict,
            "transaction already finished",
            error_out,
        );
        return None;
    }
    Some(tx_ref)
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{cyl_db_close, cyl_db_open};
    use crate::query::{cyl_db_execute, cyl_result_free};
    use std::ffi::CString;
    use tempfile::tempdir;

    /// Helper: open a fresh database.
    fn open_test_db() -> (*mut CylDb, tempfile::TempDir) {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("test.cyl");
        let c_path = CString::new(path.to_str().unwrap()).unwrap();
        let mut err = CylError::CylOk;
        let db = unsafe { cyl_db_open(c_path.as_ptr(), &mut err) };
        assert!(!db.is_null());
        (db, dir)
    }

    #[test]
    fn test_tx_begin_and_commit() {
        let (db, _dir) = open_test_db();
        let mut err = CylError::CylOk;

        // SAFETY: db is valid.
        let tx = unsafe { cyl_tx_begin(db, &mut err) };
        assert!(!tx.is_null());
        assert_eq!(err, CylError::CylOk);

        // SAFETY: tx is valid.
        unsafe { cyl_tx_commit(tx, &mut err) };
        assert_eq!(err, CylError::CylOk);

        unsafe { cyl_db_close(db) };
    }

    #[test]
    fn test_tx_begin_null_db() {
        let mut err = CylError::CylOk;
        let tx = unsafe { cyl_tx_begin(std::ptr::null_mut(), &mut err) };
        assert!(tx.is_null());
        assert_eq!(err, CylError::CylErrNullPointer);
    }

    #[test]
    fn test_tx_double_begin_fails() {
        let (db, _dir) = open_test_db();
        let mut err = CylError::CylOk;

        let tx1 = unsafe { cyl_tx_begin(db, &mut err) };
        assert!(!tx1.is_null());

        // Second begin should fail.
        let tx2 = unsafe { cyl_tx_begin(db, &mut err) };
        assert!(tx2.is_null());
        assert_eq!(err, CylError::CylErrTransactionConflict);

        // Clean up first transaction.
        unsafe { cyl_tx_rollback(tx1) };
        unsafe { cyl_db_close(db) };
    }

    #[test]
    fn test_tx_execute_create_and_commit() {
        let (db, _dir) = open_test_db();
        let mut err = CylError::CylOk;

        let tx = unsafe { cyl_tx_begin(db, &mut err) };
        assert!(!tx.is_null());

        let query = CString::new("CREATE (n:Person {name: 'Alice'})").unwrap();
        let result = unsafe { cyl_tx_execute(tx, query.as_ptr(), &mut err) };
        assert!(!result.is_null());
        assert_eq!(err, CylError::CylOk);
        unsafe { cyl_result_free(result) };

        unsafe { cyl_tx_commit(tx, &mut err) };
        assert_eq!(err, CylError::CylOk);

        // Verify data persists after commit.
        let match_q = CString::new("MATCH (n:Person) RETURN n.name").unwrap();
        let r = unsafe { cyl_db_execute(db, match_q.as_ptr(), &mut err) };
        assert!(!r.is_null());
        let res = unsafe { &*r };
        assert_eq!(res.inner.rows.len(), 1);
        unsafe { cyl_result_free(r) };

        unsafe { cyl_db_close(db) };
    }

    #[test]
    fn test_tx_rollback() {
        let (db, _dir) = open_test_db();
        let mut err = CylError::CylOk;

        let tx = unsafe { cyl_tx_begin(db, &mut err) };
        assert!(!tx.is_null());

        let query = CString::new("CREATE (n:Person {name: 'Bob'})").unwrap();
        let result = unsafe { cyl_tx_execute(tx, query.as_ptr(), &mut err) };
        assert!(!result.is_null());
        unsafe { cyl_result_free(result) };

        // Rollback.
        unsafe { cyl_tx_rollback(tx) };

        // db should be usable again.
        let match_q = CString::new("MATCH (n:Person) RETURN n.name").unwrap();
        let r = unsafe { cyl_db_execute(db, match_q.as_ptr(), &mut err) };
        assert!(!r.is_null());
        // Note: Phase 2 rollback is a no-op, data persists.
        unsafe { cyl_result_free(r) };

        unsafe { cyl_db_close(db) };
    }

    #[test]
    fn test_tx_free_auto_rollback() {
        let (db, _dir) = open_test_db();
        let mut err = CylError::CylOk;

        let tx = unsafe { cyl_tx_begin(db, &mut err) };
        assert!(!tx.is_null());

        // Free without commit or rollback.
        unsafe { cyl_tx_free(tx) };

        // db should be usable again (in_transaction flag cleared).
        let query = CString::new("CREATE (n:TestNode {v: 1})").unwrap();
        let r = unsafe { cyl_db_execute(db, query.as_ptr(), &mut err) };
        assert!(!r.is_null(), "db should accept queries after tx free");
        assert_eq!(err, CylError::CylOk);
        unsafe { cyl_result_free(r) };

        unsafe { cyl_db_close(db) };
    }

    #[test]
    fn test_tx_free_null_is_noop() {
        unsafe { cyl_tx_free(std::ptr::null_mut()) };
    }

    #[test]
    fn test_tx_rollback_null_is_noop() {
        unsafe { cyl_tx_rollback(std::ptr::null_mut()) };
    }

    #[test]
    fn test_tx_commit_null() {
        let mut err = CylError::CylOk;
        unsafe { cyl_tx_commit(std::ptr::null_mut(), &mut err) };
        assert_eq!(err, CylError::CylErrNullPointer);
    }

    #[test]
    fn test_tx_execute_null_tx() {
        let query = CString::new("RETURN 1").unwrap();
        let mut err = CylError::CylOk;
        let result = unsafe { cyl_tx_execute(std::ptr::null_mut(), query.as_ptr(), &mut err) };
        assert!(result.is_null());
        assert_eq!(err, CylError::CylErrNullPointer);
    }

    #[test]
    fn test_tx_execute_null_query() {
        let (db, _dir) = open_test_db();
        let mut err = CylError::CylOk;

        let tx = unsafe { cyl_tx_begin(db, &mut err) };
        assert!(!tx.is_null());

        let result = unsafe { cyl_tx_execute(tx, std::ptr::null(), &mut err) };
        assert!(result.is_null());
        assert_eq!(err, CylError::CylErrNullPointer);

        unsafe { cyl_tx_rollback(tx) };
        unsafe { cyl_db_close(db) };
    }

    #[test]
    fn test_db_execute_blocked_during_tx() {
        let (db, _dir) = open_test_db();
        let mut err = CylError::CylOk;

        let tx = unsafe { cyl_tx_begin(db, &mut err) };
        assert!(!tx.is_null());

        // Non-transactional execute should be blocked.
        let query = CString::new("RETURN 1").unwrap();
        let result = unsafe { cyl_db_execute(db, query.as_ptr(), &mut err) };
        assert!(result.is_null());
        assert_eq!(err, CylError::CylErrTransactionConflict);

        unsafe { cyl_tx_rollback(tx) };
        unsafe { cyl_db_close(db) };
    }

    #[test]
    fn test_tx_begin_after_commit_succeeds() {
        let (db, _dir) = open_test_db();
        let mut err = CylError::CylOk;

        let tx1 = unsafe { cyl_tx_begin(db, &mut err) };
        assert!(!tx1.is_null());
        unsafe { cyl_tx_commit(tx1, &mut err) };
        assert_eq!(err, CylError::CylOk);

        // Second transaction should succeed after first committed.
        let tx2 = unsafe { cyl_tx_begin(db, &mut err) };
        assert!(!tx2.is_null());
        unsafe { cyl_tx_rollback(tx2) };

        unsafe { cyl_db_close(db) };
    }

    #[test]
    fn test_tx_begin_after_rollback_succeeds() {
        let (db, _dir) = open_test_db();
        let mut err = CylError::CylOk;

        let tx1 = unsafe { cyl_tx_begin(db, &mut err) };
        assert!(!tx1.is_null());
        unsafe { cyl_tx_rollback(tx1) };

        let tx2 = unsafe { cyl_tx_begin(db, &mut err) };
        assert!(!tx2.is_null());
        unsafe { cyl_tx_commit(tx2, &mut err) };

        unsafe { cyl_db_close(db) };
    }
}
