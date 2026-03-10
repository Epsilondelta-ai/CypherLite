# SPEC-DB-002 Research: Storage Engine Analysis

Researcher: team-reader (researcher)
Date: 2026-03-10

---

## 1. Workspace Structure

### Workspace Layout

Root `Cargo.toml` (resolver = "2"):
- Members: `crates/cypherlite-core`, `crates/cypherlite-storage`
- No workspace-level dependencies

### cypherlite-core

- **Cargo.toml**: `crates/cypherlite-core/Cargo.toml`
- Edition: 2021, MSRV: 1.84
- Dependencies: `thiserror = "2"`, `serde = { version = "1", features = ["derive"] }`, `bincode = "1"`
- Dev-dependencies: `proptest = "1"`
- Role: Pure types + traits + config — no I/O, no storage logic. Used as a shared foundation.

### cypherlite-storage

- **Cargo.toml**: `crates/cypherlite-storage/Cargo.toml`
- Edition: 2021, MSRV: 1.84
- Dependencies: `cypherlite-core`, `parking_lot = "0.12"`, `crossbeam = "0.8"`, `dashmap = "6"`, `bincode = "1"`
- Dev-dependencies: `tempfile = "3"`, `criterion = { version = "0.5", features = ["html_reports"] }`, `proptest = "1"`
- Benchmark: `storage_bench` (harness = false)
- Role: Full storage engine — WAL, B+Tree (in-memory with page IDs), buffer pool, MVCC transactions, checkpoint

### Dependency Implications for Query Engine (cypherlite-query)

A new `crates/cypherlite-query` crate should add:
```toml
[dependencies]
cypherlite-core = { path = "../cypherlite-core" }
cypherlite-storage = { path = "../cypherlite-storage" }
```
No async runtime is used. The storage engine is entirely synchronous.

---

## 2. Core Types (cypherlite-core)

All items below are re-exported from `crates/cypherlite-core/src/lib.rs:1-9`.

### Identifier Types

| Type | File:Line | Definition | Notes |
|------|-----------|------------|-------|
| `NodeId` | `types.rs:7` | `struct NodeId(pub u64)` | Copy, Hash, Ord, Serialize |
| `EdgeId` | `types.rs:11` | `struct EdgeId(pub u64)` | Copy, Hash, Ord, Serialize |
| `PageId` | `types.rs:15` | `struct PageId(pub u32)` | Copy, Hash, Ord, Serialize |

All three are newtype wrappers. The inner `u64`/`u32` value is `pub`. NodeId and EdgeId start at 1 (set in `DatabaseHeader::new()` at `page/mod.rs:118`).

### PropertyValue

`types.rs:19-34` — the universal property type:

```rust
pub enum PropertyValue {
    Null,           // type_tag = 0
    Bool(bool),     // type_tag = 1
    Int64(i64),     // type_tag = 2
    Float64(f64),   // type_tag = 3
    String(String), // type_tag = 4
    Bytes(Vec<u8>), // type_tag = 5
    Array(Vec<PropertyValue>), // type_tag = 6 — nested, bincode-encoded
}
```

Implements: Debug, Clone, PartialEq, Serialize, Deserialize. The `type_tag()` method is at `types.rs:73`.

**Critical for query engine**: All comparisons (`=`, `<`, `>`, `IN`) must operate on `PropertyValue`. No `PartialOrd` is implemented — the query engine must implement its own comparison logic for `<`, `<=`, `>`, `>=`.

### Direction

`types.rs:38-42` — edge traversal direction:
```rust
pub enum Direction { Outgoing, Incoming, Both }
```
Implements Copy, PartialEq, Serialize, Deserialize.

### NodeRecord

`types.rs:45-54`:
```rust
pub struct NodeRecord {
    pub node_id: NodeId,
    pub labels: Vec<u32>,             // label IDs (NOT strings)
    pub properties: Vec<(u32, PropertyValue)>, // (key_id, value) pairs
    pub next_edge_id: Option<EdgeId>, // adjacency chain head
    pub overflow_page: Option<PageId>,
}
```

**Critical**: Labels and property keys are stored as `u32` integer IDs, not strings. The query engine must maintain a string-to-id mapping (symbol table) to resolve `Person` → `42u32` and `name` → `1u32`.

### RelationshipRecord

`types.rs:57-69`:
```rust
pub struct RelationshipRecord {
    pub edge_id: EdgeId,
    pub start_node: NodeId,
    pub end_node: NodeId,
    pub rel_type_id: u32,            // relationship type ID (NOT a string)
    pub direction: Direction,        // always stored as Outgoing in Phase 1
    pub next_out_edge: Option<EdgeId>,
    pub next_in_edge: Option<EdgeId>,
    pub properties: Vec<(u32, PropertyValue)>,
}
```

**Critical**: `rel_type_id` is also a `u32` integer ID. The same symbol table must handle label IDs, property key IDs, and relationship type IDs.

### Error Types

`error.rs:5-35`:
```rust
pub enum CypherLiteError {
    IoError(#[from] std::io::Error),
    CorruptedPage { page_id: u32, reason: String },
    TransactionConflict,
    OutOfSpace,
    InvalidMagicNumber,
    UnsupportedVersion { found: u32, supported: u32 },
    ChecksumMismatch { expected: u64, found: u64 },
    SerializationError(String),
    NodeNotFound(u64),
    EdgeNotFound(u64),
}
pub type Result<T> = std::result::Result<T, CypherLiteError>;
```

The query engine should define its own error enum (`QueryError`) that wraps `CypherLiteError` via `#[from]`.

### TransactionView Trait

`traits.rs:4-7`:
```rust
pub trait TransactionView {
    fn snapshot_frame(&self) -> u64;
}
```
Implemented by both `ReadTransaction` and `WriteTransaction`. Object-safe.

### DatabaseConfig

`config.rs:15-25`:
```rust
pub struct DatabaseConfig {
    pub path: PathBuf,
    pub page_size: u32,        // always 4096
    pub cache_capacity: usize, // default 256 pages
    pub wal_sync_mode: SyncMode,
}
```

`config.rs:7-12` — `SyncMode`:
```rust
pub enum SyncMode { Full, Normal }
```
`Full` = fsync after every write. `Normal` = OS decides when to flush.

---

## 3. Storage API Surface (cypherlite-storage)

### Primary Entry Point: StorageEngine

`lib.rs:26-35` — struct definition:
```rust
pub struct StorageEngine {
    page_manager: PageManager,
    buffer_pool: BufferPool,
    wal_writer: WalWriter,
    wal_reader: WalReader,
    tx_manager: TransactionManager,
    node_store: NodeStore,
    edge_store: EdgeStore,
    config: DatabaseConfig,
}
```
All fields are private. The query engine interacts exclusively through `StorageEngine`'s public methods.

### Public Methods Summary

#### Lifecycle

| Method | Signature | File:Line |
|--------|-----------|-----------|
| `open` | `fn open(config: DatabaseConfig) -> Result<Self>` | `lib.rs:39` |

`open()` handles both create-new and open-existing, runs WAL recovery automatically.

#### Node Operations

| Method | Signature | File:Line |
|--------|-----------|-----------|
| `create_node` | `fn create_node(&mut self, labels: Vec<u32>, properties: Vec<(u32, PropertyValue)>) -> NodeId` | `lib.rs:90` |
| `get_node` | `fn get_node(&self, node_id: NodeId) -> Option<&NodeRecord>` | `lib.rs:102` |
| `update_node` | `fn update_node(&mut self, node_id: NodeId, properties: Vec<(u32, PropertyValue)>) -> Result<()>` | `lib.rs:107` |
| `delete_node` | `fn delete_node(&mut self, node_id: NodeId) -> Result<NodeRecord>` | `lib.rs:117` |

#### Edge Operations

| Method | Signature | File:Line |
|--------|-----------|-----------|
| `create_edge` | `fn create_edge(&mut self, start: NodeId, end: NodeId, rel_type_id: u32, properties: Vec<(u32, PropertyValue)>) -> Result<EdgeId>` | `lib.rs:127` |
| `get_edge` | `fn get_edge(&self, edge_id: EdgeId) -> Option<&RelationshipRecord>` | `lib.rs:146` |
| `get_edges_for_node` | `fn get_edges_for_node(&self, node_id: NodeId) -> Vec<&RelationshipRecord>` | `lib.rs:151` |
| `delete_edge` | `fn delete_edge(&mut self, edge_id: EdgeId) -> Result<RelationshipRecord>` | `lib.rs:157` |

#### Transaction Operations

| Method | Signature | File:Line |
|--------|-----------|-----------|
| `begin_read` | `fn begin_read(&self) -> ReadTransaction` | `lib.rs:164` |
| `begin_write` | `fn begin_write(&self) -> Result<WriteTransaction>` | `lib.rs:169` |

#### WAL/Checkpoint Operations

| Method | Signature | File:Line |
|--------|-----------|-----------|
| `wal_write_page` | `fn wal_write_page(&mut self, page_id: PageId, data: &[u8; PAGE_SIZE]) -> Result<u64>` | `lib.rs:176` |
| `wal_commit` | `fn wal_commit(&mut self) -> Result<u64>` | `lib.rs:182` |
| `wal_discard` | `fn wal_discard(&mut self)` | `lib.rs:189` |
| `checkpoint` | `fn checkpoint(&mut self) -> Result<u64>` | `lib.rs:196` |
| `flush_header` | `fn flush_header(&mut self) -> Result<()>` | `lib.rs:207` |

#### Utility

| Method | Signature | File:Line |
|--------|-----------|-----------|
| `node_count` | `fn node_count(&self) -> usize` | `lib.rs:212` |
| `edge_count` | `fn edge_count(&self) -> usize` | `lib.rs:217` |
| `config` | `fn config(&self) -> &DatabaseConfig` | `lib.rs:222` |

### Sub-components (Internal but Relevant)

#### NodeStore (`btree/node_store.rs`)

- `iter() -> impl Iterator<Item = (&u64, &NodeRecord)>` — `node_store.rs:109` — full scan of all nodes
- `get_node_mut()` — mutable access by NodeId

#### EdgeStore (`btree/edge_store.rs`)

- `iter() -> impl Iterator<Item = (&u64, &RelationshipRecord)>` — `edge_store.rs:182` — full scan of all edges
- `get_edge_mut()` — mutable access by EdgeId

**Critical**: `NodeStore::iter()` and `EdgeStore::iter()` are the only full-scan interfaces. They are NOT exposed through `StorageEngine`'s public API. The query engine would need either new scan methods on `StorageEngine`, or direct internal access via a new module in cypherlite-storage.

#### BTree (`btree/mod.rs`)

Generic `BTree<K: Ord + Clone, V: Clone>`. Methods at `mod.rs:22-89`:
- `insert(key, value) -> Option<V>`
- `search(&key) -> Option<&V>`
- `search_mut(&key) -> Option<&mut V>`
- `delete(&key) -> Option<V>`
- `range_scan(start, end) -> Vec<(&K, &V)>` — `mod.rs:62` — key range scan with inclusive bounds
- `iter() -> impl Iterator<Item = (&K, &V)>` — `mod.rs:87`

#### TransactionManager (`transaction/mvcc.rs`)

- `new() -> Self` — `mvcc.rs:25`
- `begin_read() -> ReadTransaction` — `mvcc.rs:36`
- `begin_write() -> Result<WriteTransaction>` — `mvcc.rs:53`
- `update_current_frame(u64)` — `mvcc.rs:84`
- `current_frame() -> u64` — `mvcc.rs:89`

`TransactionManager` is `Send + Sync` (confirmed in `concurrency.rs:96-100`).

#### ReadTransaction (`mvcc.rs:104-120`)

- `tx_id() -> u64`
- `snapshot_frame() -> u64` (via TransactionView trait)

#### WriteTransaction (`mvcc.rs:129-163`)

- `tx_id() -> u64`
- `commit(new_frame: u64)` — marks committed, updates global frame
- `rollback(self)` — releases write lock, no commit
- `is_committed() -> bool`
- `snapshot_frame() -> u64` (via TransactionView trait)

---

## 4. Query Engine Integration Points

### Primary Entry Point

```rust
let engine = StorageEngine::open(DatabaseConfig { path: "mydb.cyl", .. })?;
```

`StorageEngine::open()` at `lib.rs:39` is the single entry point. The query engine receives or creates a `StorageEngine` instance and calls its public methods.

### Necessary New APIs (Gaps in Current Surface)

The current `StorageEngine` public API lacks two critical methods for query execution:

1. **Node label scan** — The query engine needs `MATCH (n:Person)` to enumerate all nodes with a given label. Currently, there is no `scan_nodes_by_label(label_id: u32) -> Vec<&NodeRecord>` method. This requires either:
   - Adding `pub fn scan_nodes(&self) -> impl Iterator<Item = &NodeRecord>` to `StorageEngine` (exposing `node_store.iter()`)
   - Adding `pub fn scan_nodes_by_label(&self, label_id: u32) -> Vec<&NodeRecord>` for filtered scans

2. **Edge type scan** — `MATCH ()-[:KNOWS]->()` needs `scan_edges_by_type(rel_type_id: u32) -> Vec<&RelationshipRecord>`. Currently only `get_edges_for_node()` exists (node-anchored traversal).

3. **All-nodes scan** — For `MATCH (n) RETURN n` (unfiltered), the engine needs a full node iterator.

These three methods need to be added to `cypherlite-storage`'s `StorageEngine` public API.

### Existing Traversal Entry Points

For node-anchored traversal (which covers most MATCH patterns):
```rust
// Get all edges for a node
let edges: Vec<&RelationshipRecord> = engine.get_edges_for_node(node_id);

// For each edge, access endpoint nodes
for edge in edges {
    let neighbor = engine.get_node(edge.end_node)?;
}
```

---

## 5. Transaction Model

### Isolation Level

Snapshot Isolation (MVCC). Implemented in `mvcc.rs`.

### Read Transactions

`begin_read()` (`lib.rs:164`) captures `current_frame` atomically at call time. Multiple concurrent readers are allowed. Readers are NEVER blocked by writers (`mvcc.rs:273-280`).

```rust
let tx = engine.begin_read();
// tx.snapshot_frame() is immutable for the tx lifetime
// Use tx.snapshot_frame() to determine which WAL frames are visible
```

**For query engine**: A read-only query (SELECT equivalent) should `begin_read()` at query start and use the snapshot_frame to maintain consistency across multi-step traversals.

### Write Transactions

`begin_write()` (`lib.rs:169`) acquires an exclusive mutex. Returns `Err(TransactionConflict)` if another write transaction is active.

```rust
let mut tx = engine.begin_write()?;
// ... perform mutations ...
engine.wal_commit()?;    // OR engine.wal_discard()
tx.commit(frame);        // update snapshot
// tx drops, releasing write lock
```

**Important**: The write lock is held by `WriteTransaction`'s `_guard` field until the `WriteTransaction` drops. The query engine must handle `TransactionConflict` errors and implement retry logic.

### Current Mutation Pattern (Phase 1 Limitation)

In the current Phase 1 implementation, mutations (`create_node`, `create_edge`, etc.) operate directly on the in-memory B-tree. They do NOT automatically write to the WAL. The caller must:
1. Call mutation methods
2. Call `wal_write_page()` for each modified page
3. Call `wal_commit()` to commit

This is a Phase 1 architectural pattern documented in `lib.rs:400`:
> "Node data is in-memory B-tree, so it won't persist across restarts without serialization."

The query engine must be designed with this limitation in mind: Phase 1 data is in-memory only. Persistence across restarts requires the upcoming B-tree serialization (likely Phase 2 scope).

### Concurrency Constraints

- Single writer at a time (enforced by `Arc<Mutex<()>>` in `mvcc.rs:18`)
- Multiple concurrent readers allowed
- `TransactionManager` is `Send + Sync` — safe for multi-threaded use
- `StorageEngine` itself has no `Send + Sync` bounds — single-threaded access assumed

---

## 6. Scan/Traversal Interfaces

### Currently Exposed via StorageEngine

| Method | Usage | Performance | File:Line |
|--------|-------|-------------|-----------|
| `get_node(NodeId)` | Point lookup | O(log n) | `lib.rs:102` |
| `get_edge(EdgeId)` | Point lookup | O(log n) | `lib.rs:146` |
| `get_edges_for_node(NodeId)` | Adjacency traversal | O(E) linear scan | `lib.rs:151` |

### Currently Internal (Need Exposure)

| Method | Location | Needed For |
|--------|----------|-----------|
| `node_store.iter()` | `node_store.rs:109` | Full node scan (`MATCH (n)`) |
| `edge_store.iter()` | `edge_store.rs:182` | Full edge scan |
| `btree.range_scan(start, end)` | `btree/mod.rs:62` | Property range queries |

### Adjacency Chain Traversal

The storage engine uses **Index-Free Adjacency** (`edge_store.rs:88-107`). `get_edges_for_node()` currently performs a **linear scan** of all edges and filters by node involvement. This is O(E) — not a proper chain walk.

From `edge_store.rs:93-106`:
```rust
// Since our adjacency chain uses a simple "head pointer" approach,
// we need to walk through all edges and filter by node involvement.
// In a full implementation, we'd follow the chain pointers.
self.tree.iter()
    .filter_map(|(_, record)| { ... })
    .collect()
```

The `next_out_edge` / `next_in_edge` pointers exist in `RelationshipRecord` (`types.rs:65-67`) but are not yet used for chain walking. This is a Phase 1 limitation. The query engine should use `get_edges_for_node()` as-is, knowing it is O(E).

### Range Scan Capability

`BTree::range_scan()` at `btree/mod.rs:62` takes inclusive `[start, end]` bounds. This can be used for:
- `WHERE n.age > 30` → `range_scan(&31, &u64::MAX)` after projecting property values
- Index range scans if a property index is built on top

However, this is only available internally. The query engine would need new `StorageEngine` methods like:
- `scan_nodes_with_label(label_id: u32) -> Vec<&NodeRecord>`
- `scan_nodes_by_property_range(key_id: u32, min: &PropertyValue, max: &PropertyValue) -> Vec<&NodeRecord>`

---

## 7. Test Patterns

### Standard Test Setup Pattern

From `crud_operations.rs:7-16`:
```rust
fn test_engine() -> (tempfile::TempDir, StorageEngine) {
    let dir = tempdir().expect("tempdir");
    let config = DatabaseConfig {
        path: dir.path().join("test.cyl"),
        wal_sync_mode: SyncMode::Normal,
        ..Default::default()
    };
    let engine = StorageEngine::open(config).expect("open");
    (dir, engine)
}
```
Key: `SyncMode::Normal` for tests (avoids fsync overhead). `TempDir` is returned to keep temp dir alive for test duration.

### Graph Construction Patterns in Tests

From `crud_operations.rs:170-188` (triangle test):
```rust
let a = engine.create_node(vec![1], vec![(1, PropertyValue::String("A".into()))]);
let b = engine.create_node(vec![1], vec![(1, PropertyValue::String("B".into()))]);
let c = engine.create_node(vec![1], vec![(1, PropertyValue::String("C".into()))]);
engine.create_edge(a, b, 1, vec![]).expect("ab");
engine.create_edge(b, c, 1, vec![]).expect("bc");
engine.create_edge(c, a, 1, vec![]).expect("ca");
```

### Label/Property Conventions in Tests

- Labels are small integers: `vec![1]`, `vec![10, 20]`
- Property keys are small integers: `(1, PropertyValue::String(...))`, `(2, PropertyValue::Int64(...))`
- Relationship type IDs are small integers: `rel_type_id = 1`, `42`, etc.

**Implication**: Query engine tests will need to maintain a `HashMap<String, u32>` symbol table. Example:
```rust
let mut labels = HashMap::new();
labels.insert("Person".to_string(), 1u32);
labels.insert("name".to_string(), 1u32);
```

### Transaction Test Pattern

From `acid_compliance.rs:63-75`:
```rust
let r1 = engine.begin_read();
assert_eq!(TransactionView::snapshot_frame(&r1), 0);
```

The `TransactionView` trait must be imported explicitly — it's not in scope by default.

### Concurrency Test Pattern

From `concurrency.rs:11-26`:
```rust
let tm = Arc::new(TransactionManager::new());
let tm1 = tm.clone();
let tm2 = tm.clone();
let w1 = tm1.begin_write().expect("first write");
let result = tm2.begin_write();
assert!(matches!(result, Err(CypherLiteError::TransactionConflict)));
```

Query engine tests that test concurrent queries should wrap `TransactionManager` (or `StorageEngine`) in `Arc`.

---

## 8. Constraints and Risks

### R1: Phase 1 In-Memory Only — No Cross-Restart Persistence

`lib.rs:400`:
> "Node data is in-memory B-tree, so it won't persist across restarts without serialization."

The graph data lives entirely in memory. Only page header (next_node_id, next_edge_id counters) persists to disk. The WAL exists for page-level durability but the B-tree is not serialized to pages yet.

**Impact for SPEC-DB-002**: Query tests that restart the engine will see an empty graph. All query integration tests must create their test graph within the same engine session.

### R2: No String Interning — Symbol Table Required

Labels (`Vec<u32>`), property keys (`u32`), and relationship type IDs (`u32`) are stored as integers. There is no built-in string registry.

**Impact**: The query engine must own a `SymbolTable` (or `SchemaRegistry`) that maps strings to integer IDs and back. This is a hard dependency for correct Cypher query execution.

### R3: No Native Secondary Index

There is no label index or property index. `MATCH (n:Person)` requires a full scan of all nodes followed by label filtering. For large graphs, this is O(N).

**Impact**: SPEC-DB-002 may need to specify:
- Acceptable: O(N) label scan for initial implementation
- Future work: Secondary index for production use

### R4: get_edges_for_node is O(E) Not O(degree)

`edge_store.rs:93-106` performs a full scan of all edges. This is a known Phase 1 limitation (comment in code).

**Impact**: For graph traversal patterns like `MATCH (a)-[r]->(b)`, performance degrades linearly with total edge count, not node degree. This is acceptable for Phase 1 scope.

### R5: Single Write Transaction Only

`begin_write()` enforces single-writer exclusion. The query engine must:
- Return errors when a write transaction is requested while another is active
- Implement retry semantics or queue writes

### R6: No PartialOrd on PropertyValue

`PropertyValue` does not implement `PartialOrd` or `Ord`. Comparison operators (`<`, `<=`, `>`, `>=`) in `WHERE` clauses require custom comparison logic.

**Impact**: The query evaluator must implement typed comparison: `Int64 < Int64`, `String < String` (lexicographic), with type mismatch errors.

### R7: Float64 Equality

`PropertyValue::Float64` implements `PartialEq` (via `#[derive(PartialEq)]` on the enum). IEEE 754 float equality semantics apply. `NaN != NaN`.

### R8: StorageEngine is Not Send

`StorageEngine` does not implement `Send` or `Sync`. The query engine must be single-threaded or wrap `StorageEngine` in a `Mutex` for multi-threaded use.

### R9: MX Warning on WriteTransaction Lifetime Transmute

`mvcc.rs:48-52` has an `@MX:WARN` annotation:
> "Uses unsafe transmute to extend MutexGuard lifetime to 'static."

The field ordering invariant (`_guard` before `_write_lock_arc`) is safety-critical. The query engine must not modify `WriteTransaction` struct field ordering without understanding this invariant.

---

## 9. Recommendations

### R1: Add Scan Methods to StorageEngine Public API

Add to `crates/cypherlite-storage/src/lib.rs`:
```rust
/// Iterate over all nodes.
pub fn scan_nodes(&self) -> impl Iterator<Item = &NodeRecord>;

/// Iterate over all nodes with a given label.
pub fn scan_nodes_by_label(&self, label_id: u32) -> Vec<&NodeRecord>;

/// Iterate over all edges.
pub fn scan_edges(&self) -> impl Iterator<Item = &RelationshipRecord>;

/// Iterate over all edges with a given relationship type.
pub fn scan_edges_by_type(&self, rel_type_id: u32) -> Vec<&RelationshipRecord>;
```

These are the minimum additions needed by the query executor for `MATCH` clause implementation.

### R2: Build SymbolTable in cypherlite-query

The query engine crate must own a `SymbolTable` mapping `String <-> u32` for labels, property keys, and relationship types. This is a hard architectural requirement derived from how the storage engine stores numeric IDs.

Suggested structure:
```rust
pub struct SymbolTable {
    labels: BiMap<String, u32>,
    rel_types: BiMap<String, u32>,
    property_keys: BiMap<String, u32>,
    next_id: u32,
}
```

### R3: Use MATCH (n) + filter as the query execution strategy

Given no secondary indexes, all MATCH patterns must start with:
1. `scan_nodes_by_label(label_id)` → filter candidates
2. For each candidate, traverse edges via `get_edges_for_node()`
3. Apply WHERE predicate

This is a nested-loop join strategy — correct and simple for Phase 1.

### R4: PropertyValue Comparison Module

Create a `cypherlite-query/src/eval/comparison.rs` module with typed comparison:
```rust
fn compare(left: &PropertyValue, right: &PropertyValue, op: ComparisonOp) -> Result<bool, QueryError>
```

### R5: Transaction Integration Strategy

For read-only queries: call `engine.begin_read()` → execute traversal → drop tx.
For write queries (CREATE, SET, DELETE): call `engine.begin_write()?` → execute mutations → call `engine.wal_commit()?` → call `tx.commit(frame)`.

The query engine's `execute()` function signature should look like:
```rust
pub fn execute(engine: &mut StorageEngine, query: &str) -> Result<QueryResult, QueryError>
```

### R6: Test Fixture Pattern

All query engine integration tests should use the same `test_engine()` pattern from `crud_operations.rs:7-16` with `SyncMode::Normal`. Keep `TempDir` alive for the test scope.

### R7: Reference Implementation for Parse-Execute Pattern

No parser or AST code exists in the current codebase. The query engine will be the first parser. Consider using `nom` or `pest` for the openCypher subset parser. No visitor pattern or AST structures exist to reference — these must be designed from scratch.

### R8: Add cypherlite-query to Workspace

The workspace `Cargo.toml` must be updated to include the new crate:
```toml
members = [
    "crates/cypherlite-core",
    "crates/cypherlite-storage",
    "crates/cypherlite-query",  # new
]
```
