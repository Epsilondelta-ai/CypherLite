// Transaction wrapper for Python with context manager support.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;
use pyo3::types::PyDict;

use cypherlite_query::api::CypherLite;
use cypherlite_query::executor::Value;

use crate::error::{mutex_poisoned, to_py_err};
use crate::result::PyResult_;
use crate::value::python_to_rust;

/// A transaction wrapping CypherLite execute calls.
///
/// Shares the database mutex with the parent Database object.
/// On context manager exit, auto-commits on clean exit and
/// auto-rollbacks on exception.
#[pyclass]
pub struct Transaction {
    pub(crate) inner: Arc<Mutex<Option<CypherLite>>>,
    pub(crate) in_transaction: Arc<AtomicBool>,
    pub(crate) finished: bool,
}

impl Transaction {
    /// Mark this transaction as finished and clear the in_transaction flag.
    fn finish(&mut self) {
        if !self.finished {
            self.finished = true;
            self.in_transaction.store(false, Ordering::SeqCst);
        }
    }
}

impl Drop for Transaction {
    fn drop(&mut self) {
        // Auto-rollback: just clear the flag.
        self.finish();
    }
}

#[pymethods]
impl Transaction {
    /// Execute a Cypher query within this transaction.
    #[pyo3(signature = (query, params = None))]
    fn execute(
        &mut self,
        py: Python<'_>,
        query: &str,
        params: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<PyResult_> {
        if self.finished {
            return Err(PyRuntimeError::new_err("transaction is already finished"));
        }
        let rust_params = convert_params(params)?;
        let query_owned = query.to_string();

        let inner = Arc::clone(&self.inner);
        let qr = py.allow_threads(move || {
            let mut guard = inner.lock().map_err(|_| mutex_poisoned())?;
            let db = guard
                .as_mut()
                .ok_or_else(|| PyRuntimeError::new_err("database is closed"))?;
            if rust_params.is_empty() {
                db.execute(&query_owned).map_err(to_py_err)
            } else {
                db.execute_with_params(&query_owned, rust_params)
                    .map_err(to_py_err)
            }
        })?;

        Ok(PyResult_::from_query_result(py, qr))
    }

    /// Commit the transaction.
    fn commit(&mut self) -> PyResult<()> {
        if self.finished {
            return Err(PyRuntimeError::new_err("transaction is already finished"));
        }
        self.finish();
        Ok(())
    }

    /// Rollback the transaction (Phase 2: no-op at storage level).
    fn rollback(&mut self) -> PyResult<()> {
        if self.finished {
            return Err(PyRuntimeError::new_err("transaction is already finished"));
        }
        self.finish();
        Ok(())
    }

    /// Context manager entry: return self.
    fn __enter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    /// Context manager exit: commit on clean exit, rollback on exception.
    #[pyo3(signature = (exc_type=None, _exc_val=None, _exc_tb=None))]
    fn __exit__(
        &mut self,
        exc_type: Option<&Bound<'_, pyo3::PyAny>>,
        _exc_val: Option<&Bound<'_, pyo3::PyAny>>,
        _exc_tb: Option<&Bound<'_, pyo3::PyAny>>,
    ) -> PyResult<bool> {
        if self.finished {
            return Ok(false);
        }
        if exc_type.is_some() {
            // Exception occurred: rollback.
            self.finish();
        } else {
            // Clean exit: commit.
            self.finish();
        }
        Ok(false) // do not suppress exceptions
    }
}

/// Convert an optional Python dict to Rust params HashMap.
fn convert_params(params: Option<&Bound<'_, PyDict>>) -> PyResult<HashMap<String, Value>> {
    let Some(dict) = params else {
        return Ok(HashMap::new());
    };
    let mut map = HashMap::with_capacity(dict.len());
    for (key, val) in dict.iter() {
        let k: String = key.extract()?;
        let v = python_to_rust(&val)?;
        map.insert(k, v);
    }
    Ok(map)
}
