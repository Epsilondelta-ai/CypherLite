// CylValue: FFI tagged union for query values.
//
// CylValue is a `#[repr(C)]` struct with a `u8` tag discriminant and a
// union payload. This allows C consumers to switch on the tag and read the
// appropriate payload field.

use cypherlite_query::Value;
use std::ffi::CStr;

// ---------------------------------------------------------------------------
// Tag constants
// ---------------------------------------------------------------------------

/// Discriminant tags for `CylValue`.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CylValueTag {
    /// Null value (tag 0).
    Null = 0,
    /// Boolean value (tag 1).
    Bool = 1,
    /// 64-bit signed integer (tag 2).
    Int64 = 2,
    /// 64-bit floating point (tag 3).
    Float64 = 3,
    /// Null-terminated UTF-8 string (tag 4).
    String = 4,
    /// Raw byte array (tag 5).
    Bytes = 5,
    /// Ordered list of values (tag 6).
    List = 6,
    /// Node entity ID (tag 7).
    Node = 7,
    /// Edge entity ID (tag 8).
    Edge = 8,
    /// DateTime as millis since epoch (tag 9).
    DateTime = 9,
    /// Subgraph entity ID (tag 10, requires subgraph feature).
    #[cfg(feature = "subgraph")]
    Subgraph = 10,
    /// Hyperedge entity ID (tag 11, requires hypergraph feature).
    #[cfg(feature = "hypergraph")]
    Hyperedge = 11,
    /// Temporal node reference: node ID + timestamp (tag 12, requires hypergraph feature).
    #[cfg(feature = "hypergraph")]
    TemporalNode = 12,
}

// ---------------------------------------------------------------------------
// CylValue payload union
// ---------------------------------------------------------------------------

/// Payload union for `CylValue`.
///
/// Which field is valid depends on the `tag` in the enclosing `CylValue`.
#[repr(C)]
#[derive(Clone, Copy)]
pub union CylValuePayload {
    /// CylValueTag::Bool
    pub boolean: bool,
    /// CylValueTag::Int64 / DateTime
    pub int64: i64,
    /// CylValueTag::Float64
    pub float64: f64,
    /// CylValueTag::String -- pointer to null-terminated UTF-8.
    /// For values returned by cyl_row_get, this is borrowed from the result.
    /// For parameter values, the caller owns the string.
    pub string: *const libc::c_char,
    /// CylValueTag::Bytes -- pointer + length.
    pub bytes: CylBytes,
    /// CylValueTag::Node -- node id.
    pub node_id: u64,
    /// CylValueTag::Edge -- edge id.
    pub edge_id: u64,
    /// CylValueTag::List -- pointer + length.
    pub list: CylList,
    /// CylValueTag::TemporalNode -- (node_id, timestamp_ms).
    #[cfg(feature = "hypergraph")]
    pub temporal_node: CylTemporalNode,
}

/// Byte slice representation for FFI.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct CylBytes {
    /// Pointer to the byte data.
    pub data: *const u8,
    /// Number of bytes.
    pub len: u32,
}

/// List representation for FFI (array of CylValue pointers).
#[repr(C)]
#[derive(Clone, Copy)]
pub struct CylList {
    /// Pointer to the first element.
    pub items: *const CylValue,
    /// Number of elements.
    pub len: u32,
}

/// Temporal node reference (node_id + timestamp in millis).
#[cfg(feature = "hypergraph")]
#[repr(C)]
#[derive(Clone, Copy)]
pub struct CylTemporalNode {
    /// Node entity ID.
    pub node_id: u64,
    /// Timestamp in milliseconds since Unix epoch.
    pub timestamp_ms: i64,
}

// ---------------------------------------------------------------------------
// CylValue struct
// ---------------------------------------------------------------------------

/// A tagged union representing a single query value for FFI.
#[repr(C)]
pub struct CylValue {
    /// Discriminant tag (see [`CylValueTag`]).
    pub tag: u8,
    /// Value payload (interpret according to `tag`).
    pub payload: CylValuePayload,
}

impl CylValue {
    /// Create a Null CylValue.
    pub fn null() -> Self {
        Self {
            tag: CylValueTag::Null as u8,
            payload: CylValuePayload { int64: 0 },
        }
    }

    /// Create a Bool CylValue.
    pub fn from_bool(v: bool) -> Self {
        Self {
            tag: CylValueTag::Bool as u8,
            payload: CylValuePayload { boolean: v },
        }
    }

    /// Create an Int64 CylValue.
    pub fn from_int64(v: i64) -> Self {
        Self {
            tag: CylValueTag::Int64 as u8,
            payload: CylValuePayload { int64: v },
        }
    }

    /// Create a Float64 CylValue.
    pub fn from_float64(v: f64) -> Self {
        Self {
            tag: CylValueTag::Float64 as u8,
            payload: CylValuePayload { float64: v },
        }
    }
}

// ---------------------------------------------------------------------------
// CylValue -> Rust Value conversion (for parameter passing)
// ---------------------------------------------------------------------------

/// Convert a CylValue (from C caller) to a Rust Value.
///
/// For String parameters, the C string is copied into a Rust String.
/// For Bytes, the data is copied into a `Vec<u8>`.
///
/// # Safety
///
/// The CylValue's payload must be valid for its tag (e.g., string pointer
/// must be non-null and null-terminated for tag==String).
pub fn cyl_value_to_rust(cv: &CylValue) -> Value {
    // SAFETY: caller guarantees the payload matches the tag.
    unsafe {
        match cv.tag {
            t if t == CylValueTag::Null as u8 => Value::Null,
            t if t == CylValueTag::Bool as u8 => Value::Bool(cv.payload.boolean),
            t if t == CylValueTag::Int64 as u8 => Value::Int64(cv.payload.int64),
            t if t == CylValueTag::Float64 as u8 => Value::Float64(cv.payload.float64),
            t if t == CylValueTag::String as u8 => {
                if cv.payload.string.is_null() {
                    Value::Null
                } else {
                    let cstr = CStr::from_ptr(cv.payload.string);
                    Value::String(cstr.to_string_lossy().into_owned())
                }
            }
            t if t == CylValueTag::Bytes as u8 => {
                let bytes_info = cv.payload.bytes;
                if bytes_info.data.is_null() || bytes_info.len == 0 {
                    Value::Bytes(vec![])
                } else {
                    let slice =
                        std::slice::from_raw_parts(bytes_info.data, bytes_info.len as usize);
                    Value::Bytes(slice.to_vec())
                }
            }
            t if t == CylValueTag::Node as u8 => {
                Value::Node(cypherlite_core::NodeId(cv.payload.node_id))
            }
            t if t == CylValueTag::Edge as u8 => {
                Value::Edge(cypherlite_core::EdgeId(cv.payload.edge_id))
            }
            t if t == CylValueTag::DateTime as u8 => Value::DateTime(cv.payload.int64),
            _ => Value::Null,
        }
    }
}

// ---------------------------------------------------------------------------
// Rust Value -> CylValue conversion (for result access)
// ---------------------------------------------------------------------------

/// Convert a Rust Value to a CylValue for FFI return.
///
/// For String and Bytes values, the returned CylValue borrows from the
/// original Value (via raw pointers). The CylValue is only valid while
/// the source CylResult is alive.
pub fn rust_value_to_cyl(value: &Value) -> CylValue {
    match value {
        Value::Null => CylValue::null(),
        Value::Bool(b) => CylValue::from_bool(*b),
        Value::Int64(i) => CylValue::from_int64(*i),
        Value::Float64(f) => CylValue::from_float64(*f),
        Value::String(s) => {
            // Return a pointer into the Rust-owned string. The caller must
            // not outlive the CylResult that owns this Value.
            CylValue {
                tag: CylValueTag::String as u8,
                payload: CylValuePayload {
                    string: s.as_ptr().cast::<libc::c_char>(),
                },
            }
        }
        Value::Bytes(b) => CylValue {
            tag: CylValueTag::Bytes as u8,
            payload: CylValuePayload {
                bytes: CylBytes {
                    data: b.as_ptr(),
                    len: b.len() as u32,
                },
            },
        },
        Value::List(items) => {
            // For lists we cannot return a borrowed array of CylValues because
            // they don't exist in memory. Return null/0 and let callers use
            // a dedicated list-access API if needed.
            CylValue {
                tag: CylValueTag::List as u8,
                payload: CylValuePayload {
                    list: CylList {
                        items: std::ptr::null(),
                        len: items.len() as u32,
                    },
                },
            }
        }
        Value::Node(nid) => CylValue {
            tag: CylValueTag::Node as u8,
            payload: CylValuePayload { node_id: nid.0 },
        },
        Value::Edge(eid) => CylValue {
            tag: CylValueTag::Edge as u8,
            payload: CylValuePayload { edge_id: eid.0 },
        },
        Value::DateTime(ms) => CylValue {
            tag: CylValueTag::DateTime as u8,
            payload: CylValuePayload { int64: *ms },
        },
        #[cfg(feature = "subgraph")]
        Value::Subgraph(sid) => CylValue {
            tag: CylValueTag::Subgraph as u8,
            payload: CylValuePayload { node_id: sid.0 },
        },
        #[cfg(feature = "hypergraph")]
        Value::Hyperedge(hid) => CylValue {
            tag: CylValueTag::Hyperedge as u8,
            payload: CylValuePayload { node_id: hid.0 },
        },
        #[cfg(feature = "hypergraph")]
        Value::TemporalNode(nid, ts) => CylValue {
            tag: CylValueTag::TemporalNode as u8,
            payload: CylValuePayload {
                temporal_node: CylTemporalNode {
                    node_id: nid.0,
                    timestamp_ms: *ts,
                },
            },
        },
    }
}

// ---------------------------------------------------------------------------
// FFI parameter constructors
// ---------------------------------------------------------------------------

/// Create a null CylValue parameter.
#[no_mangle]
pub extern "C" fn cyl_param_null() -> CylValue {
    CylValue::null()
}

/// Create a boolean CylValue parameter.
#[no_mangle]
pub extern "C" fn cyl_param_bool(value: bool) -> CylValue {
    CylValue::from_bool(value)
}

/// Create an integer CylValue parameter.
#[no_mangle]
pub extern "C" fn cyl_param_int64(value: i64) -> CylValue {
    CylValue::from_int64(value)
}

/// Create a floating-point CylValue parameter.
#[no_mangle]
pub extern "C" fn cyl_param_float64(value: f64) -> CylValue {
    CylValue::from_float64(value)
}

/// Create a string CylValue parameter.
///
/// The `value` pointer is stored directly -- the caller must keep the C
/// string alive until the parameter is consumed by `cyl_db_execute_with_params`.
///
/// # Safety
///
/// - `value` must be a valid, null-terminated C string (or NULL for Null).
#[no_mangle]
pub unsafe extern "C" fn cyl_param_string(value: *const libc::c_char) -> CylValue {
    if value.is_null() {
        return CylValue::null();
    }
    CylValue {
        tag: CylValueTag::String as u8,
        payload: CylValuePayload { string: value },
    }
}

/// Create a bytes CylValue parameter.
///
/// The data pointer is stored directly -- the caller must keep the buffer
/// alive until the parameter is consumed.
///
/// # Safety
///
/// - `data` must point to at least `len` bytes (or be NULL for empty).
#[no_mangle]
pub unsafe extern "C" fn cyl_param_bytes(data: *const u8, len: u32) -> CylValue {
    CylValue {
        tag: CylValueTag::Bytes as u8,
        payload: CylValuePayload {
            bytes: CylBytes { data, len },
        },
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CString;

    // -- CylValueTag tests -------------------------------------------------

    #[test]
    fn test_tag_null_is_zero() {
        assert_eq!(CylValueTag::Null as u8, 0);
    }

    #[test]
    fn test_tags_are_distinct() {
        let tags = [
            CylValueTag::Null as u8,
            CylValueTag::Bool as u8,
            CylValueTag::Int64 as u8,
            CylValueTag::Float64 as u8,
            CylValueTag::String as u8,
            CylValueTag::Bytes as u8,
            CylValueTag::List as u8,
            CylValueTag::Node as u8,
            CylValueTag::Edge as u8,
            CylValueTag::DateTime as u8,
        ];
        let mut sorted = tags.to_vec();
        sorted.sort();
        sorted.dedup();
        assert_eq!(sorted.len(), tags.len());
    }

    // -- CylValue constructors ---------------------------------------------

    #[test]
    fn test_cyl_value_null() {
        let v = CylValue::null();
        assert_eq!(v.tag, CylValueTag::Null as u8);
    }

    #[test]
    fn test_cyl_value_bool_true() {
        let v = CylValue::from_bool(true);
        assert_eq!(v.tag, CylValueTag::Bool as u8);
        assert!(unsafe { v.payload.boolean });
    }

    #[test]
    fn test_cyl_value_bool_false() {
        let v = CylValue::from_bool(false);
        assert!(!unsafe { v.payload.boolean });
    }

    #[test]
    fn test_cyl_value_int64() {
        let v = CylValue::from_int64(42);
        assert_eq!(v.tag, CylValueTag::Int64 as u8);
        assert_eq!(unsafe { v.payload.int64 }, 42);
    }

    #[test]
    fn test_cyl_value_float64() {
        let v = CylValue::from_float64(3.15);
        assert_eq!(v.tag, CylValueTag::Float64 as u8);
        assert!((unsafe { v.payload.float64 } - 3.15).abs() < f64::EPSILON);
    }

    // -- cyl_value_to_rust tests -------------------------------------------

    #[test]
    fn test_cyl_value_to_rust_null() {
        let cv = CylValue::null();
        assert_eq!(cyl_value_to_rust(&cv), Value::Null);
    }

    #[test]
    fn test_cyl_value_to_rust_bool() {
        let cv = CylValue::from_bool(true);
        assert_eq!(cyl_value_to_rust(&cv), Value::Bool(true));
    }

    #[test]
    fn test_cyl_value_to_rust_int64() {
        let cv = CylValue::from_int64(-100);
        assert_eq!(cyl_value_to_rust(&cv), Value::Int64(-100));
    }

    #[test]
    fn test_cyl_value_to_rust_float64() {
        let cv = CylValue::from_float64(2.719);
        match cyl_value_to_rust(&cv) {
            Value::Float64(f) => assert!((f - 2.719).abs() < f64::EPSILON),
            other => panic!("expected Float64, got {other:?}"),
        }
    }

    #[test]
    fn test_cyl_value_to_rust_string() {
        let s = CString::new("hello").unwrap();
        let cv = CylValue {
            tag: CylValueTag::String as u8,
            payload: CylValuePayload { string: s.as_ptr() },
        };
        assert_eq!(cyl_value_to_rust(&cv), Value::String("hello".into()));
    }

    #[test]
    fn test_cyl_value_to_rust_string_null_ptr() {
        let cv = CylValue {
            tag: CylValueTag::String as u8,
            payload: CylValuePayload {
                string: std::ptr::null(),
            },
        };
        assert_eq!(cyl_value_to_rust(&cv), Value::Null);
    }

    #[test]
    fn test_cyl_value_to_rust_bytes() {
        let data: Vec<u8> = vec![1, 2, 3];
        let cv = CylValue {
            tag: CylValueTag::Bytes as u8,
            payload: CylValuePayload {
                bytes: CylBytes {
                    data: data.as_ptr(),
                    len: 3,
                },
            },
        };
        assert_eq!(cyl_value_to_rust(&cv), Value::Bytes(vec![1, 2, 3]));
    }

    #[test]
    fn test_cyl_value_to_rust_unknown_tag_returns_null() {
        let cv = CylValue {
            tag: 255,
            payload: CylValuePayload { int64: 0 },
        };
        assert_eq!(cyl_value_to_rust(&cv), Value::Null);
    }

    // -- rust_value_to_cyl tests -------------------------------------------

    #[test]
    fn test_rust_to_cyl_null() {
        let cv = rust_value_to_cyl(&Value::Null);
        assert_eq!(cv.tag, CylValueTag::Null as u8);
    }

    #[test]
    fn test_rust_to_cyl_bool() {
        let cv = rust_value_to_cyl(&Value::Bool(true));
        assert_eq!(cv.tag, CylValueTag::Bool as u8);
        assert!(unsafe { cv.payload.boolean });
    }

    #[test]
    fn test_rust_to_cyl_int64() {
        let cv = rust_value_to_cyl(&Value::Int64(99));
        assert_eq!(cv.tag, CylValueTag::Int64 as u8);
        assert_eq!(unsafe { cv.payload.int64 }, 99);
    }

    #[test]
    fn test_rust_to_cyl_float64() {
        let cv = rust_value_to_cyl(&Value::Float64(1.5));
        assert_eq!(cv.tag, CylValueTag::Float64 as u8);
        assert!((unsafe { cv.payload.float64 } - 1.5).abs() < f64::EPSILON);
    }

    #[test]
    fn test_rust_to_cyl_string() {
        let val = Value::String("test".into());
        let cv = rust_value_to_cyl(&val);
        assert_eq!(cv.tag, CylValueTag::String as u8);
        // The pointer borrows from the Rust String, so it is valid while val
        // is alive. Note: Rust String is not null-terminated, so this pointer
        // should be used with the known length. For full C compatibility,
        // result.rs will provide CString-backed access.
        assert!(!unsafe { cv.payload.string }.is_null());
    }

    #[test]
    fn test_rust_to_cyl_node() {
        let cv = rust_value_to_cyl(&Value::Node(cypherlite_core::NodeId(7)));
        assert_eq!(cv.tag, CylValueTag::Node as u8);
        assert_eq!(unsafe { cv.payload.node_id }, 7);
    }

    #[test]
    fn test_rust_to_cyl_edge() {
        let cv = rust_value_to_cyl(&Value::Edge(cypherlite_core::EdgeId(3)));
        assert_eq!(cv.tag, CylValueTag::Edge as u8);
        assert_eq!(unsafe { cv.payload.edge_id }, 3);
    }

    #[test]
    fn test_rust_to_cyl_datetime() {
        let cv = rust_value_to_cyl(&Value::DateTime(1700000000000));
        assert_eq!(cv.tag, CylValueTag::DateTime as u8);
        assert_eq!(unsafe { cv.payload.int64 }, 1700000000000);
    }

    #[test]
    fn test_rust_to_cyl_bytes() {
        let val = Value::Bytes(vec![10, 20, 30]);
        let cv = rust_value_to_cyl(&val);
        assert_eq!(cv.tag, CylValueTag::Bytes as u8);
        let b = unsafe { cv.payload.bytes };
        assert_eq!(b.len, 3);
        assert!(!b.data.is_null());
    }

    #[test]
    fn test_rust_to_cyl_list() {
        let val = Value::List(vec![Value::Int64(1), Value::Int64(2)]);
        let cv = rust_value_to_cyl(&val);
        assert_eq!(cv.tag, CylValueTag::List as u8);
        let l = unsafe { cv.payload.list };
        assert_eq!(l.len, 2);
        // items is null because we don't heap-allocate CylValue arrays here.
        assert!(l.items.is_null());
    }

    // -- FFI param constructors --------------------------------------------

    #[test]
    fn test_cyl_param_null_ffi() {
        let v = cyl_param_null();
        assert_eq!(v.tag, CylValueTag::Null as u8);
    }

    #[test]
    fn test_cyl_param_bool_ffi() {
        let v = cyl_param_bool(true);
        assert_eq!(v.tag, CylValueTag::Bool as u8);
    }

    #[test]
    fn test_cyl_param_int64_ffi() {
        let v = cyl_param_int64(42);
        assert_eq!(cyl_value_to_rust(&v), Value::Int64(42));
    }

    #[test]
    fn test_cyl_param_float64_ffi() {
        let v = cyl_param_float64(3.15);
        match cyl_value_to_rust(&v) {
            Value::Float64(f) => assert!((f - 3.15).abs() < f64::EPSILON),
            other => panic!("expected Float64, got {other:?}"),
        }
    }

    #[test]
    fn test_cyl_param_string_ffi() {
        let s = CString::new("world").unwrap();
        // SAFETY: s is valid.
        let v = unsafe { cyl_param_string(s.as_ptr()) };
        assert_eq!(cyl_value_to_rust(&v), Value::String("world".into()));
    }

    #[test]
    fn test_cyl_param_string_null_returns_null_value() {
        // SAFETY: null is the test case.
        let v = unsafe { cyl_param_string(std::ptr::null()) };
        assert_eq!(v.tag, CylValueTag::Null as u8);
    }

    #[test]
    fn test_cyl_param_bytes_ffi() {
        let data: Vec<u8> = vec![0xDE, 0xAD];
        // SAFETY: data is valid.
        let v = unsafe { cyl_param_bytes(data.as_ptr(), 2) };
        assert_eq!(cyl_value_to_rust(&v), Value::Bytes(vec![0xDE, 0xAD]));
    }

    // -- Feature-gated tests -----------------------------------------------

    #[cfg(feature = "subgraph")]
    #[test]
    fn test_rust_to_cyl_subgraph() {
        let cv = rust_value_to_cyl(&Value::Subgraph(cypherlite_core::SubgraphId(5)));
        assert_eq!(cv.tag, CylValueTag::Subgraph as u8);
        assert_eq!(unsafe { cv.payload.node_id }, 5);
    }

    #[cfg(feature = "hypergraph")]
    #[test]
    fn test_rust_to_cyl_hyperedge() {
        let cv = rust_value_to_cyl(&Value::Hyperedge(cypherlite_core::HyperEdgeId(8)));
        assert_eq!(cv.tag, CylValueTag::Hyperedge as u8);
        assert_eq!(unsafe { cv.payload.node_id }, 8);
    }

    #[cfg(feature = "hypergraph")]
    #[test]
    fn test_rust_to_cyl_temporal_node() {
        let cv = rust_value_to_cyl(&Value::TemporalNode(cypherlite_core::NodeId(4), 1234567890));
        assert_eq!(cv.tag, CylValueTag::TemporalNode as u8);
        let tn = unsafe { cv.payload.temporal_node };
        assert_eq!(tn.node_id, 4);
        assert_eq!(tn.timestamp_ms, 1234567890);
    }
}
