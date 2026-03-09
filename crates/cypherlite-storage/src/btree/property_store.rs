// PropertyStore: inline (<=31 bytes) and overflow page property management

use cypherlite_core::PropertyValue;

/// Maximum inline property value size in bytes.
/// REQ-STORE-009: key_id(u32) + type_tag(u8) + value (max 31 bytes inline).
pub const MAX_INLINE_SIZE: usize = 31;

/// Property serialization format: key_id(4) + type_tag(1) + value(variable).
/// Total inline: 4 + 1 + 31 = 36 bytes max per property slot.
pub const PROPERTY_SLOT_SIZE: usize = 36;

/// Manages property serialization and overflow detection.
///
/// REQ-STORE-009: Store property as key_id(u32) + type_tag(u8) + value (max 31 bytes inline).
/// REQ-STORE-010: Overflow to separate page when > 31 bytes.
/// REQ-STORE-011: Support all PropertyValue variants.
pub struct PropertyStore;

impl PropertyStore {
    /// Serialize a property value to bytes.
    /// Returns (type_tag, serialized_bytes).
    pub fn serialize_value(value: &PropertyValue) -> (u8, Vec<u8>) {
        match value {
            PropertyValue::Null => (0, vec![]),
            PropertyValue::Bool(b) => (1, vec![if *b { 1 } else { 0 }]),
            PropertyValue::Int64(n) => (2, n.to_le_bytes().to_vec()),
            PropertyValue::Float64(f) => (3, f.to_le_bytes().to_vec()),
            PropertyValue::String(s) => (4, s.as_bytes().to_vec()),
            PropertyValue::Bytes(b) => (5, b.clone()),
            PropertyValue::Array(arr) => {
                // Serialize array using bincode for simplicity
                let encoded = bincode::serialize(arr).unwrap_or_default();
                (6, encoded)
            }
        }
    }

    /// Deserialize a property value from bytes.
    pub fn deserialize_value(type_tag: u8, data: &[u8]) -> Option<PropertyValue> {
        match type_tag {
            0 => Some(PropertyValue::Null),
            1 => {
                if data.is_empty() {
                    return None;
                }
                Some(PropertyValue::Bool(data[0] != 0))
            }
            2 => {
                if data.len() < 8 {
                    return None;
                }
                let n = i64::from_le_bytes(data[..8].try_into().ok()?);
                Some(PropertyValue::Int64(n))
            }
            3 => {
                if data.len() < 8 {
                    return None;
                }
                let f = f64::from_le_bytes(data[..8].try_into().ok()?);
                Some(PropertyValue::Float64(f))
            }
            4 => {
                let s = std::str::from_utf8(data).ok()?;
                Some(PropertyValue::String(s.to_string()))
            }
            5 => Some(PropertyValue::Bytes(data.to_vec())),
            6 => {
                let arr: Vec<PropertyValue> = bincode::deserialize(data).ok()?;
                Some(PropertyValue::Array(arr))
            }
            _ => None,
        }
    }

    /// Check if a serialized value fits inline (<=31 bytes).
    /// REQ-STORE-010: > 31 bytes -> overflow.
    pub fn is_inline(value: &PropertyValue) -> bool {
        let (_, bytes) = Self::serialize_value(value);
        bytes.len() <= MAX_INLINE_SIZE
    }

    /// Serialize a full property entry: key_id + type_tag + value bytes.
    pub fn serialize_property(key_id: u32, value: &PropertyValue) -> Vec<u8> {
        let (type_tag, value_bytes) = Self::serialize_value(value);
        let mut buf = Vec::with_capacity(5 + value_bytes.len());
        buf.extend_from_slice(&key_id.to_le_bytes());
        buf.push(type_tag);
        buf.extend_from_slice(&value_bytes);
        buf
    }

    /// Deserialize a property entry from bytes.
    /// Returns (key_id, PropertyValue, bytes_consumed).
    pub fn deserialize_property(data: &[u8]) -> Option<(u32, PropertyValue, usize)> {
        if data.len() < 5 {
            return None;
        }
        let key_id = u32::from_le_bytes(data[0..4].try_into().ok()?);
        let type_tag = data[4];
        let value_data = &data[5..];
        let value = Self::deserialize_value(type_tag, value_data)?;
        let value_len = match &value {
            PropertyValue::Null => 0,
            PropertyValue::Bool(_) => 1,
            PropertyValue::Int64(_) | PropertyValue::Float64(_) => 8,
            PropertyValue::String(s) => s.len(),
            PropertyValue::Bytes(b) => b.len(),
            // Array is bincode-encoded; re-serialize only for this uncommon case.
            PropertyValue::Array(_) => Self::serialize_value(&value).1.len(),
        };
        Some((key_id, value, 5 + value_len))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // REQ-STORE-011: Null
    #[test]
    fn test_serialize_null() {
        let (tag, bytes) = PropertyStore::serialize_value(&PropertyValue::Null);
        assert_eq!(tag, 0);
        assert!(bytes.is_empty());
        let val = PropertyStore::deserialize_value(tag, &bytes).expect("deser");
        assert_eq!(val, PropertyValue::Null);
    }

    // REQ-STORE-011: Bool
    #[test]
    fn test_serialize_bool() {
        for b in [true, false] {
            let (tag, bytes) = PropertyStore::serialize_value(&PropertyValue::Bool(b));
            assert_eq!(tag, 1);
            let val = PropertyStore::deserialize_value(tag, &bytes).expect("deser");
            assert_eq!(val, PropertyValue::Bool(b));
        }
    }

    // REQ-STORE-011: Int64
    #[test]
    fn test_serialize_int64() {
        for n in [0i64, -1, i64::MIN, i64::MAX] {
            let (tag, bytes) = PropertyStore::serialize_value(&PropertyValue::Int64(n));
            assert_eq!(tag, 2);
            assert_eq!(bytes.len(), 8);
            let val = PropertyStore::deserialize_value(tag, &bytes).expect("deser");
            assert_eq!(val, PropertyValue::Int64(n));
        }
    }

    // REQ-STORE-011: Float64
    #[test]
    fn test_serialize_float64() {
        let (tag, bytes) = PropertyStore::serialize_value(&PropertyValue::Float64(3.14));
        assert_eq!(tag, 3);
        assert_eq!(bytes.len(), 8);
        let val = PropertyStore::deserialize_value(tag, &bytes).expect("deser");
        assert_eq!(val, PropertyValue::Float64(3.14));
    }

    // REQ-STORE-011: String
    #[test]
    fn test_serialize_string() {
        let (tag, bytes) = PropertyStore::serialize_value(&PropertyValue::String("hello".into()));
        assert_eq!(tag, 4);
        assert_eq!(bytes, b"hello");
        let val = PropertyStore::deserialize_value(tag, &bytes).expect("deser");
        assert_eq!(val, PropertyValue::String("hello".into()));
    }

    // REQ-STORE-011: Bytes
    #[test]
    fn test_serialize_bytes() {
        let data = vec![0xDE, 0xAD, 0xBE, 0xEF];
        let (tag, bytes) = PropertyStore::serialize_value(&PropertyValue::Bytes(data.clone()));
        assert_eq!(tag, 5);
        assert_eq!(bytes, data);
        let val = PropertyStore::deserialize_value(tag, &bytes).expect("deser");
        assert_eq!(val, PropertyValue::Bytes(data));
    }

    // REQ-STORE-011: Array
    #[test]
    fn test_serialize_array() {
        let arr = vec![PropertyValue::Int64(1), PropertyValue::Bool(true)];
        let (tag, bytes) = PropertyStore::serialize_value(&PropertyValue::Array(arr.clone()));
        assert_eq!(tag, 6);
        let val = PropertyStore::deserialize_value(tag, &bytes).expect("deser");
        assert_eq!(val, PropertyValue::Array(arr));
    }

    // REQ-STORE-009: Inline check (<=31 bytes)
    #[test]
    fn test_is_inline_small_values() {
        assert!(PropertyStore::is_inline(&PropertyValue::Null));
        assert!(PropertyStore::is_inline(&PropertyValue::Bool(true)));
        assert!(PropertyStore::is_inline(&PropertyValue::Int64(42)));
        assert!(PropertyStore::is_inline(&PropertyValue::Float64(3.14)));
        assert!(PropertyStore::is_inline(&PropertyValue::String(
            "short".into()
        )));
    }

    // REQ-STORE-010: Overflow for large values
    #[test]
    fn test_is_inline_large_string() {
        let long_string = "a".repeat(32); // > 31 bytes
        assert!(!PropertyStore::is_inline(&PropertyValue::String(
            long_string
        )));
    }

    #[test]
    fn test_is_inline_large_bytes() {
        let large = vec![0u8; 32]; // > 31 bytes
        assert!(!PropertyStore::is_inline(&PropertyValue::Bytes(large)));
    }

    #[test]
    fn test_serialize_property_entry() {
        let buf = PropertyStore::serialize_property(42, &PropertyValue::Int64(100));
        assert_eq!(buf.len(), 4 + 1 + 8); // key_id + tag + i64
        assert_eq!(u32::from_le_bytes(buf[0..4].try_into().unwrap()), 42);
        assert_eq!(buf[4], 2); // Int64 tag
    }

    #[test]
    fn test_deserialize_property_entry() {
        let buf = PropertyStore::serialize_property(7, &PropertyValue::Bool(true));
        let (key, val, consumed) = PropertyStore::deserialize_property(&buf).expect("deser");
        assert_eq!(key, 7);
        assert_eq!(val, PropertyValue::Bool(true));
        assert_eq!(consumed, buf.len());
    }

    #[test]
    fn test_deserialize_invalid_tag() {
        assert!(PropertyStore::deserialize_value(99, &[]).is_none());
    }

    #[test]
    fn test_deserialize_truncated_int64() {
        assert!(PropertyStore::deserialize_value(2, &[0, 0, 0]).is_none());
    }
}
