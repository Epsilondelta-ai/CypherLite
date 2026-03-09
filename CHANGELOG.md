# Changelog

All notable changes to CypherLite are documented in this file.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.0.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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
- `PropertyStore` — inline property serialization (≤31 bytes), overflow page pointer for larger values, support for all 7 PropertyValue types
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

[0.1.0]: https://github.com/Epsilondelta-ai/CypherLite/releases/tag/v0.1.0
