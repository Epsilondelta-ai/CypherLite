// CypherLite Node.js Bindings via napi-rs.
//
// This crate exposes the CypherLite embedded graph database as a native
// Node.js addon built with @napi-rs/cli.

#[macro_use]
extern crate napi_derive;

pub mod database;
pub mod error;
pub mod result;
pub mod transaction;
pub mod value;

/// Return the library version string.
#[napi]
pub fn version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

/// Return a comma-separated string of compiled feature flags.
#[napi]
pub fn features() -> String {
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
