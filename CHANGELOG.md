# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [1.2.0] - 2026-03-15

### SPEC Reference

- **SPEC-DOC-001**: Documentation, i18n, and Static Website

### Added

- `#![warn(missing_docs)]` enforcement across all public APIs in `cypherlite-core`, `cypherlite-storage`, `cypherlite-query`, and `cypherlite-ffi`
- Comprehensive rustdoc coverage for all public types, traits, and functions
- `CONTRIBUTING.md` with contribution guidelines, development setup, and pull request process
- Nextra-based documentation website with 10-language internationalization support (en, ko, ja, zh-CN, zh-TW, es, fr, de, pt, ru)
- GitHub Pages deployment workflow for static documentation site
- Rust usage examples under `examples/` directory covering core operations
- FFI binding examples for C, Go, Python, and Node.js integrations
- `crates.io` publishing preparation: keywords, categories, `readme`, `documentation`, `homepage` fields in all `Cargo.toml` files
- MIT OR Apache-2.0 dual license (`LICENSE-MIT`, `LICENSE-APACHE`)

### Changed

- Version bump: `1.1.0` → `1.2.0` across all crates (`cypherlite-core`, `cypherlite-storage`, `cypherlite-query`, `cypherlite-ffi`)
- `README.md` complete rewrite with multi-language Quick Start guides, feature overview, architecture diagram, and installation instructions
- `CHANGELOG.md` expanded with full version history from v0.1.0 to v1.2.0

---

## [1.1.0] - 2026-03-14

### SPEC Reference

- **SPEC-PERF-001**: Performance Optimization

### Added

- `BufferPool` LRU cache hit-rate instrumentation and optimization
- Benchmark suite (`benches/`) with `criterion`-based micro-benchmarks for node/edge CRUD, B+Tree operations, WAL write throughput, and query execution
- `cargo bench` smoke-test integration in CI pipeline

### Changed

- Version bump: `1.0.0` → `1.1.0` across all crates
- `BufferPool` eviction policy tuned: reduced unnecessary dirty-page writeback on read-only workloads
- `PropertyStore` inline serialization path optimized: eliminated redundant length prefix for fixed-size types (`Int64`, `Float64`, `Bool`)
- `WalWriter` batch-flush threshold adjusted to reduce `fsync` call frequency under high write load
- Query executor: predicate push-down applied earlier in plan rewriting to reduce intermediate tuple cardinality

### Fixed

- Memory usage regression introduced in v1.0.0 `eval()` signature change: removed transient allocation in hot-path scalar function dispatch
- `cargo audit` advisory RUSTSEC-2025-0020 resolved by upgrading `pyo3` 0.23 → 0.24 in `cypherlite-ffi-python`

### Test Coverage

- 1,309 tests total (all-features), 0 clippy warnings
- Benchmark smoke test added to CI (ensures benchmarks compile and run without panic)

---

## [1.0.0] - 2026-03-14

### SPEC Reference

- **SPEC-PLUGIN-001**: CypherLite Plugin System

### Added

**Plugin Infrastructure** (`cypherlite-core`):
- `Plugin` base trait with `name`, `version`, `description`, `init`, `shutdown` lifecycle methods
- `ScalarFunction` trait for custom query functions callable in Cypher expressions
- `IndexPlugin` trait for custom index implementations (e.g., HNSW vector index)
- `Serializer` trait for custom import/export formats (e.g., JSON-LD, GraphML)
- `Trigger` trait with `before` and `after` hooks for CREATE, DELETE, SET operations with rollback support
- `PluginRegistry<T>` generic registry (`Send + Sync`, `HashMap`-based) for all plugin types
- `TriggerContext` struct providing `entity_type`, `operation`, and `properties` access to hook implementations
- `EntityType` enum (`Node`, `Edge`) for trigger dispatch context
- `TriggerOperation` enum (`Create`, `Delete`, `SetProperty`, `RemoveProperty`) for trigger dispatch context
- Feature flag `plugin` — independent of temporal/subgraph/hypergraph feature chain, zero overhead when disabled

**Query Integration** (`cypherlite-query`):
- `ScalarFnLookup` trait for pluggable function dispatch in `eval.rs`
- `TriggerLookup` trait for pluggable trigger dispatch in operator modules
- `register_scalar_function()`, `register_index_plugin()`, `register_serializer()`, `register_trigger()` API methods on `CypherLite`
- `list_scalar_functions()`, `list_index_plugins()`, `list_serializers()`, `list_triggers()` API methods on `CypherLite`
- `export_data()` and `import_data()` methods for `Serializer` plugin integration
- Trigger `before`/`after` hooks wired into CREATE, DELETE, and SET operators with automatic rollback on hook error

### Changed

- Version bump: `0.8.0` → `1.0.0` across all crates (`cypherlite-core`, `cypherlite-storage`, `cypherlite-query`)
- `eval()` function signature: added `scalar_fns` parameter for pluggable function dispatch
- `execute()` function signature: added `scalar_fns` and `trigger_fns` parameters for plugin dispatch
- All 12 operator modules updated to thread new function signature parameters through the Volcano iterator stack

### Test Coverage

- 1,309 tests total (all-features), 0 clippy warnings
- 47 new plugin feature tests (22 core plugin trait/registry + 25 query integration)
- Zero regressions on non-plugin builds (`cargo test --workspace` without `plugin` feature)

---

## [0.9.0] - 2026-03-13

### SPEC Reference

- **SPEC-INFRA-001**: CI/CD Pipeline

### Added

- GitHub Actions CI/CD pipeline with 6 parallel jobs:
  - `check`: `cargo check` + `cargo clippy -- -D warnings` on stable toolchain
  - `msrv`: `cargo check` on Rust 1.84 minimum supported version
  - `test`: `cargo test --workspace --all-features` full test suite
  - `coverage`: `cargo-tarpaulin` line coverage with 85% gate
  - `security`: `cargo audit` vulnerability scan
  - `bench-check`: benchmark compile-and-smoke-run check
- `rust-toolchain.toml` pinning stable channel with component `clippy`, `rustfmt`, `llvm-tools-preview`
- `.github/workflows/ci.yml` workflow definition
- Dependabot configuration for automated dependency updates

### Changed

- Version bump: `0.8.0` → `0.9.0` across all crates

---

## [0.8.0] - 2026-03-13

### SPEC Reference

- **SPEC-DB-008**: Inline Property Filter Fix

### Fixed

- `MATCH (n:Label {key: value})` inline property filter was silently ignored in `NodeScan` path — predicates now correctly applied during scan
- Inline property filter for relationship target nodes was not pushed down into `RelationshipScan` — fixed in planner

### Added

- Property-based tests (`proptest`) for inline filter correctness across arbitrary node/relationship schemas
- Integration test suite `inline_property_filters` covering all scan paths

### Changed

- Version bump: `0.7.0` → `0.8.0` across all crates
- Query planner: inline property map expressions in `MATCH` patterns now parsed and applied as scan-level predicates

---

## [0.7.0] - 2026-03-12

### SPEC Reference

- **SPEC-DB-007**: Native Hyperedge Support

### Added

**Storage** (`cypherlite-storage`):
- `HyperEdgeStore` for N:M relationship storage with participant list serialization
- `ReverseIndex` for efficient participant-to-hyperedge lookup
- Page header upgraded to v5 to include hyperedge section offset
- `TemporalRef` type for time-aware references in hyperedge participants

**Query** (`cypherlite-query`):
- `HYPEREDGE` keyword in lexer and parser for creating N:M relationships
- `MATCH HYPEREDGE` syntax for querying hyperedge participants
- Hyperedge scan operator wired into Volcano executor
- Temporal hyperedge filtering with `AT TIME` / `BETWEEN TIME` predicates

### Changed

- Version bump: `0.6.0` → `0.7.0` across all crates
- Feature flag `hypergraph` depends on `subgraph` feature (feature chain: `temporal-edge` → `subgraph` → `hypergraph`)

### Test Coverage

- Property-based tests (`proptest`) for hyperedge participant encoding
- Benchmark: hyperedge creation and participant lookup throughput

---

## [0.6.0] - 2026-03-11

### SPEC Reference

- **SPEC-DB-006**: Subgraph Entities

### Added

**Storage** (`cypherlite-storage`):
- `SubgraphStore` for named subgraph creation and membership management
- `MembershipIndex` for efficient node/edge-to-subgraph containment lookup
- Page header upgraded to v4 to include subgraph section offset

**Query** (`cypherlite-query`):
- `CREATE SNAPSHOT` syntax for creating named subgraph snapshots
- `MATCH SNAPSHOT` syntax for querying subgraph contents
- `SubgraphScan` operator in Volcano executor
- Virtual `:CONTAINS` relationship resolution for subgraph membership queries
- `Value::Subgraph` variant for passing subgraph references through query pipeline

### Changed

- Version bump: `0.5.0` → `0.6.0` across all crates
- Feature flag `subgraph` added (depends on `temporal-edge`)

### Test Coverage

- Integration tests: subgraph creation, membership queries, virtual `:CONTAINS` traversal
- Property-based tests (`proptest`) for `MembershipIndex` correctness
- Benchmark: subgraph snapshot and membership lookup throughput

---

## [0.5.0] - 2026-03-11

### SPEC Reference

- **SPEC-DB-005**: Temporal Edge Support

### Added

**Storage** (`cypherlite-storage`):
- Edge temporal properties: `created_at`, `deleted_at` timestamps on `RelationshipRecord`
- `EdgeTemporalIndex` for time-range-based edge lookup
- Page header upgraded to v3 to include edge temporal index offset

**Query** (`cypherlite-query`):
- `AT TIME` and `BETWEEN TIME` predicates extended to relationship patterns
- `RelationshipScan` operator updated to apply temporal filters at scan level
- Temporal edge filtering tests for all three scan paths (forward, backward, both)

### Changed

- Version bump: `0.4.0` → `0.5.0` across all crates
- Feature flag `temporal-edge` added; temporal core (`temporal` flag) is a prerequisite

---

## [0.4.0] - 2026-03-11

### SPEC Reference

- **SPEC-DB-004**: Temporal Query Core

### Added

**Core** (`cypherlite-core`):
- `PropertyValue::DateTime` variant with `chrono::DateTime<Utc>` backing
- Temporal helper functions: `now()`, `datetime_from_str()`, `datetime_diff_seconds()`

**Storage** (`cypherlite-storage`):
- `VersionStore` for node and edge version history (append-only log per entity)
- Automatic `created_at` / `updated_at` timestamp tracking on CREATE and SET operations

**Query** (`cypherlite-query`):
- `AT TIME <expr>` clause for point-in-time entity lookup
- `BETWEEN TIME <start> AND <end>` clause for time-range queries
- Temporal plan node `TemporalFilter` in query planner

### Changed

- Version bump: `0.3.0` → `0.4.0` across all crates
- `NodeRecord` and `RelationshipRecord` extended with optional `created_at` / `deleted_at` fields
- Feature flag `temporal` enables all temporal functionality

### Test Coverage

- Integration tests: point-in-time queries, range queries, version history correctness
- Property-based tests (`proptest`) for `VersionStore` append and lookup
- Benchmark: version history write throughput

---

## [0.3.0] - 2026-03-10

### SPEC Reference

- **SPEC-DB-003**: Advanced Query Features

### Added

**Query** (`cypherlite-query`):
- `MERGE` clause with `ON MATCH SET` and `ON CREATE SET` sub-clauses for upsert semantics
- `WITH` clause for multi-part query composition and result piping
- `UNWIND` clause for list expansion into rows
- `OPTIONAL MATCH` clause returning `null` for missing patterns
- Variable-length path traversal: `(a)-[*1..5]->(b)` syntax with configurable depth bounds
- `ORDER BY` clause with `ASC` / `DESC` support
- `LIMIT` and `SKIP` clauses for result pagination

**Storage** (`cypherlite-storage`):
- `PropertyIndex` secondary index infrastructure with `BTree`-backed range scans
- Index auto-selection in query planner when indexed property predicate detected

**Query Optimizer**:
- Predicate push-down rule: move filter predicates closer to scan operators
- Index selection rule: replace `NodeScan` + `Filter` with `IndexScan` when applicable
- Projection push-down: eliminate unused columns early in the plan

### Changed

- Version bump: `0.2.0` → `0.3.0` across all crates
- Query planner refactored into multi-pass optimizer with pluggable rewrite rules

### Test Coverage

- Property-based tests (`proptest`) for variable-length path traversal, `OPTIONAL MATCH`, and `UNWIND`
- Integration test suite expanded to cover `MERGE`, `WITH`, and pagination

---

## [0.2.0] - 2026-03-10

### SPEC Reference

- **SPEC-DB-002**: Query Engine

### Added

**Core** (`cypherlite-core`):
- `LabelRegistry` trait and `QueryError` error variants for query-layer error handling

**Storage** (`cypherlite-storage`):
- `Catalog` for label-to-NodeId index management
- Full-scan APIs: `scan_nodes()`, `scan_edges()` for sequential access
- `LabelRegistry` integration into `StorageEngine`

**Query** (`cypherlite-query`) — new crate:
- Cypher lexer: tokenization of openCypher subset keywords, identifiers, literals, and operators
- Cypher parser: recursive descent parser producing an AST for `CREATE`, `MATCH`, `SET`, `DELETE`, `RETURN` clauses
- Query planner: AST → logical plan (NodeScan, RelationshipScan, Filter, Projection, Create, Delete, Set operators)
- Volcano iterator executor: pull-based evaluation model with `open()` / `next()` / `close()` protocol
- `CypherLite` public API struct exposing `execute(query: &str)` entry point
- Pattern matching: single-hop relationship traversal `(a)-[r]->(b)` with label and property filters
- `RETURN` with expression evaluation: property access, comparison operators, boolean logic

### Changed

- Version bump: `0.1.0` → `0.2.0` across all crates
- `cypherlite-storage` Cargo.toml: added `cypherlite-core` dependency on query error types

### Test Coverage

- Property-based tests (`proptest`) and benchmarks added to `cypherlite-query`
- `#![warn(missing_docs)]` enforced on all public items in `cypherlite-query`
- Thread-safety verified with TSAN (`cargo test -Z sanitizer=thread`)

---

## [0.1.0] - 2026-03-10

### SPEC Reference

- **SPEC-DB-001**: Storage Engine

### Added

**Core** (`cypherlite-core`) — new crate:
- `NodeId`, `EdgeId`, `PageId` newtype wrappers (u64/u32-based)
- `PropertyValue` enum: `Null`, `Bool`, `Int64`, `Float64`, `String`, `Bytes`, `Array`
- `NodeRecord`, `RelationshipRecord` data structures with adjacency chain fields
- `CypherLiteError` error type covering all storage error cases: `IoError`, `CorruptedPage`, `TransactionConflict`, `OutOfSpace`, `InvalidMagicNumber`, `UnsupportedVersion`, `ChecksumMismatch`, `NodeNotFound`, `EdgeNotFound`
- `Config` struct: configurable page size, cache capacity, WAL path derivation
- `TransactionView` trait for snapshot isolation abstraction

**Storage** (`cypherlite-storage`) — new crate:
- `StorageEngine` — public API: `open`, `create_node`, `get_node`, `update_node`, `delete_node`, `create_edge`, `get_edge`, `delete_edge`, `get_edges_for_node`, `begin_read`, `begin_write`, `commit`, `rollback`, `checkpoint`
- `PageManager` — 4KB fixed-size page I/O with header validation (magic `CYLT` / `0x43594C54`, version check) and bitmap-based free space map
- `BufferPool` — LRU page cache (default 256 pages / 1MB) with pin/unpin support and dirty page tracking
- `BTree` — generic B+Tree with interior and leaf node distinction (~100 branching factor on 4KB pages)
- `NodeStore` — node CRUD backed by `BTree` with adjacency chain management
- `EdgeStore` — edge CRUD backed by `BTree` with bidirectional adjacency chain updates
- `PropertyStore` — inline property serialization (≤31 bytes), overflow page pointer for larger values, support for all 7 `PropertyValue` types
- `WalWriter` — WAL frame writing with `fsync`, frame number tracking, wrapping-add checksum generation
- `WalReader` — WAL index (`DashMap`-based), snapshot-aware page lookup, snapshot isolation
- `Checkpoint` — WAL-to-main-file page copy, WAL reset after completion
- `Recovery` — WAL replay on startup, corrupted frame detection via checksum, uncommitted frame discard
- `MvccTransactionManager` — Single-Writer Multiple-Reader via `parking_lot::RwLock`, snapshot frame capture on begin, exclusive write lock, commit/rollback semantics
- File extensions: `.cyl` for data files, `.cyl-wal` for WAL files

### Technical Decisions

- I/O model: synchronous (no async) — follows SQLite single-process embedded model
- Concurrency: Single-Writer Multiple-Reader via `parking_lot::RwLock`
- Isolation level: Snapshot Isolation (WAL frame index-based)
- MSRV: Rust 1.84+ (uses `Option::is_none_or`)
- `dashmap` 6 (upgraded from spec's dashmap 5 for performance improvements)
- `thiserror` 2 (upgraded from spec's thiserror 1, cleaner derive macro)

### Test Coverage

- 207 tests total (36 unit in `cypherlite-core`, 146 unit in `cypherlite-storage`, 25 integration)
- 96.82% line coverage (target: 85%)
- Test suites: `acid_compliance`, `concurrency`, `crud_operations`

---

[Unreleased]: https://github.com/Epsilondelta-ai/CypherLite/compare/v1.2.0...HEAD
[1.2.0]: https://github.com/Epsilondelta-ai/CypherLite/compare/v1.1.0...v1.2.0
[1.1.0]: https://github.com/Epsilondelta-ai/CypherLite/compare/v1.0.0...v1.1.0
[1.0.0]: https://github.com/Epsilondelta-ai/CypherLite/compare/v0.9.0...v1.0.0
[0.9.0]: https://github.com/Epsilondelta-ai/CypherLite/compare/v0.8.0...v0.9.0
[0.8.0]: https://github.com/Epsilondelta-ai/CypherLite/compare/v0.7.0...v0.8.0
[0.7.0]: https://github.com/Epsilondelta-ai/CypherLite/compare/v0.6.0...v0.7.0
[0.6.0]: https://github.com/Epsilondelta-ai/CypherLite/compare/v0.5.0...v0.6.0
[0.5.0]: https://github.com/Epsilondelta-ai/CypherLite/compare/v0.4.0...v0.5.0
[0.4.0]: https://github.com/Epsilondelta-ai/CypherLite/compare/v0.3.0...v0.4.0
[0.3.0]: https://github.com/Epsilondelta-ai/CypherLite/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/Epsilondelta-ai/CypherLite/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/Epsilondelta-ai/CypherLite/releases/tag/v0.1.0
