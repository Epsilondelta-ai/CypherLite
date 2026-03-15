#![warn(missing_docs)]
//! C ABI bindings for the CypherLite embedded graph database.
//!
//! This crate provides a C-compatible interface wrapping the `cypherlite-query`
//! public API. All functions use `extern "C"` calling convention and follow
//! null-safe, error-out patterns suitable for consumption by C, Go, Python,
//! and Node.js callers.

/// Database lifecycle: open, close, and configuration.
pub mod db;
/// Error codes, thread-local error strings, and helper utilities.
pub mod error;
/// Direct query execution against a database handle.
pub mod query;
/// Query result iteration: columns, rows, and cell access.
pub mod result;
/// Transaction begin, execute, commit, and rollback.
pub mod transaction;
/// Value type conversions between C (`CylValue`) and Rust (`Value`).
pub mod value;

/// Library version string (null-terminated).
const VERSION: &str = concat!(env!("CARGO_PKG_VERSION"), "\0");

/// Return the library version as a C string.
///
/// The returned pointer is valid for the lifetime of the process and MUST NOT
/// be freed by the caller.
#[no_mangle]
pub extern "C" fn cyl_version() -> *const libc::c_char {
    // SAFETY: VERSION is a compile-time constant &str with embedded NUL.
    // The pointer is valid for the static lifetime.
    VERSION.as_ptr().cast::<libc::c_char>()
}

/// Build a comma-separated feature-flag string at compile time.
///
/// Each feature is conditionally included via `cfg!()` checks.  The result is
/// a `&'static str` with an embedded NUL terminator so it can be returned
/// directly as a C string pointer without allocation.
macro_rules! build_features_str {
    () => {{
        // Collect enabled features at compile time.
        const FEATURES: &[&str] = &[
            if cfg!(feature = "temporal-core") {
                "temporal-core"
            } else {
                ""
            },
            if cfg!(feature = "temporal-edge") {
                "temporal-edge"
            } else {
                ""
            },
            if cfg!(feature = "subgraph") {
                "subgraph"
            } else {
                ""
            },
            if cfg!(feature = "hypergraph") {
                "hypergraph"
            } else {
                ""
            },
            if cfg!(feature = "full-temporal") {
                "full-temporal"
            } else {
                ""
            },
            if cfg!(feature = "plugin") {
                "plugin"
            } else {
                ""
            },
        ];
        FEATURES
    }};
}

/// Return a comma-separated list of enabled feature flags as a C string.
///
/// The returned pointer is valid for the lifetime of the process and MUST NOT
/// be freed by the caller.  If no features are enabled the string is empty
/// (a single NUL byte).
#[no_mangle]
pub extern "C" fn cyl_features() -> *const libc::c_char {
    use std::sync::Once;

    static mut FEATURES_PTR: *const libc::c_char = std::ptr::null();
    static INIT: Once = Once::new();

    // SAFETY: `FEATURES_PTR` is written exactly once inside `call_once` and
    // only read after the `Once` barrier, so there is no data race.  The
    // `CString` is intentionally leaked to produce a `'static` pointer.
    unsafe {
        INIT.call_once(|| {
            let enabled: Vec<&str> = build_features_str!()
                .iter()
                .copied()
                .filter(|s| !s.is_empty())
                .collect();
            let joined = enabled.join(",");
            let cstring =
                std::ffi::CString::new(joined).expect("feature names do not contain NUL bytes");
            FEATURES_PTR = cstring.into_raw() as *const libc::c_char;
        });
        FEATURES_PTR
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CStr;

    // -- cyl_version tests ----------------------------------------------------

    #[test]
    fn test_cyl_version_returns_non_null() {
        let ptr = cyl_version();
        assert!(!ptr.is_null());
    }

    #[test]
    fn test_cyl_version_matches_cargo_version() {
        let ptr = cyl_version();
        // SAFETY: VERSION is a static &str with embedded null terminator.
        let cstr = unsafe { CStr::from_ptr(ptr) };
        assert_eq!(cstr.to_str().unwrap(), env!("CARGO_PKG_VERSION"));
    }

    #[test]
    fn test_cyl_version_is_valid_utf8() {
        let ptr = cyl_version();
        assert!(!ptr.is_null());
        // SAFETY: VERSION is a static &str with embedded null terminator.
        let cstr = unsafe { CStr::from_ptr(ptr) };
        assert!(cstr.to_str().is_ok(), "version string must be valid UTF-8");
    }

    #[test]
    fn test_cyl_version_stable_across_calls() {
        let ptr1 = cyl_version();
        let ptr2 = cyl_version();
        assert_eq!(ptr1, ptr2, "cyl_version must return stable pointer");
    }

    // -- cyl_features tests ---------------------------------------------------

    #[test]
    fn test_cyl_features_returns_non_null() {
        let ptr = cyl_features();
        assert!(!ptr.is_null());
    }

    #[test]
    fn test_cyl_features_is_valid_utf8() {
        let ptr = cyl_features();
        assert!(!ptr.is_null());
        // SAFETY: cyl_features returns a static, null-terminated C string.
        let cstr = unsafe { CStr::from_ptr(ptr) };
        assert!(cstr.to_str().is_ok(), "features string must be valid UTF-8");
    }

    #[test]
    fn test_cyl_features_stable_across_calls() {
        let ptr1 = cyl_features();
        let ptr2 = cyl_features();
        assert_eq!(ptr1, ptr2, "cyl_features must return stable pointer");
    }

    #[test]
    fn test_cyl_features_reflects_default_features() {
        let ptr = cyl_features();
        // SAFETY: cyl_features returns a static, null-terminated C string.
        let cstr = unsafe { CStr::from_ptr(ptr) };
        let features = cstr.to_str().unwrap();

        // The default feature set includes temporal-core.
        if cfg!(feature = "temporal-core") {
            assert!(
                features.contains("temporal-core"),
                "features should include temporal-core when enabled"
            );
        }
    }

    #[test]
    fn test_cyl_features_comma_separated_format() {
        let ptr = cyl_features();
        // SAFETY: cyl_features returns a static, null-terminated C string.
        let cstr = unsafe { CStr::from_ptr(ptr) };
        let features = cstr.to_str().unwrap();

        // If the string is non-empty, each item should not start/end with comma.
        if !features.is_empty() {
            assert!(
                !features.starts_with(','),
                "features must not start with comma"
            );
            assert!(!features.ends_with(','), "features must not end with comma");
            // Each segment between commas should be non-empty.
            for part in features.split(',') {
                assert!(!part.is_empty(), "feature name must not be empty");
            }
        }
    }

    #[cfg(feature = "subgraph")]
    #[test]
    fn test_cyl_features_includes_subgraph_when_enabled() {
        let ptr = cyl_features();
        let cstr = unsafe { CStr::from_ptr(ptr) };
        let features = cstr.to_str().unwrap();
        assert!(features.contains("subgraph"));
    }

    #[cfg(feature = "hypergraph")]
    #[test]
    fn test_cyl_features_includes_hypergraph_when_enabled() {
        let ptr = cyl_features();
        let cstr = unsafe { CStr::from_ptr(ptr) };
        let features = cstr.to_str().unwrap();
        assert!(features.contains("hypergraph"));
    }

    #[cfg(feature = "plugin")]
    #[test]
    fn test_cyl_features_includes_plugin_when_enabled() {
        let ptr = cyl_features();
        let cstr = unsafe { CStr::from_ptr(ptr) };
        let features = cstr.to_str().unwrap();
        assert!(features.contains("plugin"));
    }
}
