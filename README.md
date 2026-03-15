<p align="center">
  <img src="assets/logo.png" alt="CypherLite Logo" width="180">
</p>

<h1 align="center">CypherLite</h1>

<p align="center">
  <img src="https://github.com/Epsilondelta-ai/CypherLite/actions/workflows/ci.yml/badge.svg" alt="CI">
  <a href="https://crates.io/crates/cypherlite-query"><img src="https://img.shields.io/crates/v/cypherlite-query.svg" alt="crates.io"></a>
  <a href="https://docs.rs/cypherlite-query"><img src="https://docs.rs/cypherlite-query/badge.svg" alt="docs.rs"></a>
  <img src="https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg" alt="License">
  <img src="https://img.shields.io/badge/MSRV-1.84-orange.svg" alt="MSRV">
</p>

<p align="center"><em>SQLite-like simplicity for graph databases.</em></p>

```
        (\-.
        / _`>  CypherLite
       / /
      / /      Lightweight Embedded
     / /       Graph Database
    / /
   (,/
    ``
```

A lightweight, embedded, single-file graph database engine written in Rust. CypherLite brings zero-config, single-file deployment to the graph database ecosystem — with full ACID compliance, native property graph support, temporal queries, subgraph entities, hyperedges, and a trait-based plugin system.

**Available in**: [中文](docs/i18n/README.zh.md) | [हिन्दी](docs/i18n/README.hi.md) | [Español](docs/i18n/README.es.md) | [Français](docs/i18n/README.fr.md) | [العربية](docs/i18n/README.ar.md) | [বাংলা](docs/i18n/README.bn.md) | [Português](docs/i18n/README.pt.md) | [Русский](docs/i18n/README.ru.md) | [한국어](docs/i18n/README.ko.md)

---

## Features

### Storage Engine

- **ACID Transactions** — Full atomicity, consistency, isolation, and durability via Write-Ahead Logging
- **Single-Writer / Multiple-Reader** — SQLite-compatible concurrency model using `parking_lot`
- **Snapshot Isolation** — WAL frame index-based MVCC for consistent reads
- **B+Tree Storage** — O(log n) node and edge lookup with index-free adjacency
- **Crash Recovery** — WAL replay on startup for consistency after unexpected shutdown
- **Property Graph** — Nodes and edges with typed properties: `Null`, `Bool`, `Int64`, `Float64`, `String`, `Bytes`, `Array`
- **Embedded Library** — No server process, zero configuration, single `.cyl` file

### Query Engine (Cypher)

- **openCypher Subset** — `MATCH`, `CREATE`, `MERGE`, `SET`, `DELETE`, `RETURN`, `WHERE`, `WITH`, `ORDER BY`
- **Recursive Descent Parser** — Hand-written parser with Pratt expression parsing and 28+ keywords
- **Semantic Analysis** — Variable scope validation and label/type resolution
- **Cost-Based Optimizer** — Logical-to-physical plan conversion with predicate pushdown
- **Volcano Executor** — Iterator-based execution with 12 operators
- **Three-Valued Logic** — Full NULL propagation per openCypher specification
- **Inline Property Filter** — `MATCH (n:Label {key: value})` pattern support

### Temporal Features

- **AT TIME Queries** — Point-in-time graph state retrieval
- **Version Store** — Immutable property version chain per node and edge
- **Temporal Edge Versioning** — Edge creation/deletion timestamps with temporal relationship queries
- **Temporal Aggregation** — Time-range queries with aggregate functions over versioned data

### Subgraph & Hyperedge

- **SubgraphStore** — Named subgraphs as first-class entities stored alongside nodes and edges
- **CREATE / MATCH SNAPSHOT** — Capture and query named subgraph entities
- **Native Hyperedges** — N:M relations connecting arbitrary numbers of nodes
- **HYPEREDGE Syntax** — `CREATE HYPEREDGE :TYPE CONNECTING (n1), (n2), (n3)`
- **TemporalRef** — Hyperedge members carry temporal reference metadata

### Plugin System

- **ScalarFunction** — Register custom query functions callable in Cypher expressions
- **IndexPlugin** — Pluggable custom index implementations (e.g., HNSW vector index)
- **Serializer** — Custom import/export format plugins (e.g., JSON-LD, GraphML)
- **Trigger** — Before/after hooks for `CREATE`, `DELETE`, `SET` operations with rollback support
- **PluginRegistry** — Generic, thread-safe `HashMap`-based registry (`Send + Sync`)
- **Zero Overhead** — `plugin` feature flag; cfg-gated, no cost when disabled

### FFI Bindings

- **C ABI** — Static library with a C header for embedding in any C-compatible project
- **Python** — PyO3-based bindings via `pip install cypherlite`
- **Go** — CGo bindings via `go get github.com/Epsilondelta-ai/CypherLite/bindings/go/cypherlite`
- **Node.js** — napi-rs native addon via `npm install cypherlite`

---

## Quick Start

### Rust

```toml
# Cargo.toml
[dependencies]
cypherlite-query = "1.2"
```

```rust
use cypherlite_query::CypherLite;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let db = CypherLite::open("my_graph.cyl")?;

    // Create nodes and a relationship
    db.execute("CREATE (a:Person {name: 'Alice', age: 30})")?;
    db.execute("CREATE (b:Person {name: 'Bob', age: 25})")?;
    db.execute(
        "MATCH (a:Person {name: 'Alice'}), (b:Person {name: 'Bob'}) \
         CREATE (a)-[:KNOWS {since: 2023}]->(b)",
    )?;

    // Query the graph
    let result = db.execute("MATCH (p:Person) WHERE p.age > 20 RETURN p.name, p.age")?;
    for row in result {
        let row = row?;
        println!("{}: {}", row.get("p.name").unwrap(), row.get("p.age").unwrap());
    }
    Ok(())
}
```

### Python

```bash
pip install cypherlite
```

```python
import cypherlite

db = cypherlite.open("my_graph.cyl")

db.execute("CREATE (a:Person {name: 'Alice', age: 30})")
db.execute("CREATE (b:Person {name: 'Bob', age: 25})")
db.execute(
    "MATCH (a:Person {name: 'Alice'}), (b:Person {name: 'Bob'}) "
    "CREATE (a)-[:KNOWS {since: 2023}]->(b)"
)

result = db.execute("MATCH (n:Person) RETURN n.name, n.age")
for row in result:
    print(f"{row['n.name']} (age: {row['n.age']})")

db.close()
```

### Go

```bash
go get github.com/Epsilondelta-ai/CypherLite/bindings/go/cypherlite
```

```go
package main

import (
    "fmt"
    "github.com/Epsilondelta-ai/CypherLite/bindings/go/cypherlite"
)

func main() {
    db, _ := cypherlite.Open("my_graph.cyl")
    defer db.Close()

    db.Execute("CREATE (a:Person {name: 'Alice', age: 30})")
    db.Execute("CREATE (b:Person {name: 'Bob', age: 25})")

    result, _ := db.Execute("MATCH (n:Person) RETURN n.name, n.age")
    for result.Next() {
        row := result.Row()
        name, _ := row.GetString("n.name")
        age, _ := row.GetInt64("n.age")
        fmt.Printf("%s (age: %d)\n", name, age)
    }
}
```

### Node.js

```bash
npm install cypherlite
```

```js
const { open } = require("cypherlite");

const db = open("my_graph.cyl");

db.execute("CREATE (a:Person {name: 'Alice', age: 30})");
db.execute("CREATE (b:Person {name: 'Bob', age: 25})");

const result = db.execute("MATCH (n:Person) RETURN n.name, n.age");
for (const row of result) {
  console.log(`${row["n.name"]} (age: ${row["n.age"]})`);
}

db.close();
```

---

## Installation

### Rust (Cargo)

```toml
[dependencies]
cypherlite-query = "1.2"

# Optional: enable specific feature flags
# cypherlite-query = { version = "1.2", features = ["temporal-edge", "plugin"] }
```

### Python (pip)

```bash
pip install cypherlite
```

Build from source with full features:

```bash
cd crates/cypherlite-python
pip install maturin
maturin develop --release
```

### Go (go get)

```bash
go get github.com/Epsilondelta-ai/CypherLite/bindings/go/cypherlite
```

Requires: Go 1.21+, Rust toolchain (to build the C static library), and a C compiler for CGo.

### Node.js (npm)

```bash
npm install cypherlite
```

Build from source:

```bash
cd crates/cypherlite-node
npx napi build --release
```

Requires: Node.js 18+ with N-API v9 support and the Rust toolchain.

### C (header + static library)

```bash
cargo build -p cypherlite-ffi --release --all-features
```

Link `target/release/libcypherlite_ffi.a` and include the generated `cypherlite.h` header.

---

## Architecture

```
┌─────────────────────────────────────────┐
│           Application Layer             │
│  (user code: Rust, Python, Go, Node.js) │
├─────────────────────────────────────────┤
│     cypherlite-ffi / Bindings           │
│     (C ABI, PyO3, CGo, napi-rs)         │
├─────────────────────────────────────────┤
│         cypherlite-query                │
│  (Lexer → Parser → Planner → Executor)  │
├─────────────────────────────────────────┤
│        cypherlite-storage               │
│   (WAL, B+Tree, BufferPool, MVCC)       │
├─────────────────────────────────────────┤
│         cypherlite-core                 │
│   (Types, Traits, Error Handling)       │
└─────────────────────────────────────────┘
         ┊ plugin (orthogonal) ┊
```

**Crate dependency graph:**

```
cypherlite-query
    └── cypherlite-storage
            └── cypherlite-core

cypherlite-ffi
    └── cypherlite-query

cypherlite-python  (wraps cypherlite-ffi via PyO3)
cypherlite-node    (wraps cypherlite-ffi via napi-rs)
```

**Query execution pipeline:**

```
Cypher string
    → Lexer (logos tokenizer)
    → Parser (recursive descent + Pratt)
    → Semantic Analyzer (scope, labels)
    → Planner (logical → physical, cost-based)
    → Executor (Volcano iterator model)
    → QueryResult (iterable rows)
```

---

## Feature Flags

Feature flags are additive. Each flag enables the features of all flags listed above it in the table, except `plugin` which is independent.

| Flag | Default | Description |
|------|---------|-------------|
| `temporal-core` | Yes | Core temporal features (`AT TIME` queries, version store) |
| `temporal-edge` | No | Temporal edge versioning and temporal relationship queries |
| `subgraph` | No | Subgraph entities (`CREATE / MATCH SNAPSHOT`) |
| `hypergraph` | No | Native N:M hyperedges (`HYPEREDGE` syntax); implies `subgraph` |
| `full-temporal` | No | All temporal features combined |
| `plugin` | No | Plugin system — 4 plugin types, zero overhead when disabled |

Enable flags in `Cargo.toml`:

```toml
cypherlite-query = { version = "1.2", features = ["hypergraph", "plugin"] }
```

---

## Performance

Benchmarks run with Criterion on an Apple M2 (single-threaded, in-memory WAL flush disabled):

| Operation | Throughput |
|-----------|------------|
| Node INSERT | ~180,000 ops/sec |
| Node LOOKUP by ID | ~950,000 ops/sec |
| Edge INSERT | ~160,000 ops/sec |
| Simple MATCH query | ~120,000 queries/sec |
| WAL write throughput | ~450 MB/sec |

Run benchmarks locally:

```bash
cargo bench --workspace --all-features
```

---

## Testing

```bash
# All tests (default features)
cargo test --workspace

# All tests with all features
cargo test --workspace --all-features

# Coverage report
cargo llvm-cov --workspace --all-features --summary-only

# Linter (zero warnings enforced)
cargo clippy --workspace --all-targets --all-features -- -D warnings

# Benchmark smoke test
cargo bench --workspace --all-features -- --test
```

**Test suite**: ~1,490 tests across workspace, all features enabled, 0 clippy warnings, 85%+ coverage.

---

## Documentation

- **API Reference (docs.rs)**: [docs.rs/cypherlite-query](https://docs.rs/cypherlite-query)
- **Documentation Website**: [Epsilondelta-ai.github.io/CypherLite](https://Epsilondelta-ai.github.io/CypherLite)
- **Quick Start Examples**: [`examples/`](examples/) — Rust, Python, Go, and Node.js scripts
- **FFI Binding Examples**: [`bindings/`](bindings/) — Go package with full test coverage

---

## Contributing

Contributions are welcome. Please read [CONTRIBUTING.md](CONTRIBUTING.md) for:

- Bug reporting guidelines
- Branch naming and pull request process
- Development setup (Rust 1.84+)
- Code style: `cargo fmt`, `cargo clippy -- -D warnings`
- Test requirements: 85%+ coverage per commit

Open an [issue](https://github.com/Epsilondelta-ai/CypherLite/issues) first for significant changes to discuss the approach before implementation.

---

## License

Licensed under either of:

- [MIT License](LICENSE-MIT)
- [Apache License, Version 2.0](LICENSE-APACHE)

at your option.

---

## Status / Roadmap

| Phase | Version | Feature | Status |
|-------|---------|---------|--------|
| 1 | v0.1 | Storage Engine (WAL, B+Tree, ACID) | Complete |
| 2 | v0.2 | Query Engine (Cypher lexer, parser, executor) | Complete |
| 3 | v0.3 | Advanced Query (MERGE, WITH, ORDER BY, optimizer) | Complete |
| 4 | v0.4 | Temporal Core (AT TIME, version store) | Complete |
| 5 | v0.5 | Temporal Edge (edge versioning) | Complete |
| 6 | v0.6 | Subgraph Entities (SubgraphStore, SNAPSHOT) | Complete |
| 7 | v0.7 | Native Hyperedge (N:M, HYPEREDGE syntax) | Complete |
| 8 | v0.8 | Inline Property Filter (pattern fix) | Complete |
| 9 | v0.9 | CI/CD Pipeline (GitHub Actions, 6 jobs) | Complete |
| 10 | v1.0 | Plugin System (4 plugin types, registry) | Complete |
| 11 | v1.1 | Performance Optimization (benchmarks, buffer pool) | Complete |
| 12 | v1.1 | FFI Bindings (C, Python, Go, Node.js) | Complete |
| 13 | v1.2 | Documentation & i18n (rustdoc, website, examples) | **Current** |
