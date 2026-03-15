// Value conversion between Rust executor::Value and Python objects.

use crate::types::{EdgeID, NodeID};
use cypherlite_query::executor::Value;
use pyo3::exceptions::PyTypeError;
use pyo3::prelude::*;
use pyo3::types::{PyBool, PyBytes, PyFloat, PyInt, PyList, PyString};
#[cfg(feature = "hypergraph")]
use pyo3::types::PyDict;

/// Convert a Rust Value into a Python object.
pub fn rust_to_python(py: Python<'_>, val: &Value) -> PyObject {
    match val {
        Value::Null => py.None(),
        Value::Bool(b) => b.into_pyobject(py).expect("bool").to_owned().into_any().unbind(),
        Value::Int64(i) => i.into_pyobject(py).expect("int").into_any().unbind(),
        Value::Float64(f) => f.into_pyobject(py).expect("float").into_any().unbind(),
        Value::String(s) => s.into_pyobject(py).expect("str").into_any().unbind(),
        Value::Bytes(b) => PyBytes::new(py, b).into_any().unbind(),
        Value::List(items) => {
            let py_items: Vec<PyObject> = items.iter().map(|v| rust_to_python(py, v)).collect();
            PyList::new(py, &py_items)
                .expect("list")
                .into_any()
                .unbind()
        }
        Value::Node(id) => {
            let node_id = NodeID { id: id.0 };
            node_id
                .into_pyobject(py)
                .expect("NodeID")
                .into_any()
                .unbind()
        }
        Value::Edge(id) => {
            let edge_id = EdgeID { id: id.0 };
            edge_id
                .into_pyobject(py)
                .expect("EdgeID")
                .into_any()
                .unbind()
        }
        Value::DateTime(ms) => {
            // Return as plain integer (milliseconds since epoch).
            ms.into_pyobject(py).expect("datetime").into_any().unbind()
        }
        #[cfg(feature = "subgraph")]
        Value::Subgraph(id) => id.0.into_pyobject(py).expect("subgraph").into_any().unbind(),
        #[cfg(feature = "hypergraph")]
        Value::Hyperedge(id) => id.0.into_pyobject(py).expect("hyperedge").into_any().unbind(),
        #[cfg(feature = "hypergraph")]
        Value::TemporalNode(id, ms) => {
            let dict = PyDict::new(py);
            dict.set_item("node_id", id.0).expect("set node_id");
            dict.set_item("timestamp", ms).expect("set timestamp");
            dict.into_any().unbind()
        }
    }
}

/// Convert a Python object into a Rust Value.
pub fn python_to_rust(obj: &Bound<'_, pyo3::PyAny>) -> PyResult<Value> {
    // Order matters: check bool before int (bool is subclass of int in Python).
    if obj.is_none() {
        return Ok(Value::Null);
    }
    if obj.is_instance_of::<PyBool>() {
        return Ok(Value::Bool(obj.extract::<bool>()?));
    }
    if obj.is_instance_of::<PyInt>() {
        return Ok(Value::Int64(obj.extract::<i64>()?));
    }
    if obj.is_instance_of::<PyFloat>() {
        return Ok(Value::Float64(obj.extract::<f64>()?));
    }
    if obj.is_instance_of::<PyString>() {
        return Ok(Value::String(obj.extract::<String>()?));
    }
    if obj.is_instance_of::<PyBytes>() {
        return Ok(Value::Bytes(obj.extract::<Vec<u8>>()?));
    }
    if obj.is_instance_of::<PyList>() {
        let list = obj.downcast::<PyList>()?;
        let mut items = Vec::with_capacity(list.len());
        for item in list.iter() {
            items.push(python_to_rust(&item)?);
        }
        return Ok(Value::List(items));
    }
    if let Ok(node) = obj.extract::<NodeID>() {
        return Ok(Value::Node(cypherlite_core::NodeId(node.id)));
    }
    if let Ok(edge) = obj.extract::<EdgeID>() {
        return Ok(Value::Edge(cypherlite_core::EdgeId(edge.id)));
    }

    Err(PyTypeError::new_err(format!(
        "cannot convert {} to CypherLite value",
        obj.get_type().name()?
    )))
}

// Rust-level unit tests for value conversion are tested via pytest since the
// PyO3 abi3 extension module cannot be linked into a standalone test binary.
// See tests/test_cypherlite.py for comprehensive value round-trip tests.
