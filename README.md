# CypherLite

A lightweight, embedded, single-file graph database engine written in Rust.

CypherLite brings SQLite-like simplicity to the graph database ecosystem — zero-config, single-file deployment with full ACID compliance, native property graph support, temporal queries, subgraph entities, hyperedges, and a trait-based plugin system.

## Status

**v1.0.0 — All Phases Complete**

| Phase | Version | SPEC | Description | Status |
|-------|---------|------|-------------|--------|
| 1 | v0.1.0 | SPEC-DB-001 | Storage Engine (WAL, B+Tree, Buffer Pool, ACID) | Complete |
| 2 | v0.2.0 | SPEC-DB-002 | Query Engine (Cypher lexer, parser, planner, executor) | Complete |
| 3 | v0.3.0 | SPEC-DB-003 | Advanced Query (MERGE, WITH, ORDER BY, indexing, optimizer) | Complete |
| 4 | v0.4.0 | SPEC-DB-004 | Temporal Core (AT TIME, version store, temporal queries) | Complete |
| 5 | v0.5.0 | SPEC-DB-005 | Temporal Edge (edge versioning, temporal relationship queries) | Complete |
| 6 | v0.6.0 | SPEC-DB-006 | Subgraph Entities (SubgraphStore, CREATE/MATCH SNAPSHOT) | Complete |
| 7 | v0.7.0 | SPEC-DB-007 | Native Hyperedge (N:M relations, HYPEREDGE syntax, TemporalRef) | Complete |
| 8 | v0.8.0 | SPEC-DB-008 | Inline Property Filter (MATCH pattern {key: value} fix) | Complete |
| 9 | v0.9.0 | SPEC-INFRA-001 | CI/CD Pipeline (GitHub Actions, 6 parallel jobs) | Complete |
| 10 | v1.0.0 | SPEC-PLUGIN-001 | Plugin System (4 plugin types, registry, feature flag) | Complete |

## Features

### Storage Engine

- **ACID Transactions**: Full atomicity, consistency, isolation, and durability via Write-Ahead Logging
- **Single-Writer Multiple-Reader**: SQLite-compatible concurrency model using `parking_lot`
- **Snapshot Isolation**: WAL frame index-based MVCC for consistent reads
- **B+Tree Storage**: O(log n) node and edge lookup with index-free adjacency
- **Crash Recovery**: WAL replay on startup for consistency after unexpected shutdown
- **Property Graph**: Nodes and edges with typed properties (Null, Bool, Int64, Float64, String, Bytes, Array)
- **Embedded Library**: No server process, zero configuration, single `.cyl` file

### Query Engine

- **Cypher Query Language**: openCypher subset with MATCH, CREATE, MERGE, SET, DELETE, RETURN, WHERE, WITH, ORDER BY
- **Recursive Descent Parser**: Hand-written parser with Pratt expression parsing, 28+ keywords
- **Semantic Analysis**: Variable scope validation and label/type resolution
- **Query Planner**: Logical-to-physical plan conversion with cost-based optimizer and predicate pushdown
- **Volcano Executor**: Iterator-based execution with 12 operators (NodeScan, Expand, Filter, Project, Create, Delete, Set, Aggregate, Limit, Sort, Merge, With)
- **Three-Valued Logic**: Full NULL propagation per openCypher spec
- **Type Coercion**: Automatic Int64/Float64 promotion in expressions
- **Inline Property Filter**: `MATCH (n:Label {key: value})` syntax in pattern matching

### Temporal Features

- **AT TIME Queries**: Point-in-time graph state retrieval
- **Version Store**: Immutable property version chain per node/edge
- **Temporal Edge Versioning**: Edge creation/deletion timestamps with temporal relationship queries
- **Snapshot Isolation**: MVCC-based consistent temporal reads
- **Temporal Aggregation**: Time-range queries with aggregate functions over versioned data

### Subgraph Entities

- **SubgraphStore**: Named subgraphs as first-class entities stored alongside nodes and edges
- **CREATE SNAPSHOT**: Capture current graph state as a named subgraph
- **MATCH SNAPSHOT**: Query and retrieve named subgraph entities
- **MembershipIndex**: Efficient node/edge-to-subgraph membership lookup
- **Virtual :CONTAINS**: Virtual relationship type for subgraph membership queries

### Hyperedge Support

- **Native Hyperedges**: N:M relations connecting arbitrary numbers of nodes
- **HYPEREDGE Syntax**: `CREATE HYPEREDGE :TYPE CONNECTING (n1), (n2), (n3)` DDL
- **TemporalRef**: Hyperedge members carry temporal reference metadata
- **:INVOLVES Virtual Relation**: Query hyperedge participation via virtual relationship type
- **SubgraphScan Operator**: Dedicated executor operator for hyperedge and subgraph traversal

### Plugin System

- **ScalarFunction**: Register custom query functions callable in Cypher expressions
- **IndexPlugin**: Pluggable custom index implementations (e.g., HNSW vector index)
- **Serializer**: Custom import/export format plugins (e.g., JSON-LD, GraphML)
- **Trigger**: Before/after hooks for CREATE, DELETE, SET operations with rollback support
- **PluginRegistry**: Generic, thread-safe `HashMap`-based registry (`Send + Sync`)
- **Feature Flag**: `plugin` feature flag — zero overhead when disabled (cfg-branched)

### CI/CD Pipeline

- **GitHub Actions**: 6 parallel jobs (check, msrv, test, coverage, security, bench-check)
- **Coverage Gate**: 85% minimum enforced in CI
- **MSRV Verification**: Rust 1.84 compatibility check on every PR
- **Security Audit**: `cargo audit` integrated into pipeline
- **Benchmark Regression**: Criterion benchmark smoke-test on all feature combinations

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

### Plugin Example

Enable the `plugin` feature and register a custom scalar function:

```toml
[dependencies]
cypherlite-query = { path = "crates/cypherlite-query", features = ["plugin"] }
cypherlite-core  = { path = "crates/cypherlite-core",  features = ["plugin"] }
```

```rust
use cypherlite_query::CypherLite;
use cypherlite_core::plugin::{Plugin, ScalarFunction};
use cypherlite_core::PropertyValue;

struct UpperFn;

impl Plugin for UpperFn {
    fn name(&self)        -> &str { "upper" }
    fn version(&self)     -> &str { "1.0.0" }
    fn description(&self) -> &str { "Converts a string to uppercase" }
    fn init(&mut self)    -> Result<(), Box<dyn std::error::Error>> { Ok(()) }
    fn shutdown(&mut self) {}
}

impl ScalarFunction for UpperFn {
    fn call(&self, args: &[PropertyValue]) -> Result<PropertyValue, Box<dyn std::error::Error>> {
        match &args[0] {
            PropertyValue::String(s) => Ok(PropertyValue::String(s.to_uppercase())),
            _ => Err("expected string".into()),
        }
    }
}

let db = CypherLite::open("my_graph.cyl")?;
db.register_scalar_function(Box::new(UpperFn))?;

// Custom function is now available in Cypher queries
let result = db.execute("MATCH (p:Person) RETURN upper(p.name)")?;
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
│   ├── cypherlite-core/        # Core types, traits, error handling, plugin traits
│   │   └── src/
│   │       ├── lib.rs          # Re-exports
│   │       ├── types.rs        # NodeId, EdgeId, PropertyValue, NodeRecord, RelationshipRecord
│   │       ├── error.rs        # CypherLiteError
│   │       ├── config.rs       # Config struct
│   │       ├── traits.rs       # TransactionView and other core traits
│   │       ├── trigger_types.rs # TriggerContext, EntityType, TriggerOperation
│   │       └── plugin/         # Plugin, ScalarFunction, IndexPlugin, Serializer, Trigger traits
│   │           └── mod.rs      # PluginRegistry<T>
│   │
│   ├── cypherlite-storage/     # Storage engine implementation
│   │   └── src/
│   │       ├── lib.rs          # StorageEngine public API
│   │       ├── page/           # Page format, Buffer Pool, Page Manager
│   │       ├── btree/          # B+Tree, Node Store, Edge Store, Property Store
│   │       ├── wal/            # Write-Ahead Log, Checkpoint, Recovery
│   │       ├── catalog/        # Label/Type registry (BiMap String<->u32)
│   │       ├── transaction/    # MVCC Transaction Manager
│   │       └── subgraph/       # SubgraphStore, MembershipIndex
│   │
│   └── cypherlite-query/       # Query engine
│       └── src/
│           ├── api/            # CypherLite, QueryResult, Row, Transaction, plugin registration
│           ├── lexer/          # logos-based tokenizer
│           ├── parser/         # Recursive descent parser, Pratt expressions, AST
│           ├── semantic/       # Variable scope validation, symbol table
│           ├── planner/        # Logical/physical plan, cost-based optimization
│           └── executor/       # Volcano iterator model, operators, ScalarFnLookup, TriggerLookup
│
└── docs/                       # Architecture and design documentation
```

### Feature Flags

Feature flags form an additive chain (each enables all previous):

| Feature | Enables |
|---------|---------|
| `temporal-core` | AT TIME queries, version store (default) |
| `temporal-edge` | Edge versioning, temporal relationship queries |
| `subgraph` | SubgraphStore, CREATE/MATCH SNAPSHOT |
| `hypergraph` | Native hyperedges, HYPEREDGE syntax |
| `full-temporal` | All temporal/graph features |
| `plugin` | Plugin system (independent, zero overhead when disabled) |

## File Format

- **`.cyl`**: Primary database file (4KB pages, B+Tree structure)
- **`.cyl-wal`**: Write-Ahead Log (frame-based, checksum-verified)
- **Magic Number**: `CYLT` (0x43594C54)
- **Page Size**: 4,096 bytes (fixed)

## Dependencies

| Crate | Version | Purpose |
|-------|---------|---------|
| `parking_lot` | 0.12 | RwLock, Mutex for single-writer multiple-reader concurrency |
| `dashmap` | 6 | Concurrent hash map for WAL frame index |
| `bincode` | 1 | Binary serialization for page data |
| `serde` | 1 | Derive macros for serializable structs |
| `thiserror` | 2 | Error type definitions |
| `crossbeam` | 0.8 | Channel and concurrency utilities |
| `logos` | 0.14 | Lexer generator for Cypher tokenization |
| `proptest` | 1 | Property-based testing (dev) |
| `criterion` | 0.5 | Benchmark harness (dev) |
| `tempfile` | 3 | Temporary files for tests (dev) |

**MSRV**: Rust 1.84+ (Edition 2021)

## Testing

```bash
# Run all tests (default features)
cargo test --workspace

# Run with all features enabled
cargo test --workspace --all-features

# Run with coverage
cargo llvm-cov --workspace --all-features --summary-only

# Run linter
cargo clippy --workspace --all-targets --all-features -- -D warnings

# Run benchmarks (smoke test)
cargo bench --workspace --all-features -- --test
```

**Test coverage**: 1,309 tests across workspace, all features, 0 clippy warnings

## TRUST 5 Quality Gates

| Dimension | Status | Details |
|-----------|--------|---------|
| Tested | Pass | 1,309 tests (all-features), 85%+ coverage, proptest + criterion benchmarks |
| Readable | Pass | English comments, conventional naming, `#![warn(missing_docs)]` |
| Unified | Pass | Consistent Rust style, rustfmt, clippy clean (0 warnings) |
| Secured | Pass | No unsafe, cargo audit clean, TSAN verified (0 data races) |
| Trackable | Pass | Conventional commits, all 10 SPECs referenced in history |

## SPECs

| SPEC | Version | Description | Status |
|------|---------|-------------|--------|
| [SPEC-DB-001](.moai/specs/SPEC-DB-001/spec.md) | v0.1.0 | Storage Engine | Complete |
| [SPEC-DB-002](.moai/specs/SPEC-DB-002/spec.md) | v0.2.0 | Query Engine | Complete |
| [SPEC-DB-003](.moai/specs/SPEC-DB-003/spec.md) | v0.3.0 | Advanced Query | Complete |
| [SPEC-DB-004](.moai/specs/SPEC-DB-004/spec.md) | v0.4.0 | Temporal Core | Complete |
| [SPEC-DB-005](.moai/specs/SPEC-DB-005/spec.md) | v0.5.0 | Temporal Edge | Complete |
| [SPEC-DB-006](.moai/specs/SPEC-DB-006/spec.md) | v0.6.0 | Subgraph Entities | Complete |
| [SPEC-DB-007](.moai/specs/SPEC-DB-007/spec.md) | v0.7.0 | Native Hyperedge | Complete |
| [SPEC-DB-008](.moai/specs/SPEC-DB-008/spec.md) | v0.8.0 | Inline Property Filter | Complete |
| [SPEC-INFRA-001](.moai/specs/SPEC-INFRA-001/spec.md) | v0.9.0 | CI/CD Pipeline | Complete |
| [SPEC-PLUGIN-001](.moai/specs/SPEC-PLUGIN-001/spec.md) | v1.0.0 | Plugin System | Complete |

## License

TBD
