// CypherLite Python Bindings via PyO3.
//
// This crate exposes the CypherLite embedded graph database as a native
// Python extension module built with maturin.

pub mod database;
pub mod error;
pub mod result;
pub mod transaction;
pub mod types;
pub mod value;

use pyo3::prelude::*;

/// Return the library version string.
#[pyfunction]
fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// Return a comma-separated string of compiled feature flags.
#[pyfunction]
fn features() -> String {
    let mut flags = Vec::new();
    if cfg!(feature = "temporal-core") {
        flags.push("temporal-core");
    }
    if cfg!(feature = "temporal-edge") {
        flags.push("temporal-edge");
    }
    if cfg!(feature = "subgraph") {
        flags.push("subgraph");
    }
    if cfg!(feature = "hypergraph") {
        flags.push("hypergraph");
    }
    if cfg!(feature = "full-temporal") {
        flags.push("full-temporal");
    }
    if cfg!(feature = "plugin") {
        flags.push("plugin");
    }
    flags.join(",")
}

/// The `_cypherlite` native extension module.
#[pymodule]
fn _cypherlite(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(database::open, m)?)?;
    m.add_function(wrap_pyfunction!(version, m)?)?;
    m.add_function(wrap_pyfunction!(features, m)?)?;
    m.add_class::<database::Database>()?;
    m.add_class::<result::PyResult_>()?;
    m.add_class::<transaction::Transaction>()?;
    m.add_class::<types::NodeID>()?;
    m.add_class::<types::EdgeID>()?;
    m.add(
        "CypherLiteError",
        m.py().get_type::<error::CypherLiteError>(),
    )?;
    Ok(())
}
