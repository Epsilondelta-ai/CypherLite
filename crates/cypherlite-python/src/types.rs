// Python-visible NodeID and EdgeID wrapper types.

use pyo3::prelude::*;

/// A node identifier returned by CypherLite queries.
#[pyclass(frozen)]
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct NodeID {
    /// The raw numeric identifier.
    #[pyo3(get)]
    pub id: u64,
}

#[pymethods]
impl NodeID {
    #[new]
    pub fn new(id: u64) -> Self {
        Self { id }
    }

    fn __repr__(&self) -> String {
        format!("NodeID({})", self.id)
    }

    fn __eq__(&self, other: &Self) -> bool {
        self.id == other.id
    }

    fn __hash__(&self) -> u64 {
        self.id
    }
}

/// An edge identifier returned by CypherLite queries.
#[pyclass(frozen)]
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct EdgeID {
    /// The raw numeric identifier.
    #[pyo3(get)]
    pub id: u64,
}

#[pymethods]
impl EdgeID {
    #[new]
    pub fn new(id: u64) -> Self {
        Self { id }
    }

    fn __repr__(&self) -> String {
        format!("EdgeID({})", self.id)
    }

    fn __eq__(&self, other: &Self) -> bool {
        self.id == other.id
    }

    fn __hash__(&self) -> u64 {
        self.id
    }
}
