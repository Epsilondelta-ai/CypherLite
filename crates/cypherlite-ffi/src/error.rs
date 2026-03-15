// CylError: FFI error codes and thread-local error message storage.
//
// Every FFI function that can fail accepts an `error_out: *mut CylError`
// parameter. On success the pointed-to value is set to CYL_OK (0).
// On failure it is set to the appropriate error code and a human-readable
// message is stored in a thread-local buffer retrievable via
// `cyl_last_error_message()`.

use cypherlite_core::CypherLiteError;
use std::cell::RefCell;
use std::ffi::{CStr, CString};

// ---------------------------------------------------------------------------
// CylError -- #[repr(i32)] error code enum
// ---------------------------------------------------------------------------

/// FFI error codes returned by CypherLite C API functions.
#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CylError {
    /// Operation succeeded.
    CylOk = 0,
    /// I/O error (file system, permissions, etc.).
    CylErrIo = 1,
    /// Corrupted database page.
    CylErrCorruptedPage = 2,
    /// Write transaction conflict (another transaction is active).
    CylErrTransactionConflict = 3,
    /// Disk or buffer pool full.
    CylErrOutOfSpace = 4,
    /// Invalid magic number in database file.
    CylErrInvalidMagic = 5,
    /// Unsupported database format version.
    CylErrUnsupportedVersion = 6,
    /// Checksum verification failed.
    CylErrChecksumMismatch = 7,
    /// Serialization / deserialization failure.
    CylErrSerialization = 8,
    /// Referenced node does not exist.
    CylErrNodeNotFound = 9,
    /// Referenced edge does not exist.
    CylErrEdgeNotFound = 10,
    /// Cypher query parse error.
    CylErrParse = 11,
    /// Semantic analysis error.
    CylErrSemantic = 12,
    /// Query execution error.
    CylErrExecution = 13,
    /// Unsupported Cypher syntax.
    CylErrUnsupportedSyntax = 14,
    /// Constraint violation (uniqueness, etc.).
    CylErrConstraintViolation = 15,
    /// Invalid datetime format string.
    CylErrInvalidDateTime = 16,
    /// Attempted write to a read-only system property.
    CylErrSystemPropertyReadOnly = 17,
    /// Feature incompatibility between database file and compiled binary.
    CylErrFeatureIncompatible = 18,
    /// Null pointer passed where non-null was required.
    CylErrNullPointer = 19,
    /// String is not valid UTF-8.
    CylErrInvalidUtf8 = 20,

    // -- Feature-gated error codes -----------------------------------------
    /// Subgraph not found (requires `subgraph` feature).
    CylErrSubgraphNotFound = 100,
    /// Hyperedge not found (requires `hypergraph` feature).
    CylErrHyperedgeNotFound = 200,

    // -- Plugin error codes ------------------------------------------------
    /// Generic plugin error.
    CylErrPlugin = 300,
    /// Requested function not found in plugin registry.
    CylErrFunctionNotFound = 301,
    /// Unsupported index type.
    CylErrUnsupportedIndexType = 302,
    /// Unsupported serialization format.
    CylErrUnsupportedFormat = 303,
    /// Trigger execution error.
    CylErrTrigger = 304,
}

// ---------------------------------------------------------------------------
// Conversion from CypherLiteError to CylError
// ---------------------------------------------------------------------------

/// Map a Rust CypherLiteError to its FFI error code.
pub fn error_to_code(err: &CypherLiteError) -> CylError {
    match err {
        CypherLiteError::IoError(_) => CylError::CylErrIo,
        CypherLiteError::CorruptedPage { .. } => CylError::CylErrCorruptedPage,
        CypherLiteError::TransactionConflict => CylError::CylErrTransactionConflict,
        CypherLiteError::OutOfSpace => CylError::CylErrOutOfSpace,
        CypherLiteError::InvalidMagicNumber => CylError::CylErrInvalidMagic,
        CypherLiteError::UnsupportedVersion { .. } => CylError::CylErrUnsupportedVersion,
        CypherLiteError::ChecksumMismatch { .. } => CylError::CylErrChecksumMismatch,
        CypherLiteError::SerializationError(_) => CylError::CylErrSerialization,
        CypherLiteError::NodeNotFound(_) => CylError::CylErrNodeNotFound,
        CypherLiteError::EdgeNotFound(_) => CylError::CylErrEdgeNotFound,
        CypherLiteError::ParseError { .. } => CylError::CylErrParse,
        CypherLiteError::SemanticError(_) => CylError::CylErrSemantic,
        CypherLiteError::ExecutionError(_) => CylError::CylErrExecution,
        CypherLiteError::UnsupportedSyntax(_) => CylError::CylErrUnsupportedSyntax,
        CypherLiteError::ConstraintViolation(_) => CylError::CylErrConstraintViolation,
        CypherLiteError::InvalidDateTimeFormat(_) => CylError::CylErrInvalidDateTime,
        CypherLiteError::SystemPropertyReadOnly(_) => CylError::CylErrSystemPropertyReadOnly,
        CypherLiteError::FeatureIncompatible { .. } => CylError::CylErrFeatureIncompatible,
        #[cfg(feature = "subgraph")]
        CypherLiteError::SubgraphNotFound(_) => CylError::CylErrSubgraphNotFound,
        #[cfg(feature = "subgraph")]
        CypherLiteError::FeatureRequiresSubgraph => CylError::CylErrSubgraphNotFound,
        #[cfg(feature = "hypergraph")]
        CypherLiteError::HyperEdgeNotFound(_) => CylError::CylErrHyperedgeNotFound,
        #[cfg(feature = "plugin")]
        CypherLiteError::PluginError(_) => CylError::CylErrPlugin,
        #[cfg(feature = "plugin")]
        CypherLiteError::FunctionNotFound(_) => CylError::CylErrFunctionNotFound,
        #[cfg(feature = "plugin")]
        CypherLiteError::UnsupportedIndexType(_) => CylError::CylErrUnsupportedIndexType,
        #[cfg(feature = "plugin")]
        CypherLiteError::UnsupportedFormat(_) => CylError::CylErrUnsupportedFormat,
        #[cfg(feature = "plugin")]
        CypherLiteError::TriggerError(_) => CylError::CylErrTrigger,
    }
}

// ---------------------------------------------------------------------------
// Thread-local error message buffer
// ---------------------------------------------------------------------------

thread_local! {
    static LAST_ERROR: RefCell<Option<CString>> = const { RefCell::new(None) };
}

/// Store a human-readable error message in thread-local storage.
pub fn set_last_error(msg: &str) {
    // Replace interior NUL bytes with '?' to guarantee CString validity.
    let sanitized = msg.replace('\0', "?");
    LAST_ERROR.with(|cell| {
        // SAFETY: CString::new cannot fail here because we replaced NUL bytes.
        *cell.borrow_mut() = Some(CString::new(sanitized).expect("sanitized string"));
    });
}

/// Clear the thread-local error message.
pub fn clear_last_error() {
    LAST_ERROR.with(|cell| {
        *cell.borrow_mut() = None;
    });
}

/// Write `code` to `*error_out` if the pointer is non-null.
///
/// # Safety
///
/// `error_out` must be null or point to a valid, aligned `CylError`.
unsafe fn write_error_out(error_out: *mut CylError, code: CylError) {
    if !error_out.is_null() {
        // SAFETY: caller checked or guaranteed validity.
        unsafe { *error_out = code };
    }
}

/// Record a CypherLiteError: set the error code at `error_out` and store the
/// Display message in thread-local storage.
///
/// Returns the CylError code (for convenience in early-return patterns).
///
/// # Safety
///
/// `error_out` must be null or point to a valid `CylError`.
pub unsafe fn set_error(err: &CypherLiteError, error_out: *mut CylError) -> CylError {
    let code = error_to_code(err);
    set_last_error(&err.to_string());
    write_error_out(error_out, code);
    code
}

/// Write CylOk to `error_out` and clear the thread-local error message.
///
/// # Safety
///
/// `error_out` must be null or point to a valid `CylError`.
pub unsafe fn set_ok(error_out: *mut CylError) {
    clear_last_error();
    write_error_out(error_out, CylError::CylOk);
}

/// Write a specific CylError code to `error_out` with a custom message.
///
/// # Safety
///
/// `error_out` must be null or point to a valid `CylError`.
pub unsafe fn set_error_code(code: CylError, msg: &str, error_out: *mut CylError) {
    set_last_error(msg);
    write_error_out(error_out, code);
}

// ---------------------------------------------------------------------------
// Public FFI: cyl_last_error_message
// ---------------------------------------------------------------------------

/// Retrieve the most recent error message as a C string.
///
/// Returns `NULL` if no error has been recorded on the current thread.
/// The returned pointer is valid until the next FFI call on the same thread.
/// The caller MUST NOT free the returned pointer.
#[no_mangle]
pub extern "C" fn cyl_last_error_message() -> *const libc::c_char {
    LAST_ERROR.with(|cell| {
        let borrow = cell.borrow();
        match borrow.as_ref() {
            Some(cstr) => cstr.as_ptr(),
            None => std::ptr::null(),
        }
    })
}

/// Clear the thread-local error state.
#[no_mangle]
pub extern "C" fn cyl_clear_error() {
    clear_last_error();
}

// ---------------------------------------------------------------------------
// Helper: validate a C string argument and convert to &str.
// ---------------------------------------------------------------------------

/// Convert a `*const c_char` to `&str`, writing an error to `error_out` on
/// failure (null pointer or invalid UTF-8). Returns `None` on failure.
///
/// # Safety
///
/// - `ptr` must be null or a valid, null-terminated C string.
/// - `error_out` must be null or point to a valid `CylError`.
pub(crate) unsafe fn c_str_to_str<'a>(
    ptr: *const libc::c_char,
    arg_name: &str,
    error_out: *mut CylError,
) -> Option<&'a str> {
    if ptr.is_null() {
        set_error_code(
            CylError::CylErrNullPointer,
            &format!("{arg_name} is null"),
            error_out,
        );
        return None;
    }
    // SAFETY: caller guarantees ptr is a valid, null-terminated C string.
    let cstr = CStr::from_ptr(ptr);
    match cstr.to_str() {
        Ok(s) => Some(s),
        Err(_) => {
            set_error_code(
                CylError::CylErrInvalidUtf8,
                &format!("{arg_name} is not valid UTF-8"),
                error_out,
            );
            None
        }
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // -- CylError repr tests -----------------------------------------------

    #[test]
    fn test_cyl_ok_is_zero() {
        assert_eq!(CylError::CylOk as i32, 0);
    }

    #[test]
    fn test_error_codes_are_distinct() {
        let codes = [
            CylError::CylOk,
            CylError::CylErrIo,
            CylError::CylErrCorruptedPage,
            CylError::CylErrTransactionConflict,
            CylError::CylErrOutOfSpace,
            CylError::CylErrInvalidMagic,
            CylError::CylErrUnsupportedVersion,
            CylError::CylErrChecksumMismatch,
            CylError::CylErrSerialization,
            CylError::CylErrNodeNotFound,
            CylError::CylErrEdgeNotFound,
            CylError::CylErrParse,
            CylError::CylErrSemantic,
            CylError::CylErrExecution,
            CylError::CylErrUnsupportedSyntax,
            CylError::CylErrConstraintViolation,
            CylError::CylErrInvalidDateTime,
            CylError::CylErrSystemPropertyReadOnly,
            CylError::CylErrFeatureIncompatible,
            CylError::CylErrNullPointer,
            CylError::CylErrInvalidUtf8,
            CylError::CylErrSubgraphNotFound,
            CylError::CylErrHyperedgeNotFound,
        ];
        let mut values: Vec<i32> = codes.iter().map(|c| *c as i32).collect();
        let len_before = values.len();
        values.sort();
        values.dedup();
        assert_eq!(values.len(), len_before, "duplicate error codes detected");
    }

    // -- error_to_code mapping tests ---------------------------------------

    #[test]
    fn test_error_to_code_io() {
        let err =
            CypherLiteError::IoError(std::io::Error::new(std::io::ErrorKind::NotFound, "gone"));
        assert_eq!(error_to_code(&err), CylError::CylErrIo);
    }

    #[test]
    fn test_error_to_code_corrupted_page() {
        let err = CypherLiteError::CorruptedPage {
            page_id: 1,
            reason: "bad".into(),
        };
        assert_eq!(error_to_code(&err), CylError::CylErrCorruptedPage);
    }

    #[test]
    fn test_error_to_code_transaction_conflict() {
        assert_eq!(
            error_to_code(&CypherLiteError::TransactionConflict),
            CylError::CylErrTransactionConflict
        );
    }

    #[test]
    fn test_error_to_code_parse() {
        let err = CypherLiteError::ParseError {
            line: 1,
            column: 1,
            message: "bad".into(),
        };
        assert_eq!(error_to_code(&err), CylError::CylErrParse);
    }

    #[test]
    fn test_error_to_code_semantic() {
        assert_eq!(
            error_to_code(&CypherLiteError::SemanticError("x".into())),
            CylError::CylErrSemantic
        );
    }

    #[test]
    fn test_error_to_code_execution() {
        assert_eq!(
            error_to_code(&CypherLiteError::ExecutionError("x".into())),
            CylError::CylErrExecution
        );
    }

    #[test]
    fn test_error_to_code_node_not_found() {
        assert_eq!(
            error_to_code(&CypherLiteError::NodeNotFound(1)),
            CylError::CylErrNodeNotFound
        );
    }

    #[test]
    fn test_error_to_code_edge_not_found() {
        assert_eq!(
            error_to_code(&CypherLiteError::EdgeNotFound(1)),
            CylError::CylErrEdgeNotFound
        );
    }

    #[cfg(feature = "subgraph")]
    #[test]
    fn test_error_to_code_subgraph_not_found() {
        assert_eq!(
            error_to_code(&CypherLiteError::SubgraphNotFound(1)),
            CylError::CylErrSubgraphNotFound
        );
    }

    #[cfg(feature = "hypergraph")]
    #[test]
    fn test_error_to_code_hyperedge_not_found() {
        assert_eq!(
            error_to_code(&CypherLiteError::HyperEdgeNotFound(1)),
            CylError::CylErrHyperedgeNotFound
        );
    }

    // -- Thread-local message tests ----------------------------------------

    #[test]
    fn test_set_last_error_stores_message() {
        set_last_error("something went wrong");
        let ptr = cyl_last_error_message();
        assert!(!ptr.is_null());
        // SAFETY: ptr was just returned from thread-local storage.
        let msg = unsafe { CStr::from_ptr(ptr) }.to_str().unwrap();
        assert_eq!(msg, "something went wrong");
    }

    #[test]
    fn test_clear_last_error_nulls_message() {
        set_last_error("error");
        clear_last_error();
        let ptr = cyl_last_error_message();
        assert!(ptr.is_null());
    }

    #[test]
    fn test_cyl_clear_error_ffi() {
        set_last_error("error");
        cyl_clear_error();
        assert!(cyl_last_error_message().is_null());
    }

    #[test]
    fn test_set_last_error_sanitizes_nul_bytes() {
        set_last_error("hello\0world");
        let ptr = cyl_last_error_message();
        assert!(!ptr.is_null());
        // SAFETY: ptr was just set.
        let msg = unsafe { CStr::from_ptr(ptr) }.to_str().unwrap();
        assert_eq!(msg, "hello?world");
    }

    // -- set_error / set_ok tests ------------------------------------------

    #[test]
    fn test_set_error_writes_code_and_message() {
        let err = CypherLiteError::NodeNotFound(42);
        let mut code = CylError::CylOk;
        // SAFETY: code is a valid stack variable.
        unsafe { set_error(&err, &mut code) };
        assert_eq!(code, CylError::CylErrNodeNotFound);
        let ptr = cyl_last_error_message();
        assert!(!ptr.is_null());
        // SAFETY: ptr was just set.
        let msg = unsafe { CStr::from_ptr(ptr) }.to_str().unwrap();
        assert!(msg.contains("42"));
    }

    #[test]
    fn test_set_ok_clears_error() {
        set_last_error("previous error");
        let mut code = CylError::CylErrIo;
        // SAFETY: code is a valid stack variable.
        unsafe { set_ok(&mut code) };
        assert_eq!(code, CylError::CylOk);
        assert!(cyl_last_error_message().is_null());
    }

    #[test]
    fn test_set_error_null_error_out_does_not_crash() {
        let err = CypherLiteError::OutOfSpace;
        // SAFETY: null error_out should be handled gracefully.
        unsafe { set_error(&err, std::ptr::null_mut()) };
    }

    // -- c_str_to_str tests ------------------------------------------------

    #[test]
    fn test_c_str_to_str_null_returns_none() {
        let mut code = CylError::CylOk;
        // SAFETY: null ptr is the test case; code is valid.
        let result = unsafe { c_str_to_str(std::ptr::null(), "path", &mut code) };
        assert!(result.is_none());
        assert_eq!(code, CylError::CylErrNullPointer);
    }

    #[test]
    fn test_c_str_to_str_valid_string() {
        let s = CString::new("hello").unwrap();
        let mut code = CylError::CylOk;
        // SAFETY: s is a valid CString; code is valid.
        let result = unsafe { c_str_to_str(s.as_ptr(), "arg", &mut code) };
        assert_eq!(result, Some("hello"));
    }

    #[test]
    fn test_c_str_to_str_invalid_utf8() {
        // Create a byte sequence that is not valid UTF-8.
        let bytes: Vec<u8> = vec![0xFF, 0xFE, 0x00]; // null-terminated
        let mut code = CylError::CylOk;
        // SAFETY: bytes is a valid null-terminated sequence; code is valid.
        let result =
            unsafe { c_str_to_str(bytes.as_ptr().cast::<libc::c_char>(), "bad", &mut code) };
        assert!(result.is_none());
        assert_eq!(code, CylError::CylErrInvalidUtf8);
    }
}
