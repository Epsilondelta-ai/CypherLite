// Query execution: cyl_db_execute, cyl_db_execute_with_params.
//
// CylResult is an opaque FFI handle owning a QueryResult and pre-computed
// CString column names for zero-copy access from C.

use crate::db::{validate_db, CylDb};
use crate::error::{c_str_to_str, set_error, set_error_code, set_ok, CylError};
use crate::value::cyl_value_to_rust;
use cypherlite_query::{QueryResult, Value};
use std::collections::HashMap;
use std::ffi::CString;

// ---------------------------------------------------------------------------
// CylResult -- opaque FFI result handle
// ---------------------------------------------------------------------------

/// Opaque FFI handle owning query results.
///
/// Stores the original `QueryResult` plus pre-computed CString column names
/// so that `cyl_result_column_name` can return borrowed pointers without
/// per-call allocation.
pub struct CylResult {
    pub(crate) inner: QueryResult,
    /// CString versions of column names (same order as inner.columns).
    pub(crate) column_cstrings: Vec<CString>,
}

impl CylResult {
    /// Wrap a QueryResult, pre-computing CString column names.
    pub(crate) fn new(qr: QueryResult) -> Self {
        let column_cstrings = qr
            .columns
            .iter()
            .map(|c| CString::new(c.as_str()).unwrap_or_else(|_| CString::new("?").unwrap()))
            .collect();
        Self {
            inner: qr,
            column_cstrings,
        }
    }
}

// ---------------------------------------------------------------------------
// cyl_db_execute
// ---------------------------------------------------------------------------

/// Execute a Cypher query string on the database.
///
/// Returns a heap-allocated `CylResult` on success, or `NULL` on failure.
/// The caller MUST call `cyl_result_free()` to release the result.
///
/// # Safety
///
/// - `db` must be a valid `CylDb` pointer (not in-transaction).
/// - `query` must be a valid, null-terminated C string.
/// - `error_out` must point to a valid `CylError` or be `NULL`.
#[no_mangle]
pub unsafe extern "C" fn cyl_db_execute(
    db: *mut CylDb,
    query: *const libc::c_char,
    error_out: *mut CylError,
) -> *mut CylResult {
    let Some(db_ref) = validate_db(db, error_out) else {
        return std::ptr::null_mut();
    };
    let Some(query_str) = c_str_to_str(query, "query", error_out) else {
        return std::ptr::null_mut();
    };

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

/// Execute a Cypher query with named parameters.
///
/// Parameters are passed as parallel arrays of keys and values plus a count.
/// Each `param_keys[i]` is a null-terminated C string and `param_values[i]`
/// is a `CylValue` (see value.rs).
///
/// # Safety
///
/// - `db` must be a valid `CylDb` pointer (not in-transaction).
/// - `query` must be a valid, null-terminated C string.
/// - `param_keys` must point to an array of `param_count` valid C strings.
/// - `param_values` must point to an array of `param_count` `CylValue`s.
/// - `error_out` must point to a valid `CylError` or be `NULL`.
#[no_mangle]
pub unsafe extern "C" fn cyl_db_execute_with_params(
    db: *mut CylDb,
    query: *const libc::c_char,
    param_keys: *const *const libc::c_char,
    param_values: *const crate::value::CylValue,
    param_count: u32,
    error_out: *mut CylError,
) -> *mut CylResult {
    let Some(db_ref) = validate_db(db, error_out) else {
        return std::ptr::null_mut();
    };
    let Some(query_str) = c_str_to_str(query, "query", error_out) else {
        return std::ptr::null_mut();
    };

    // Build parameter map.
    let mut params: HashMap<String, Value> = HashMap::new();
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
            let value = cyl_value_to_rust(cyl_val);
            params.insert(key.to_string(), value);
        }
    }

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

/// Free a CylResult handle. No-op if `result` is NULL.
///
/// # Safety
///
/// - `result` must be a pointer returned by `cyl_db_execute` (or NULL).
#[no_mangle]
pub unsafe extern "C" fn cyl_result_free(result: *mut CylResult) {
    if !result.is_null() {
        // SAFETY: result was allocated by Box::into_raw.
        drop(Box::from_raw(result));
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{cyl_db_close, cyl_db_open};
    use crate::value::{CylValue, CylValueTag};
    use std::ffi::CString;
    use tempfile::tempdir;

    /// Helper: open a fresh database and return (db_ptr, _dir).
    fn open_test_db() -> (*mut CylDb, tempfile::TempDir) {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("test.cyl");
        let c_path = CString::new(path.to_str().unwrap()).unwrap();
        let mut err = CylError::CylOk;
        // SAFETY: c_path is valid.
        let db = unsafe { cyl_db_open(c_path.as_ptr(), &mut err) };
        assert!(!db.is_null());
        (db, dir)
    }

    #[test]
    fn test_cyl_db_execute_create() {
        let (db, _dir) = open_test_db();
        let query = CString::new("CREATE (n:Person {name: 'Alice'})").unwrap();
        let mut err = CylError::CylOk;

        // SAFETY: db and query are valid.
        let result = unsafe { cyl_db_execute(db, query.as_ptr(), &mut err) };
        assert!(!result.is_null(), "execute CREATE should succeed");
        assert_eq!(err, CylError::CylOk);

        // SAFETY: result was returned by cyl_db_execute.
        unsafe { cyl_result_free(result) };
        unsafe { cyl_db_close(db) };
    }

    #[test]
    fn test_cyl_db_execute_match_returns_rows() {
        let (db, _dir) = open_test_db();
        let mut err = CylError::CylOk;

        // Create a node first.
        let create = CString::new("CREATE (n:Person {name: 'Bob'})").unwrap();
        // SAFETY: db and create are valid.
        let r = unsafe { cyl_db_execute(db, create.as_ptr(), &mut err) };
        unsafe { cyl_result_free(r) };

        // Now query.
        let query = CString::new("MATCH (n:Person) RETURN n.name").unwrap();
        // SAFETY: db and query are valid.
        let result = unsafe { cyl_db_execute(db, query.as_ptr(), &mut err) };
        assert!(!result.is_null());
        assert_eq!(err, CylError::CylOk);

        // Check row count via the inner QueryResult.
        // SAFETY: result is valid.
        let res = unsafe { &*result };
        assert_eq!(res.inner.rows.len(), 1);

        unsafe { cyl_result_free(result) };
        unsafe { cyl_db_close(db) };
    }

    #[test]
    fn test_cyl_db_execute_null_query() {
        let (db, _dir) = open_test_db();
        let mut err = CylError::CylOk;

        // SAFETY: null query is the test case.
        let result = unsafe { cyl_db_execute(db, std::ptr::null(), &mut err) };
        assert!(result.is_null());
        assert_eq!(err, CylError::CylErrNullPointer);

        unsafe { cyl_db_close(db) };
    }

    #[test]
    fn test_cyl_db_execute_null_db() {
        let query = CString::new("RETURN 1").unwrap();
        let mut err = CylError::CylOk;

        // SAFETY: null db is the test case.
        let result = unsafe { cyl_db_execute(std::ptr::null_mut(), query.as_ptr(), &mut err) };
        assert!(result.is_null());
        assert_eq!(err, CylError::CylErrNullPointer);
    }

    #[test]
    fn test_cyl_db_execute_parse_error() {
        let (db, _dir) = open_test_db();
        let query = CString::new("INVALID QUERY @#$").unwrap();
        let mut err = CylError::CylOk;

        // SAFETY: db and query are valid.
        let result = unsafe { cyl_db_execute(db, query.as_ptr(), &mut err) };
        assert!(result.is_null());
        assert_eq!(err, CylError::CylErrParse);

        unsafe { cyl_db_close(db) };
    }

    #[test]
    fn test_cyl_result_free_null_is_noop() {
        // SAFETY: null is allowed.
        unsafe { cyl_result_free(std::ptr::null_mut()) };
    }

    #[test]
    fn test_cyl_db_execute_with_params_basic() {
        let (db, _dir) = open_test_db();
        let mut err = CylError::CylOk;

        // Create a node.
        let create = CString::new("CREATE (n:Person {name: 'Alice'})").unwrap();
        // SAFETY: db and create are valid.
        let r = unsafe { cyl_db_execute(db, create.as_ptr(), &mut err) };
        unsafe { cyl_result_free(r) };

        // Query with parameter.
        let query = CString::new("MATCH (n:Person) WHERE n.name = $name RETURN n.name").unwrap();
        let key = CString::new("name").unwrap();
        let key_ptr = key.as_ptr();
        let name_bytes = CString::new("Alice").unwrap();
        let val = CylValue {
            tag: CylValueTag::String as u8,
            payload: crate::value::CylValuePayload {
                string: name_bytes.as_ptr(),
            },
        };

        // SAFETY: all pointers are valid.
        let result = unsafe {
            cyl_db_execute_with_params(
                db,
                query.as_ptr(),
                &key_ptr as *const *const libc::c_char,
                &val as *const CylValue,
                1,
                &mut err,
            )
        };
        assert!(!result.is_null(), "execute_with_params should succeed");
        assert_eq!(err, CylError::CylOk);

        // SAFETY: result is valid.
        let res = unsafe { &*result };
        assert_eq!(res.inner.rows.len(), 1);

        unsafe { cyl_result_free(result) };
        unsafe { cyl_db_close(db) };
    }

    #[test]
    fn test_cyl_db_execute_in_transaction_returns_error() {
        let (db, _dir) = open_test_db();

        // Simulate transaction state.
        // SAFETY: db is valid.
        unsafe { &*db }
            .in_transaction
            .store(true, std::sync::atomic::Ordering::Release);

        let query = CString::new("RETURN 1").unwrap();
        let mut err = CylError::CylOk;

        // SAFETY: db and query are valid.
        let result = unsafe { cyl_db_execute(db, query.as_ptr(), &mut err) };
        assert!(result.is_null());
        assert_eq!(err, CylError::CylErrTransactionConflict);

        // Reset.
        unsafe { &*db }
            .in_transaction
            .store(false, std::sync::atomic::Ordering::Release);
        unsafe { cyl_db_close(db) };
    }
}
