# CypherLite

A lightweight, embedded, single-file graph database engine written in Rust.

CypherLite brings SQLite-like simplicity to the graph database ecosystem — zero-config, single-file deployment with full ACID compliance and native property graph support.

## Status

**Phase 2 (Query Engine): Complete** — SPEC-DB-002 implemented and tested.

| Phase | Status | Description |
|-------|--------|-------------|
| Phase 1: Storage Engine | Complete (v0.1.0) | WAL, B+Tree, Buffer Pool, Transaction Manager |
| Phase 2: Query Engine | Complete (v0.2.0) | Cypher lexer, parser, planner, executor |
| Phase 3: Advanced Features | Planned | MERGE, WITH, ORDER BY execution, indexing |

## Features

### Phase 2: Query Engine (Current)

- **Cypher Query Language**: openCypher subset with MATCH, CREATE, SET, DELETE, RETURN, WHERE
- **Recursive Descent Parser**: Hand-written parser with Pratt expression parsing, 28 keywords
- **Semantic Analysis**: Variable scope validation and label/type resolution
- **Query Planner**: Logical-to-physical plan conversion with predicate pushdown
- **Volcano Executor**: Iterator-based execution with 10 operators (NodeScan, Expand, Filter, Project, Create, Delete, Set, Aggregate, Limit, Sort)
- **Three-Valued Logic**: Full NULL propagation per openCypher spec
- **Type Coercion**: Automatic Int64/Float64 promotion in expressions

### Phase 1: Storage Engine

- **ACID Transactions**: Full atomicity, consistency, isolation, and durability via Write-Ahead Logging
- **Single-Writer Multiple-Reader**: SQLite-compatible concurrency model using `parking_lot`
- **Snapshot Isolation**: WAL frame index-based MVCC for consistent reads
- **B+Tree Storage**: O(log n) node and edge lookup with index-free adjacency
- **Crash Recovery**: WAL replay on startup for consistency after unexpected shutdown
- **Property Graph**: Nodes and edges with typed properties (Null, Bool, Int64, Float64, String, Bytes, Array)
- **Embedded Library**: No server process, zero configuration, single `.cyl` file

## Quick Start

Add CypherLite to your `Cargo.toml`:

```toml
[dependencies]
cypherlite-query = { path = "crates/cypherlite-query" }
```

Query with Cypher:

```rust
use cypherlite_query::CypherLite;

// Open or create a database
let db = CypherLite::open("my_graph.cyl")?;

// Create nodes and relationships
db.execute("CREATE (a:Person {name: 'Alice', age: 30})")?;
db.execute("CREATE (b:Person {name: 'Bob', age: 25})")?;
db.execute("MATCH (a:Person {name: 'Alice'}), (b:Person {name: 'Bob'}) CREATE (a)-[:KNOWS]->(b)")?;

// Query the graph
let result = db.execute("MATCH (p:Person) WHERE p.age > 20 RETURN p.name, p.age")?;
for row in result {
    let row = row?;
    println!("{}: {}", row.get("p.name").unwrap(), row.get("p.age").unwrap());
}
```

<details>
<summary>Low-level Storage API</summary>

```rust
use cypherlite_storage::StorageEngine;
use cypherlite_core::{Config, PropertyValue};

let config = Config::new("my_graph.cyl");
let mut engine = StorageEngine::open(config)?;

let alice = engine.create_node(
    vec![1],
    vec![
        (0, PropertyValue::String("Alice".into())),
        (1, PropertyValue::Int64(30)),
    ],
)?;

let bob = engine.create_node(vec![1], vec![
    (0, PropertyValue::String("Bob".into())),
])?;
engine.create_edge(alice, bob, 1, vec![])?;
```

</details>

## Architecture

```
CypherLite/
├── crates/
│   ├── cypherlite-core/        # Core types: NodeId, EdgeId, PropertyValue, Error, Config, Traits
│   ├── cypherlite-storage/     # Storage engine implementation
│   │   ├── src/
│   │   │   ├── lib.rs          # StorageEngine public API
│   │   │   ├── page/           # Page format, Buffer Pool, Page Manager
│   │   │   ├── btree/          # B+Tree, Node Store, Edge Store, Property Store
│   │   │   ├── wal/            # Write-Ahead Log, Checkpoint, Recovery
│   │   │   ├── catalog/        # Label/Type registry (BiMap String↔u32)
│   │   │   └── transaction/    # MVCC Transaction Manager
│   │   └── tests/              # Integration + property-based tests
│   └── cypherlite-query/       # Query engine (Phase 2)
│       ├── src/
│       │   ├── api/            # CypherLite, QueryResult, Row, Transaction
│       │   ├── lexer/          # logos-based tokenizer (28 keywords)
│       │   ├── parser/         # Recursive descent parser, Pratt expressions, AST
│       │   ├── semantic/       # Variable scope validation, symbol table
│       │   ├── planner/        # Logical/physical plan, optimization
│       │   └── executor/       # Volcano iterator model, 10 operators
│       └── tests/              # Integration + property-based tests
└── docs/                       # Architecture and design documentation
```

## File Format

- **`.cyl`**: Primary database file (4KB pages, B+Tree structure)
- **`.cyl-wal`**: Write-Ahead Log (frame-based, checksum-verified)
- **Magic Number**: `CYLT` (0x43594C54)
- **Page Size**: 4,096 bytes (fixed)

## Dependencies

| Crate | Version | Purpose |
|-------|---------|---------|
| `parking_lot` | 0.12 | RwLock, Mutex for concurrency |
| `dashmap` | 6 | Concurrent hash map for WAL index |
| `bincode` | 1 | Binary serialization |
| `thiserror` | 2 | Error type definitions |
| `crossbeam` | 0.8 | Channel and concurrency utilities |
| `logos` | 0.14 | Lexer generator for tokenization |

**MSRV**: Rust 1.84+

## Testing

```bash
# Run all tests
cargo test --workspace

# Run with coverage
cargo llvm-cov --workspace --summary-only

# Run linter
cargo clippy --workspace --all-targets -- -D warnings
```

**Test coverage**: 93.02% (570 tests across workspace)

## TRUST 5 Quality Gates

| Dimension | Status | Details |
|-----------|--------|---------|
| Tested | Pass | 570 tests, 93.02% coverage, proptest + criterion benchmarks |
| Readable | Pass | English comments, conventional naming, `#![warn(missing_docs)]` |
| Unified | Pass | Consistent Rust style, rustfmt, clippy clean |
| Secured | Pass | No unsafe, cargo audit clean, TSAN verified (0 data races) |
| Trackable | Pass | Conventional commits, SPEC-DB-001 + SPEC-DB-002 referenced |

## Design Documents

Comprehensive design documentation is available in [`docs/`](docs/):

- [`docs/00_master_overview.md`](docs/00_master_overview.md) — Executive summary and full roadmap
- [`docs/design/01_core_architecture.md`](docs/design/01_core_architecture.md) — System architecture
- [`docs/design/02_storage_engine.md`](docs/design/02_storage_engine.md) — Storage layer specification
- [`docs/design/03_query_engine.md`](docs/design/03_query_engine.md) — Query engine design (Phase 2)
- [`docs/design/04_plugin_architecture.md`](docs/design/04_plugin_architecture.md) — Plugin system (Phase 3)

## SPECs

| SPEC | Phase | Status |
|------|-------|--------|
| [SPEC-DB-001](.moai/specs/SPEC-DB-001/spec.md) | Phase 1: Storage Engine | Complete |
| [SPEC-DB-002](.moai/specs/SPEC-DB-002/spec.md) | Phase 2: Query Engine | Complete |

## License

TBD
