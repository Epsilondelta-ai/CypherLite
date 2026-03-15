// CylResult access: column count/names, row count/row, cell access.
//
// All "borrow" functions return pointers valid only while the parent
// CylResult is alive. The caller MUST NOT free borrowed pointers.

use crate::error::{c_str_to_str, CylError};
use crate::query::CylResult;
use crate::value::{rust_value_to_cyl, CylValue};

// ---------------------------------------------------------------------------
// Column access
// ---------------------------------------------------------------------------

/// Return the number of columns in the result.
///
/// Returns 0 if `result` is NULL.
///
/// # Safety
///
/// - `result` must be a valid `CylResult` pointer (or NULL).
#[no_mangle]
pub unsafe extern "C" fn cyl_result_column_count(result: *const CylResult) -> u32 {
    if result.is_null() {
        return 0;
    }
    // SAFETY: caller guarantees result is valid.
    let res = &*result;
    res.inner.columns.len() as u32
}

/// Return the name of the column at `index` as a C string.
///
/// The returned pointer is borrowed from the CylResult and valid until the
/// result is freed. Returns NULL if `result` is NULL or `index` is out of
/// range.
///
/// # Safety
///
/// - `result` must be a valid `CylResult` pointer (or NULL).
#[no_mangle]
pub unsafe extern "C" fn cyl_result_column_name(
    result: *const CylResult,
    index: u32,
) -> *const libc::c_char {
    if result.is_null() {
        return std::ptr::null();
    }
    // SAFETY: caller guarantees result is valid.
    let res = &*result;
    let idx = index as usize;
    if idx >= res.column_cstrings.len() {
        return std::ptr::null();
    }
    res.column_cstrings[idx].as_ptr()
}

// ---------------------------------------------------------------------------
// Row access
// ---------------------------------------------------------------------------

/// Return the number of rows in the result.
///
/// Returns 0 if `result` is NULL.
///
/// # Safety
///
/// - `result` must be a valid `CylResult` pointer (or NULL).
#[no_mangle]
pub unsafe extern "C" fn cyl_result_row_count(result: *const CylResult) -> u64 {
    if result.is_null() {
        return 0;
    }
    // SAFETY: caller guarantees result is valid.
    let res = &*result;
    res.inner.rows.len() as u64
}

// ---------------------------------------------------------------------------
// Cell access (by column index)
// ---------------------------------------------------------------------------

/// Get a value from a specific row and column index.
///
/// Returns a CylValue by value. For String/Bytes the internal pointers borrow
/// from the CylResult -- they are valid until `cyl_result_free()` is called.
///
/// Returns a Null CylValue if any argument is NULL or out of range.
///
/// # Safety
///
/// - `result` must be a valid `CylResult` pointer (or NULL).
#[no_mangle]
pub unsafe extern "C" fn cyl_result_get(
    result: *const CylResult,
    row_index: u64,
    col_index: u32,
) -> CylValue {
    if result.is_null() {
        return CylValue::null();
    }
    // SAFETY: caller guarantees result is valid.
    let res = &*result;
    let ri = row_index as usize;
    let ci = col_index as usize;

    if ri >= res.inner.rows.len() || ci >= res.inner.columns.len() {
        return CylValue::null();
    }

    let col_name = &res.inner.columns[ci];
    match res.inner.rows[ri].get(col_name) {
        Some(val) => rust_value_to_cyl(val),
        None => CylValue::null(),
    }
}

/// Get a value from a specific row by column name.
///
/// Returns a CylValue by value. For String/Bytes the internal pointers borrow
/// from the CylResult.
///
/// Returns a Null CylValue if any argument is NULL, the column does not exist,
/// or the row index is out of range.
///
/// # Safety
///
/// - `result` must be a valid `CylResult` pointer (or NULL).
/// - `col_name` must be a valid, null-terminated C string (or NULL).
#[no_mangle]
pub unsafe extern "C" fn cyl_result_get_by_name(
    result: *const CylResult,
    row_index: u64,
    col_name: *const libc::c_char,
    error_out: *mut CylError,
) -> CylValue {
    if result.is_null() || col_name.is_null() {
        return CylValue::null();
    }
    let Some(name_str) = c_str_to_str(col_name, "col_name", error_out) else {
        return CylValue::null();
    };

    // SAFETY: caller guarantees result is valid.
    let res = &*result;
    let ri = row_index as usize;

    if ri >= res.inner.rows.len() {
        return CylValue::null();
    }

    match res.inner.rows[ri].get(name_str) {
        Some(val) => rust_value_to_cyl(val),
        None => CylValue::null(),
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{cyl_db_close, cyl_db_open};
    use crate::error::CylError;
    use crate::query::{cyl_db_execute, cyl_result_free};
    use crate::value::CylValueTag;
    use std::ffi::{CStr, CString};
    use tempfile::tempdir;

    /// Open a fresh db, create some data, and execute a MATCH query.
    fn setup_with_data() -> (*mut CylResult, *mut crate::db::CylDb, tempfile::TempDir) {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("res.cyl");
        let c_path = CString::new(path.to_str().unwrap()).unwrap();
        let mut err = CylError::CylOk;

        let db = unsafe { cyl_db_open(c_path.as_ptr(), &mut err) };
        assert!(!db.is_null());

        let create = CString::new("CREATE (n:Person {name: 'Alice', age: 30})").unwrap();
        let r = unsafe { cyl_db_execute(db, create.as_ptr(), &mut err) };
        unsafe { cyl_result_free(r) };

        let query = CString::new("MATCH (n:Person) RETURN n.name, n.age").unwrap();
        let result = unsafe { cyl_db_execute(db, query.as_ptr(), &mut err) };
        assert!(!result.is_null());

        (result, db, dir)
    }

    // -- Column access tests -----------------------------------------------

    #[test]
    fn test_column_count() {
        let (result, db, _dir) = setup_with_data();
        let count = unsafe { cyl_result_column_count(result) };
        assert_eq!(count, 2);
        unsafe { cyl_result_free(result) };
        unsafe { cyl_db_close(db) };
    }

    #[test]
    fn test_column_count_null_result() {
        assert_eq!(unsafe { cyl_result_column_count(std::ptr::null()) }, 0);
    }

    #[test]
    fn test_column_name_valid() {
        let (result, db, _dir) = setup_with_data();
        let res = unsafe { &*result };

        // Column names are sorted, so "n.age" comes before "n.name".
        let mut names: Vec<String> = vec![];
        for i in 0..res.inner.columns.len() {
            let ptr = unsafe { cyl_result_column_name(result, i as u32) };
            assert!(!ptr.is_null());
            let name = unsafe { CStr::from_ptr(ptr) }.to_str().unwrap();
            names.push(name.to_string());
        }
        names.sort();
        assert_eq!(names, vec!["n.age", "n.name"]);

        unsafe { cyl_result_free(result) };
        unsafe { cyl_db_close(db) };
    }

    #[test]
    fn test_column_name_out_of_range() {
        let (result, db, _dir) = setup_with_data();
        let ptr = unsafe { cyl_result_column_name(result, 999) };
        assert!(ptr.is_null());
        unsafe { cyl_result_free(result) };
        unsafe { cyl_db_close(db) };
    }

    #[test]
    fn test_column_name_null_result() {
        let ptr = unsafe { cyl_result_column_name(std::ptr::null(), 0) };
        assert!(ptr.is_null());
    }

    // -- Row access tests --------------------------------------------------

    #[test]
    fn test_row_count() {
        let (result, db, _dir) = setup_with_data();
        let count = unsafe { cyl_result_row_count(result) };
        assert_eq!(count, 1);
        unsafe { cyl_result_free(result) };
        unsafe { cyl_db_close(db) };
    }

    #[test]
    fn test_row_count_null_result() {
        assert_eq!(unsafe { cyl_result_row_count(std::ptr::null()) }, 0);
    }

    // -- Cell access by index tests ----------------------------------------

    #[test]
    fn test_result_get_string_value() {
        let (result, db, _dir) = setup_with_data();
        let res = unsafe { &*result };

        // Find the column index for "n.name".
        let name_idx = res
            .inner
            .columns
            .iter()
            .position(|c| c == "n.name")
            .unwrap();

        let cv = unsafe { cyl_result_get(result, 0, name_idx as u32) };
        assert_eq!(cv.tag, CylValueTag::String as u8);

        unsafe { cyl_result_free(result) };
        unsafe { cyl_db_close(db) };
    }

    #[test]
    fn test_result_get_int_value() {
        let (result, db, _dir) = setup_with_data();
        let res = unsafe { &*result };

        let age_idx = res.inner.columns.iter().position(|c| c == "n.age").unwrap();

        let cv = unsafe { cyl_result_get(result, 0, age_idx as u32) };
        assert_eq!(cv.tag, CylValueTag::Int64 as u8);
        assert_eq!(unsafe { cv.payload.int64 }, 30);

        unsafe { cyl_result_free(result) };
        unsafe { cyl_db_close(db) };
    }

    #[test]
    fn test_result_get_out_of_range_row() {
        let (result, db, _dir) = setup_with_data();
        let cv = unsafe { cyl_result_get(result, 999, 0) };
        assert_eq!(cv.tag, CylValueTag::Null as u8);
        unsafe { cyl_result_free(result) };
        unsafe { cyl_db_close(db) };
    }

    #[test]
    fn test_result_get_out_of_range_col() {
        let (result, db, _dir) = setup_with_data();
        let cv = unsafe { cyl_result_get(result, 0, 999) };
        assert_eq!(cv.tag, CylValueTag::Null as u8);
        unsafe { cyl_result_free(result) };
        unsafe { cyl_db_close(db) };
    }

    #[test]
    fn test_result_get_null_result() {
        let cv = unsafe { cyl_result_get(std::ptr::null(), 0, 0) };
        assert_eq!(cv.tag, CylValueTag::Null as u8);
    }

    // -- Cell access by name tests -----------------------------------------

    #[test]
    fn test_result_get_by_name() {
        let (result, db, _dir) = setup_with_data();
        let col = CString::new("n.age").unwrap();
        let mut err = CylError::CylOk;

        let cv = unsafe { cyl_result_get_by_name(result, 0, col.as_ptr(), &mut err) };
        assert_eq!(cv.tag, CylValueTag::Int64 as u8);
        assert_eq!(unsafe { cv.payload.int64 }, 30);

        unsafe { cyl_result_free(result) };
        unsafe { cyl_db_close(db) };
    }

    #[test]
    fn test_result_get_by_name_missing_column() {
        let (result, db, _dir) = setup_with_data();
        let col = CString::new("nonexistent").unwrap();
        let mut err = CylError::CylOk;

        let cv = unsafe { cyl_result_get_by_name(result, 0, col.as_ptr(), &mut err) };
        assert_eq!(cv.tag, CylValueTag::Null as u8);

        unsafe { cyl_result_free(result) };
        unsafe { cyl_db_close(db) };
    }

    #[test]
    fn test_result_get_by_name_null_col() {
        let (result, db, _dir) = setup_with_data();
        let cv =
            unsafe { cyl_result_get_by_name(result, 0, std::ptr::null(), std::ptr::null_mut()) };
        assert_eq!(cv.tag, CylValueTag::Null as u8);
        unsafe { cyl_result_free(result) };
        unsafe { cyl_db_close(db) };
    }

    #[test]
    fn test_result_get_by_name_null_result() {
        let col = CString::new("x").unwrap();
        let cv = unsafe {
            cyl_result_get_by_name(std::ptr::null(), 0, col.as_ptr(), std::ptr::null_mut())
        };
        assert_eq!(cv.tag, CylValueTag::Null as u8);
    }

    // -- Empty result tests ------------------------------------------------

    #[test]
    fn test_empty_result_column_and_row_count() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("empty.cyl");
        let c_path = CString::new(path.to_str().unwrap()).unwrap();
        let mut err = CylError::CylOk;

        let db = unsafe { cyl_db_open(c_path.as_ptr(), &mut err) };
        assert!(!db.is_null());

        // Query on empty database.
        let query = CString::new("MATCH (n:Nothing) RETURN n.x").unwrap();
        let result = unsafe { cyl_db_execute(db, query.as_ptr(), &mut err) };
        assert!(!result.is_null());

        assert_eq!(unsafe { cyl_result_row_count(result) }, 0);
        // Columns may or may not be present for empty results, depending on engine.

        unsafe { cyl_result_free(result) };
        unsafe { cyl_db_close(db) };
    }
}
