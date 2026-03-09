# CypherLite

A lightweight, embedded, single-file graph database engine written in Rust.

CypherLite brings SQLite-like simplicity to the graph database ecosystem — zero-config, single-file deployment with full ACID compliance and native property graph support.

## Status

**Phase 1 (Storage Engine): Complete** — SPEC-DB-001 implemented and tested.

| Phase | Status | Description |
|-------|--------|-------------|
| Phase 1: Storage Engine | Complete (v0.1.0) | WAL, B+Tree, Buffer Pool, Transaction Manager |
| Phase 2: Query Engine | Planned | Cypher parsing, logical/physical planning |
| Phase 3: Plugin System | Planned | Extensibility, vector index, full-text search |

## Features (Phase 1)

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
cypherlite-storage = { path = "crates/cypherlite-storage" }
cypherlite-core = { path = "crates/cypherlite-core" }
```

Basic usage:

```rust
use cypherlite_storage::StorageEngine;
use cypherlite_core::{Config, PropertyValue};

// Open or create a database
let config = Config::new("my_graph.cyl");
let mut engine = StorageEngine::open(config)?;

// Create a node with properties
let alice = engine.create_node(
    vec![1], // label IDs
    vec![
        (0, PropertyValue::String("Alice".into())),
        (1, PropertyValue::Int64(30)),
    ],
)?;

// Create another node and an edge
let bob = engine.create_node(vec![1], vec![
    (0, PropertyValue::String("Bob".into())),
])?;
engine.create_edge(alice, bob, 1, vec![])?; // rel_type_id=1 (KNOWS)

// Read back
let node = engine.get_node(alice)?;
```

## Architecture

```
CypherLite/
├── crates/
│   ├── cypherlite-core/        # Core types: NodeId, EdgeId, PropertyValue, Error, Config, Traits
│   └── cypherlite-storage/     # Storage engine implementation
│       ├── src/
│       │   ├── lib.rs          # StorageEngine public API
│       │   ├── page/           # Page format, Buffer Pool, Page Manager
│       │   ├── btree/          # B+Tree, Node Store, Edge Store, Property Store
│       │   ├── wal/            # Write-Ahead Log, Checkpoint, Recovery
│       │   └── transaction/    # MVCC Transaction Manager
│       └── tests/              # Integration tests (ACID, concurrency, CRUD)
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

**Test coverage**: 96.82% (207 tests)

## TRUST 5 Quality Gates

| Dimension | Status | Details |
|-----------|--------|---------|
| Tested | Pass | 207 tests, 96.82% coverage |
| Readable | Pass | English comments, conventional naming |
| Unified | Pass | Consistent Rust style, rustfmt |
| Secured | Pass | No unsafe without justification, OWASP compliant |
| Trackable | Pass | Conventional commits, SPEC-DB-001 referenced |

## Design Documents

Comprehensive design documentation is available in [`docs/`](docs/):

- [`docs/00_master_overview.md`](docs/00_master_overview.md) — Executive summary and full roadmap
- [`docs/design/01_core_architecture.md`](docs/design/01_core_architecture.md) — System architecture
- [`docs/design/02_storage_engine.md`](docs/design/02_storage_engine.md) — Storage layer specification
- [`docs/design/03_query_engine.md`](docs/design/03_query_engine.md) — Query engine design (Phase 2)
- [`docs/design/04_plugin_architecture.md`](docs/design/04_plugin_architecture.md) — Plugin system (Phase 3)

## SPEC

Implementation specification: [`.moai/specs/SPEC-DB-001/spec.md`](.moai/specs/SPEC-DB-001/spec.md)

## License

TBD
