// Value conversion between Rust executor::Value and JavaScript values.

use cypherlite_query::executor::Value;
use napi::{Env, JsObject, JsUnknown, ValueType};

/// Create a JS BigInt from a u64 entity ID.
fn u64_to_bigint(env: &Env, id: u64) -> napi::Result<JsUnknown> {
    // Node IDs are unsigned; create_bigint_from_words takes (sign, words).
    let (sign, words) = if (id as i64) < 0 {
        (true, vec![id])
    } else {
        (false, vec![id])
    };
    env.create_bigint_from_words(sign, words)?.into_unknown()
}

/// Convert a Rust Value to a JavaScript value.
pub fn rust_to_js(env: &Env, val: &Value) -> napi::Result<JsUnknown> {
    match val {
        Value::Null => Ok(env.get_null()?.into_unknown()),
        Value::Bool(b) => Ok(env.get_boolean(*b)?.into_unknown()),
        Value::Int64(i) => Ok(env.create_int64(*i)?.into_unknown()),
        Value::Float64(f) => Ok(env.create_double(*f)?.into_unknown()),
        Value::String(s) => Ok(env.create_string(s)?.into_unknown()),
        Value::Bytes(b) => {
            let buf = env.create_buffer_with_data(b.clone())?;
            Ok(buf.into_unknown())
        }
        Value::List(items) => {
            let mut arr = env.create_array_with_length(items.len())?;
            for (i, item) in items.iter().enumerate() {
                let js_val = rust_to_js(env, item)?;
                arr.set_element(i as u32, js_val)?;
            }
            Ok(arr.into_unknown())
        }
        Value::Node(id) => u64_to_bigint(env, id.0),
        Value::Edge(id) => u64_to_bigint(env, id.0),
        Value::DateTime(ms) => Ok(env.create_int64(*ms)?.into_unknown()),
        #[cfg(feature = "subgraph")]
        Value::Subgraph(id) => u64_to_bigint(env, id.0),
        #[cfg(feature = "hypergraph")]
        Value::Hyperedge(id) => u64_to_bigint(env, id.0),
        #[cfg(feature = "hypergraph")]
        Value::TemporalNode(id, ms) => {
            // Return as a plain object with nodeId and timestamp fields.
            let mut obj = env.create_object()?;
            let bigint = u64_to_bigint(env, id.0)?;
            obj.set_named_property("nodeId", bigint)?;
            obj.set_named_property("timestamp", env.create_int64(*ms)?)?;
            Ok(obj.into_unknown())
        }
    }
}

/// Convert a JavaScript value to a Rust Value.
#[allow(clippy::only_used_in_recursion)]
pub fn js_to_rust(env: &Env, val: JsUnknown) -> napi::Result<Value> {
    match val.get_type()? {
        ValueType::Null | ValueType::Undefined => Ok(Value::Null),
        ValueType::Boolean => {
            let b = val.coerce_to_bool()?.get_value()?;
            Ok(Value::Bool(b))
        }
        ValueType::Number => {
            let n = val.coerce_to_number()?;
            let f = n.get_double()?;
            // If the number is an integer (no fractional part and within i64 range),
            // store as Int64 for better compatibility with Cypher integer semantics.
            if f.fract() == 0.0 && f >= i64::MIN as f64 && f <= i64::MAX as f64 {
                Ok(Value::Int64(f as i64))
            } else {
                Ok(Value::Float64(f))
            }
        }
        ValueType::String => {
            let s = val.coerce_to_string()?.into_utf8()?;
            Ok(Value::String(s.as_str()?.to_string()))
        }
        ValueType::BigInt => {
            let mut bigint = unsafe { val.cast::<napi::JsBigInt>() };
            let (signed, value) = bigint.get_words()?;
            // Simple conversion: take the first word as u64, apply sign.
            let raw = value.first().copied().unwrap_or(0);
            let result = if signed { -(raw as i64) } else { raw as i64 };
            Ok(Value::Int64(result))
        }
        ValueType::Object => {
            // Check if it is an array.
            let obj: JsObject = unsafe { val.cast() };
            if obj.is_array()? {
                let len = obj.get_array_length()?;
                let mut items = Vec::with_capacity(len as usize);
                for i in 0..len {
                    let elem: JsUnknown = obj.get_element(i)?;
                    items.push(js_to_rust(env, elem)?);
                }
                return Ok(Value::List(items));
            }
            // Check if it is a Buffer.
            if obj.is_buffer()? {
                let unknown = obj.into_unknown();
                let buf = napi::JsBuffer::try_from(unknown)?;
                let data = buf.into_value()?;
                return Ok(Value::Bytes(data.to_vec()));
            }
            Err(napi::Error::from_reason(
                "cannot convert JS object to CypherLite value",
            ))
        }
        _ => Err(napi::Error::from_reason(format!(
            "cannot convert JS type {:?} to CypherLite value",
            val.get_type()?
        ))),
    }
}

/// Convert a JS object of params ({ key: value, ... }) to a HashMap.
pub fn convert_params(
    env: &Env,
    params: Option<JsObject>,
) -> napi::Result<std::collections::HashMap<String, Value>> {
    let Some(obj) = params else {
        return Ok(std::collections::HashMap::new());
    };
    let keys = obj.get_property_names()?;
    let len = keys.get_array_length()?;
    let mut map = std::collections::HashMap::with_capacity(len as usize);
    for i in 0..len {
        let key: napi::JsString = keys.get_element(i)?;
        let key_str = key.into_utf8()?.as_str()?.to_string();
        let val: JsUnknown = obj.get_named_property(&key_str)?;
        map.insert(key_str, js_to_rust(env, val)?);
    }
    Ok(map)
}
