// Database lifecycle and query execution for Python.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;
use pyo3::types::PyDict;

use cypherlite_core::DatabaseConfig;
use cypherlite_query::api::CypherLite;
use cypherlite_query::executor::Value;

use crate::error::{mutex_poisoned, to_py_err};
use crate::result::PyResult_;
use crate::transaction::Transaction;
use crate::value::python_to_rust;

/// The main CypherLite database handle for Python.
#[pyclass]
pub struct Database {
    pub(crate) inner: Arc<Mutex<Option<CypherLite>>>,
    pub(crate) in_transaction: Arc<AtomicBool>,
}

#[pymethods]
impl Database {
    /// Execute a Cypher query string.
    ///
    /// Optional keyword argument `params` provides named parameters as a dict.
    #[pyo3(signature = (query, params = None))]
    fn execute(
        &self,
        py: Python<'_>,
        query: &str,
        params: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<PyResult_> {
        if self.in_transaction.load(Ordering::SeqCst) {
            return Err(PyRuntimeError::new_err(
                "cannot execute on database while a transaction is active",
            ));
        }
        let rust_params = convert_params(params)?;
        let query_owned = query.to_string();

        // Run DB operation outside the GIL
        let inner = Arc::clone(&self.inner);
        let qr = py.allow_threads(move || {
            let mut guard = inner.lock().map_err(|_| mutex_poisoned())?;
            let db = guard
                .as_mut()
                .ok_or_else(|| crate::error::CypherLiteError::new_err("database is closed"))?;
            if rust_params.is_empty() {
                db.execute(&query_owned).map_err(to_py_err)
            } else {
                db.execute_with_params(&query_owned, rust_params)
                    .map_err(to_py_err)
            }
        })?;

        // Convert result with GIL held
        Ok(PyResult_::from_query_result(py, qr))
    }

    /// Close the database. Safe to call multiple times.
    fn close(&self, py: Python<'_>) -> PyResult<()> {
        let inner = Arc::clone(&self.inner);
        py.allow_threads(move || {
            let mut guard = inner.lock().map_err(|_| mutex_poisoned())?;
            // Take ownership and drop the CypherLite instance.
            let _ = guard.take();
            Ok(())
        })
    }

    /// Begin a new transaction.
    fn begin(&self) -> PyResult<Transaction> {
        // Check database is open.
        {
            let guard = self.inner.lock().map_err(|_| mutex_poisoned())?;
            if guard.is_none() {
                return Err(crate::error::CypherLiteError::new_err("database is closed"));
            }
        }

        // Check no transaction is already active.
        if self
            .in_transaction
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return Err(PyRuntimeError::new_err("a transaction is already active"));
        }

        Ok(Transaction {
            inner: Arc::clone(&self.inner),
            in_transaction: Arc::clone(&self.in_transaction),
            finished: false,
        })
    }

    /// Context manager entry: return self.
    fn __enter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    /// Context manager exit: close the database.
    #[pyo3(signature = (_exc_type=None, _exc_val=None, _exc_tb=None))]
    fn __exit__(
        &self,
        py: Python<'_>,
        _exc_type: Option<&Bound<'_, pyo3::PyAny>>,
        _exc_val: Option<&Bound<'_, pyo3::PyAny>>,
        _exc_tb: Option<&Bound<'_, pyo3::PyAny>>,
    ) -> PyResult<bool> {
        self.close(py)?;
        Ok(false) // do not suppress exceptions
    }
}

/// Module-level function: open a CypherLite database.
#[pyfunction]
#[pyo3(signature = (path, page_size = None, cache_capacity = None))]
pub fn open(path: &str, page_size: Option<u32>, cache_capacity: Option<u32>) -> PyResult<Database> {
    let config = DatabaseConfig {
        path: std::path::PathBuf::from(path),
        page_size: page_size.unwrap_or(4096),
        cache_capacity: cache_capacity.unwrap_or(256) as usize,
        ..Default::default()
    };
    let db = CypherLite::open(config).map_err(to_py_err)?;
    Ok(Database {
        inner: Arc::new(Mutex::new(Some(db))),
        in_transaction: Arc::new(AtomicBool::new(false)),
    })
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
