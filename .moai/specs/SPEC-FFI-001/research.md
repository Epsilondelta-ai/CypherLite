# FFI Bindings Research Document (SPEC-FFI-001)

## 1. Public API Surface Analysis

### 1.1 Main Entry Point: CypherLite Facade
**File**: `crates/cypherlite-query/src/api/mod.rs` (471 lines)

The `CypherLite` struct is the primary public API (@MX:ANCHOR, fan_in >= 3).

**Core Methods**:
- `CypherLite::open(config: DatabaseConfig) -> Result<Self>` — Creates/opens database
- `execute(&mut self, query: &str) -> Result<QueryResult>` — Execute Cypher query
- `execute_with_params(&mut self, query, params) -> Result<QueryResult>` — Parameterized queries
- `engine(&self) -> &StorageEngine` — Read access to storage
- `engine_mut(&mut self) -> &mut StorageEngine` — Write access to storage
- `begin(&mut self) -> Transaction<'_>` — Start transaction

**Plugin Methods** (feature-gated `plugin`):
- `register_scalar_function(func) -> Result<()>`
- `register_index_plugin(plugin) -> Result<()>`
- `register_serializer(serializer) -> Result<()>`
- `register_trigger(trigger) -> Result<()>`
- `export_data(format, query) -> Result<Vec<u8>>`
- `import_data(format, bytes) -> Result<Vec<HashMap<...>>>`
- `list_scalar_functions()`, `list_index_plugins()`, `list_serializers()`, `list_triggers()`

### 1.2 Query Result Types

**QueryResult**: `{ columns: Vec<String>, rows: Vec<Row> }`
**Row**: `{ values: HashMap<String, Value>, columns: Vec<String> }`
- Methods: `get(column) -> Option<&Value>`, `get_as<T: FromValue>(column) -> Option<T>`
- FromValue impl: i64, f64, String, bool

### 1.3 Transaction Type

**Transaction<'a>**: `{ db: &'a mut CypherLite, committed: bool }`
- `execute()`, `execute_with_params()`, `commit()`, `rollback()`
- Auto-rollback on Drop if not committed

## 2. Type System Analysis

### 2.1 Value Enum (Query Runtime)
```rust
pub enum Value {
    Null, Bool(bool), Int64(i64), Float64(f64), String(String),
    Bytes(Vec<u8>), List(Vec<Value>), Node(NodeId), Edge(EdgeId),
    DateTime(i64),
    #[cfg(feature = "subgraph")] Subgraph(SubgraphId),
    #[cfg(feature = "hypergraph")] Hyperedge(HyperEdgeId),
    #[cfg(feature = "hypergraph")] TemporalNode(NodeId, i64),
}
```

### 2.2 Core Identifiers (all Copy + Clone + Send + Sync)
- `NodeId(pub u64)`, `EdgeId(pub u64)`, `PageId(pub u32)`
- `SubgraphId(pub u64)` (feature="subgraph")
- `HyperEdgeId(pub u64)` (feature="hypergraph")

### 2.3 PropertyValue Enum (Storage Layer)
```rust
pub enum PropertyValue {
    Null, Bool(bool), Int64(i64), Float64(f64),
    String(String), Bytes(Vec<u8>), Array(Vec<PropertyValue>), DateTime(i64),
}
```

### 2.4 Error Type: CypherLiteError (thiserror)
- IoError, CorruptedPage, TransactionConflict, OutOfSpace
- InvalidMagicNumber, UnsupportedVersion, ChecksumMismatch, SerializationError
- NodeNotFound, EdgeNotFound, ParseError, SemanticError, ExecutionError
- UnsupportedSyntax, ConstraintViolation, InvalidDateTimeFormat
- Feature-gated: SubgraphNotFound, HyperEdgeNotFound, PluginError, FunctionNotFound

### 2.5 Records
- **NodeRecord**: node_id, labels: Vec<u32>, properties: Vec<(u32, PropertyValue)>
- **RelationshipRecord**: edge_id, start_node, end_node, rel_type_id, direction, properties
- **SubgraphRecord**: subgraph_id, temporal_anchor, properties
- **HyperEdgeRecord**: id, rel_type_id, sources: Vec<GraphEntity>, targets: Vec<GraphEntity>, properties

## 3. Configuration
**DatabaseConfig**: path, page_size, cache_capacity, wal_sync_mode, temporal_tracking_enabled, version_storage_enabled
**SyncMode**: Full, Normal

## 4. Feature Flags
```toml
temporal-core (default), temporal-edge, subgraph, hypergraph, full-temporal, plugin
```
Chain: temporal-core → temporal-edge → subgraph → hypergraph → full-temporal. plugin is independent.

## 5. Plugin System (feature="plugin")
- **Plugin** base trait: name(), version() — requires Send + Sync
- **ScalarFunction**: call(&[PropertyValue]) -> Result<PropertyValue>
- **IndexPlugin**: insert/remove/lookup by PropertyValue + NodeId
- **Serializer**: export/import data as Vec<u8>
- **Trigger**: on_before/after_create/update/delete(TriggerContext)
- **PluginRegistry<T>**: register, get, get_mut, list, contains

## 6. Storage Engine Public API
- Node CRUD: create_node, get_node, update_node, delete_node
- Edge CRUD: create_edge, get_edge, update_edge, delete_edge
- Scans: scan_nodes, scan_nodes_by_label, scan_edges_by_type
- Transactions: begin_read, begin_write, wal_commit, wal_discard
- Metadata: node_count, edge_count, config
- LabelRegistry trait: get_or_create_label/rel_type/prop_key, lookups by id/name

## 7. Concurrency Model
- parking_lot::RwLock for concurrent reads
- Single-writer model (transaction conflict on second write)
- MVCC with snapshot isolation

## 8. Existing FFI Patterns
- **None found**: No cbindgen, uniffi, pyo3, neon dependencies exist yet
- Project docs mention cypherlite-ffi, cypherlite-python, cypherlite-node as planned

## 9. Dependencies
- **core**: thiserror 2, serde 1, bincode 1
- **storage**: core, parking_lot 0.12, bincode 1, serde 1
- **query**: core, storage, logos 0.14

## 10. FFI Strategy Recommendations

### Priority 1 — C ABI Layer (cypherlite-ffi crate):
- Opaque pointers: `*mut CypherLite`, `*mut Transaction`
- CString for query input, null-terminated error messages
- Value as tagged union (u8 tag + payload)
- Collections as pointer + length pairs
- Error codes + error_out pointer pattern

### Priority 2 — Go Bindings:
- CGo wrapping the C header
- Go-native error handling from C error codes

### Priority 3 — Python Bindings (pyo3):
- Direct Rust-Python via PyO3 (no C intermediate needed)
- Pythonic API with context managers for transactions

### Priority 4 — Node.js Bindings (napi-rs or neon):
- N-API for ABI stability across Node versions
- Promise-based async API
