// CylDb: FFI wrapper around CypherLite database handle.
//
// CylDb wraps a Mutex<CypherLite> to provide Send+Sync guarantees for
// multi-threaded C consumers. An AtomicBool flag prevents concurrent access
// when a transaction is active.

use crate::error::{c_str_to_str, set_error, set_error_code, set_ok, CylError};
use cypherlite_core::DatabaseConfig;
use cypherlite_query::CypherLite;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

/// Opaque FFI handle to a CypherLite database.
pub struct CylDb {
    pub(crate) inner: Mutex<CypherLite>,
    pub(crate) in_transaction: AtomicBool,
}

/// Open a CypherLite database at the given file path with default settings.
///
/// Returns a heap-allocated `CylDb` pointer on success, or `NULL` on failure
/// (with `*error_out` set to the error code).
///
/// The caller MUST eventually call `cyl_db_close()` to free the handle.
///
/// # Safety
///
/// - `path` must be a valid, null-terminated C string.
/// - `error_out` must point to a valid `CylError` or be `NULL`.
#[no_mangle]
pub unsafe extern "C" fn cyl_db_open(
    path: *const libc::c_char,
    error_out: *mut CylError,
) -> *mut CylDb {
    let Some(path_str) = c_str_to_str(path, "path", error_out) else {
        return std::ptr::null_mut();
    };

    let config = DatabaseConfig {
        path: std::path::PathBuf::from(path_str),
        ..Default::default()
    };

    match CypherLite::open(config) {
        Ok(db) => {
            set_ok(error_out);
            Box::into_raw(Box::new(CylDb {
                inner: Mutex::new(db),
                in_transaction: AtomicBool::new(false),
            }))
        }
        Err(e) => {
            set_error(&e, error_out);
            std::ptr::null_mut()
        }
    }
}

/// Open a CypherLite database with explicit page size and cache capacity.
///
/// # Safety
///
/// - `path` must be a valid, null-terminated C string.
/// - `error_out` must point to a valid `CylError` or be `NULL`.
#[no_mangle]
pub unsafe extern "C" fn cyl_db_open_with_config(
    path: *const libc::c_char,
    page_size: u32,
    cache_capacity: u32,
    error_out: *mut CylError,
) -> *mut CylDb {
    let Some(path_str) = c_str_to_str(path, "path", error_out) else {
        return std::ptr::null_mut();
    };

    let config = DatabaseConfig {
        path: std::path::PathBuf::from(path_str),
        page_size,
        cache_capacity: cache_capacity as usize,
        ..Default::default()
    };

    match CypherLite::open(config) {
        Ok(db) => {
            set_ok(error_out);
            Box::into_raw(Box::new(CylDb {
                inner: Mutex::new(db),
                in_transaction: AtomicBool::new(false),
            }))
        }
        Err(e) => {
            set_error(&e, error_out);
            std::ptr::null_mut()
        }
    }
}

/// Close and free a CypherLite database handle.
///
/// This is a no-op if `db` is `NULL`. After this call the pointer is invalid.
///
/// # Safety
///
/// - `db` must be a pointer previously returned by `cyl_db_open` (or NULL).
/// - `db` must not be used after this call.
#[no_mangle]
pub unsafe extern "C" fn cyl_db_close(db: *mut CylDb) {
    if !db.is_null() {
        // SAFETY: db was allocated by Box::into_raw in cyl_db_open.
        drop(Box::from_raw(db));
    }
}

/// Check whether `db` is valid and not currently in a transaction.
/// Returns a reference to the CylDb on success, or None with error on failure.
///
/// # Safety
///
/// - `db` must be null or a valid `CylDb` pointer.
/// - `error_out` must be null or point to a valid `CylError`.
pub(crate) unsafe fn validate_db<'a>(
    db: *mut CylDb,
    error_out: *mut CylError,
) -> Option<&'a CylDb> {
    if db.is_null() {
        set_error_code(CylError::CylErrNullPointer, "db is null", error_out);
        return None;
    }
    // SAFETY: caller guarantees db points to a valid CylDb.
    let db_ref = &*db;
    if db_ref.in_transaction.load(Ordering::Acquire) {
        set_error_code(
            CylError::CylErrTransactionConflict,
            "database has an active transaction",
            error_out,
        );
        return None;
    }
    Some(db_ref)
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CString;
    use tempfile::tempdir;

    #[test]
    fn test_cyl_db_open_and_close() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("test.cyl");
        let c_path = CString::new(path.to_str().unwrap()).unwrap();
        let mut err = CylError::CylOk;

        // SAFETY: c_path is valid, err is on the stack.
        let db = unsafe { cyl_db_open(c_path.as_ptr(), &mut err) };
        assert!(!db.is_null(), "cyl_db_open should return non-null");
        assert_eq!(err, CylError::CylOk);

        // SAFETY: db was returned by cyl_db_open.
        unsafe { cyl_db_close(db) };
    }

    #[test]
    fn test_cyl_db_open_null_path_returns_null() {
        let mut err = CylError::CylOk;
        // SAFETY: null path is the test case.
        let db = unsafe { cyl_db_open(std::ptr::null(), &mut err) };
        assert!(db.is_null());
        assert_eq!(err, CylError::CylErrNullPointer);
    }

    #[test]
    fn test_cyl_db_close_null_is_noop() {
        // SAFETY: closing null should be a no-op.
        unsafe { cyl_db_close(std::ptr::null_mut()) };
    }

    #[test]
    fn test_cyl_db_open_with_config() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("cfg.cyl");
        let c_path = CString::new(path.to_str().unwrap()).unwrap();
        let mut err = CylError::CylOk;

        // SAFETY: c_path is valid.
        let db = unsafe { cyl_db_open_with_config(c_path.as_ptr(), 4096, 128, &mut err) };
        assert!(!db.is_null());
        assert_eq!(err, CylError::CylOk);

        // SAFETY: db was returned by cyl_db_open_with_config.
        unsafe { cyl_db_close(db) };
    }

    #[test]
    fn test_validate_db_null_returns_none() {
        let mut err = CylError::CylOk;
        // SAFETY: null is the test case; err is valid.
        assert!(unsafe { validate_db(std::ptr::null_mut(), &mut err) }.is_none());
        assert_eq!(err, CylError::CylErrNullPointer);
    }

    #[test]
    fn test_validate_db_in_transaction_returns_none() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("tx.cyl");
        let c_path = CString::new(path.to_str().unwrap()).unwrap();
        let mut err = CylError::CylOk;

        // SAFETY: c_path is valid.
        let db = unsafe { cyl_db_open(c_path.as_ptr(), &mut err) };
        assert!(!db.is_null());

        // Simulate in-transaction state.
        // SAFETY: db is valid.
        unsafe { &*db }
            .in_transaction
            .store(true, Ordering::Release);

        let mut err2 = CylError::CylOk;
        // SAFETY: db is valid; err2 is valid.
        assert!(unsafe { validate_db(db, &mut err2) }.is_none());
        assert_eq!(err2, CylError::CylErrTransactionConflict);

        // Reset and clean up.
        // SAFETY: db is valid.
        unsafe { &*db }
            .in_transaction
            .store(false, Ordering::Release);
        unsafe { cyl_db_close(db) };
    }

    #[test]
    fn test_validate_db_valid_returns_some() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("ok.cyl");
        let c_path = CString::new(path.to_str().unwrap()).unwrap();
        let mut err = CylError::CylOk;

        // SAFETY: c_path is valid.
        let db = unsafe { cyl_db_open(c_path.as_ptr(), &mut err) };
        assert!(!db.is_null());

        let mut err2 = CylError::CylOk;
        // SAFETY: db is valid; err2 is valid.
        assert!(unsafe { validate_db(db, &mut err2) }.is_some());
        assert_eq!(err2, CylError::CylOk);

        // SAFETY: db was returned by cyl_db_open.
        unsafe { cyl_db_close(db) };
    }
}
