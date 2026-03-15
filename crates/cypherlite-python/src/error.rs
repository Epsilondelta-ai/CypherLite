// Error handling: convert CypherLiteError to Python exceptions.

use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;

pyo3::create_exception!(cypherlite._cypherlite, CypherLiteError, pyo3::exceptions::PyException);

/// Convert a Rust CypherLiteError into a Python CypherLiteError exception.
pub fn to_py_err(e: cypherlite_core::CypherLiteError) -> PyErr {
    CypherLiteError::new_err(e.to_string())
}

/// Helper for mutex poisoned errors.
pub fn mutex_poisoned() -> PyErr {
    PyRuntimeError::new_err("internal lock poisoned")
}
