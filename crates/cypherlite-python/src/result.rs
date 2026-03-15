// Query result wrapper for Python.

use std::sync::atomic::{AtomicUsize, Ordering};

use crate::value::rust_to_python;
use cypherlite_query::api::QueryResult;
use pyo3::exceptions::PyIndexError;
use pyo3::prelude::*;
use pyo3::types::PyDict;

/// A query result containing columns and rows.
///
/// Rows are eagerly converted to Python objects at creation time.
#[pyclass(name = "Result")]
pub struct PyResult_ {
    columns: Vec<String>,
    /// Each row is a Vec of PyObject in column order.
    rows: Vec<Vec<PyObject>>,
    /// Iterator state (AtomicUsize for Sync requirement).
    iter_index: AtomicUsize,
}

impl PyResult_ {
    /// Create from a Rust QueryResult, eagerly converting all values.
    pub fn from_query_result(py: Python<'_>, qr: QueryResult) -> Self {
        let columns = qr.columns;
        let rows: Vec<Vec<PyObject>> = qr
            .rows
            .iter()
            .map(|row| {
                columns
                    .iter()
                    .map(|col| {
                        let val = row.get(col);
                        match val {
                            Some(v) => rust_to_python(py, v),
                            None => py.None(),
                        }
                    })
                    .collect()
            })
            .collect();
        Self {
            columns,
            rows,
            iter_index: AtomicUsize::new(0),
        }
    }

    /// Build a Python dict for a single row.
    fn row_to_dict(&self, py: Python<'_>, row_idx: usize) -> PyResult<PyObject> {
        let dict = PyDict::new(py);
        let row = &self.rows[row_idx];
        for (i, col) in self.columns.iter().enumerate() {
            dict.set_item(col, &row[i])?;
        }
        Ok(dict.into_any().unbind())
    }
}

#[pymethods]
impl PyResult_ {
    /// Column names as a list of strings.
    #[getter]
    fn columns(&self) -> Vec<String> {
        self.columns.clone()
    }

    /// Number of rows.
    fn __len__(&self) -> usize {
        self.rows.len()
    }

    /// Get a row by index as a dict.
    fn __getitem__(&self, py: Python<'_>, index: isize) -> PyResult<PyObject> {
        let len = self.rows.len() as isize;
        let actual = if index < 0 { len + index } else { index };
        if actual < 0 || actual >= len {
            return Err(PyIndexError::new_err("row index out of range"));
        }
        self.row_to_dict(py, actual as usize)
    }

    /// Reset iterator and return self.
    fn __iter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf.iter_index.store(0, Ordering::Relaxed);
        slf
    }

    /// Yield the next row dict or raise StopIteration.
    fn __next__(&self, py: Python<'_>) -> PyResult<Option<PyObject>> {
        let idx = self.iter_index.fetch_add(1, Ordering::Relaxed);
        if idx >= self.rows.len() {
            return Ok(None);
        }
        self.row_to_dict(py, idx).map(Some)
    }
}
