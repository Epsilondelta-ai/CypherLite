# Changelog

All notable changes to CypherLite are documented in this file.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.0.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [1.0.0] - 2026-03-14

### SPEC Reference

- **SPEC-PLUGIN-001**: CypherLite Plugin System (v1.0)

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
- Feature flag `plugin` — independent of temporal/subgraph/hypergraph feature chain, zero overhead when disabled (cfg-branched)

**Query Integration** (`cypherlite-query`):
- `ScalarFnLookup` trait for pluggable function dispatch in `eval.rs`
- `TriggerLookup` trait for pluggable trigger dispatch in operator modules
- `register_scalar_function()` API method on `CypherLite`
- `register_index_plugin()` API method on `CypherLite`
- `register_serializer()` API method on `CypherLite`
- `register_trigger()` API method on `CypherLite`
- `list_scalar_functions()` API method on `CypherLite`
- `list_index_plugins()` API method on `CypherLite`
- `list_serializers()` API method on `CypherLite`
- `list_triggers()` API method on `CypherLite`
- `export_data()` method for `Serializer` plugin integration
- `import_data()` method for `Serializer` plugin integration
- Trigger `before`/`after` hooks wired into CREATE, DELETE, and SET operators with automatic rollback on hook error

### Changed

- Version bump: `0.8.0` → `1.0.0` across all 3 crates (`cypherlite-core`, `cypherlite-storage`, `cypherlite-query`)
- `eval()` function signature: added `scalar_fns` parameter for pluggable function dispatch
- `execute()` function signature: added `scalar_fns` and `trigger_fns` parameters for plugin dispatch
- All 12 operator modules updated to thread new function signature parameters through the Volcano iterator stack

### Test Coverage

- 1,309 tests total (all-features), 0 clippy warnings
- Plugin feature tests: 47 new tests (22 core plugin trait/registry tests + 25 query integration tests)
- Zero regressions on non-plugin builds (`cargo test --workspace` without `plugin` feature)

---

## [0.1.0] - 2026-03-10

### SPEC Reference

- **SPEC-DB-001**: CypherLite Phase 1 - Storage Engine (v0.1)

### Added

**cypherlite-core** crate:
- `NodeId`, `EdgeId`, `PageId` newtype wrappers (u64/u32-based)
- `PropertyValue` enum: Null, Bool, Int64, Float64, String, Bytes, Array
- `NodeRecord`, `RelationshipRecord` data structures with adjacency chain fields
- `CypherLiteError` error type covering all storage error cases (IoError, CorruptedPage, TransactionConflict, OutOfSpace, InvalidMagicNumber, UnsupportedVersion, ChecksumMismatch, NodeNotFound, EdgeNotFound)
- `Config` struct: configurable page size, cache capacity, WAL path derivation
- `TransactionView` trait for snapshot isolation abstraction

**cypherlite-storage** crate:
- `StorageEngine` — public API for all database operations (open, create_node, get_node, update_node, delete_node, create_edge, get_edge, delete_edge, get_edges_for_node, begin_read, begin_write, commit, rollback, checkpoint)
- `PageManager` — 4KB fixed-size page I/O with header validation (magic `CYLT`, version check), free space map (bitmap-based allocation)
- `BufferPool` — LRU page cache (default 256 pages / 1MB), pin/unpin support, dirty page tracking
- `BTree` — generic B+Tree with interior and leaf node distinction (~100 branching factor on 4KB pages)
- `NodeStore` — node CRUD backed by BTree with adjacency chain management
- `EdgeStore` — edge CRUD backed by BTree with bidirectional adjacency chain updates
- `PropertyStore` — inline property serialization (<=31 bytes), overflow page pointer for larger values, support for all 7 PropertyValue types
- `WalWriter` — WAL frame writing with fsync, frame number tracking, checksum generation
- `WalReader` — WAL index (DashMap-based), snapshot-aware page lookup, snapshot isolation
- `Checkpoint` — WAL-to-main-file page copy, WAL reset after completion
- `Recovery` — WAL replay on startup, corrupted frame detection via checksum, uncommitted frame discard
- `MvccTransactionManager` — Single-Writer Multiple-Reader via `parking_lot::RwLock`, snapshot frame capture on begin, exclusive write lock, commit/rollback semantics

### Technical Decisions

- **I/O model**: Synchronous (no async) — follows SQLite single-process embedded model
- **Concurrency**: Single-Writer Multiple-Reader via `parking_lot::RwLock`
- **Isolation level**: Snapshot Isolation (WAL frame index-based)
- **MSRV**: Rust 1.84+
- **dashmap 6** (upgraded from spec's dashmap 5 for performance improvements)
- **thiserror 2** (upgraded from spec's thiserror 1, cleaner derive macro)

### MX Annotations Added

- `@MX:WARN` — `cypherlite-storage/src/transaction/mvcc.rs:48` — unsafe transmute for MutexGuard lifetime extension
- `@MX:ANCHOR` — `cypherlite-storage/src/wal/checkpoint.rs:13` — critical WAL-to-main-file flush path
- `@MX:NOTE` — `cypherlite-storage/src/wal/recovery.rs:15` — WAL reset behavior after replay
- `@MX:NOTE` — `cypherlite-storage/src/wal/mod.rs:115` — wrapping-add checksum algorithm

### Test Coverage

- 207 tests total (36 unit in core, 146 unit in storage, 25 integration)
- 96.82% line coverage (target: 85%)
- Test suites: acid_compliance, concurrency, crud_operations

---

[1.0.0]: https://github.com/Epsilondelta-ai/CypherLite/compare/v0.8.0...v1.0.0
[0.1.0]: https://github.com/Epsilondelta-ai/CypherLite/releases/tag/v0.1.0
