# SPEC-PERSIST-001: Data Persistence Layer

| Field | Value |
|-------|-------|
| **SPEC ID** | SPEC-PERSIST-001 |
| **Title** | Data Persistence — WAL-Based Durable Storage for All Graph Data |
| **Created** | 2026-03-16 |
| **Status** | Planned |
| **Priority** | Critical (P0) |
| **Target Version** | v2.0.0 |

---

## 1. Problem Statement

CypherLite stores ALL graph data (nodes, edges, properties, catalog, indexes) in in-memory BTreeMap structures. Data is lost when the database is closed and reopened. This is a fundamental violation of the "D" (Durability) in ACID and contradicts the product's positioning as an "embedded database."

### Impact
- `db.close()` → `db.open()` = all data lost
- Process crash = all data lost
- Users expect SQLite-like persistence but get in-memory-only behavior
- README, PyPI, npm all promise a database, not an in-memory cache

---

## 2. Architecture Overview

### Current Data Flow (BROKEN)
```
execute("CREATE ...") → NodeStore.create_node() → BTreeMap.insert() → MEMORY ONLY
                                                                        ↓
                                                              close() → LOST
```

### Target Data Flow (FIXED)
```
execute("CREATE ...") → NodeStore.create_node() → BTreeMap.insert() → MEMORY
                                                        ↓
                                              serialize to page(s) → WAL write
                                                        ↓
                                              commit() → WAL fsync → DURABLE
                                                        ↓
                                              checkpoint → main .cyl file
                                                        ↓
                                              open() → read pages → rebuild BTreeMap
```

### Design Decision: Page-Serialized WAL Approach

Each mutation (create/update/delete) serializes affected records into page format and writes through the existing WAL. On startup, pages are read from the main file to rebuild in-memory indexes.

**Why this approach:**
- Reuses existing PageManager, WAL, checkpoint, recovery infrastructure
- No new file formats needed
- WAL provides crash recovery for free
- Consistent with SQLite's architecture

---

## 3. Requirements (EARS Format)

### 3.1 Core Persistence [R-PERSIST-001 ~ R-PERSIST-006]

**R-PERSIST-001** [Ubiquitous]
WHEN a node is created via `create_node()` THEN it MUST be serialized to a data page and written through WAL before the operation returns.

**R-PERSIST-002** [Ubiquitous]
WHEN an edge is created via `create_edge()` THEN it MUST be serialized to a data page and written through WAL before the operation returns.

**R-PERSIST-003** [Ubiquitous]
WHEN a node or edge is updated via `update_node()`/`update_edge_properties()` THEN the updated record MUST be re-serialized and written through WAL.

**R-PERSIST-004** [Ubiquitous]
WHEN a node or edge is deleted THEN the deletion MUST be recorded through WAL (page freed or tombstone written).

**R-PERSIST-005** [Event-Driven]
WHEN the database is opened with `StorageEngine::open()` THEN ALL previously committed nodes and edges MUST be loaded from disk pages into the in-memory BTreeMap.

**R-PERSIST-006** [Event-Driven]
WHEN the database is closed (Drop) THEN checkpoint MUST flush all WAL frames to the main file, and the WAL file MUST be deleted only on success.

### 3.2 Catalog Persistence [R-PERSIST-010 ~ R-PERSIST-012]

**R-PERSIST-010** [Ubiquitous]
The Catalog (label names, property key names, relationship type names) MUST be persisted to a reserved page range.

**R-PERSIST-011** [Event-Driven]
WHEN a new label, property key, or relationship type is registered THEN the catalog page MUST be updated through WAL.

**R-PERSIST-012** [Event-Driven]
WHEN the database is opened THEN the catalog MUST be loaded from its reserved page(s).

### 3.3 Page Layout [R-PERSIST-020 ~ R-PERSIST-025]

**R-PERSIST-020** [Ubiquitous]
Each data page (4096 bytes) MUST begin with a page header:
- `page_type` (u8): 0=free, 1=node_data, 2=edge_data, 3=catalog, 4=overflow
- `record_count` (u16): number of records in this page
- `free_offset` (u16): byte offset to free space
- `next_page` (u32): overflow page ID (0 = no overflow)

**R-PERSIST-021** [Ubiquitous]
Each node record in a page MUST be serialized as:
- `node_id` (u64)
- `label_count` (u16) + labels (u32 each)
- `prop_count` (u16) + properties (key_id u32 + PropertyValue)
- `flags` (u8): 0x01=deleted (tombstone)

**R-PERSIST-022** [Ubiquitous]
Each edge record in a page MUST be serialized as:
- `edge_id` (u64)
- `source_id` (u64)
- `target_id` (u64)
- `rel_type_id` (u32)
- `prop_count` (u16) + properties
- `flags` (u8): 0x01=deleted

**R-PERSIST-023** [State-Driven]
IF a record exceeds the available space in its data page THEN it MUST be split across overflow pages linked by `next_page`.

**R-PERSIST-024** [Ubiquitous]
PropertyValue serialization MUST use the existing `property_store.rs` format:
- Tag byte (0=Null, 1=Bool, 2=Int64, 3=Float64, 4=String, 5=Bytes, 6=Array)
- Length-prefixed data for variable-size types

**R-PERSIST-025** [Ubiquitous]
The database header MUST be extended with:
- `catalog_page_id` (u32): starting page for catalog data
- `node_data_root_page` (u32): starting page for node data
- `edge_data_root_page` (u32): starting page for edge data

### 3.4 Startup Recovery [R-PERSIST-030 ~ R-PERSIST-032]

**R-PERSIST-030** [Event-Driven]
WHEN the database is opened THEN WAL recovery MUST run first (replay uncommitted frames).

**R-PERSIST-031** [Event-Driven]
AFTER WAL recovery THEN all node data pages MUST be read and deserialized into NodeStore BTreeMap.

**R-PERSIST-032** [Event-Driven]
AFTER WAL recovery THEN all edge data pages MUST be read and deserialized into EdgeStore BTreeMap.

### 3.5 Verification [R-PERSIST-040 ~ R-PERSIST-044]

**R-PERSIST-040** [Event-Driven]
WHEN data is created, the database is closed, and reopened THEN all previously created data MUST be queryable.

**R-PERSIST-041** [Event-Driven]
WHEN `cargo test --workspace --all-features` is run THEN all existing 1,490 tests MUST still pass (backward compatibility).

**R-PERSIST-042** [Event-Driven]
WHEN the database process is killed (SIGKILL) during a write THEN reopening MUST recover all committed data via WAL replay.

**R-PERSIST-043** [Event-Driven]
WHEN `cargo run -p cypherlite-query --example basic_crud --all-features` runs, closes, and reopens THEN created data MUST be present.

**R-PERSIST-044** [Ubiquitous]
Test coverage for persistence operations MUST be >= 85%.

### 3.6 Feature Flag Persistence [R-PERSIST-050 ~ R-PERSIST-053]

**R-PERSIST-050** [State-Driven]
IF `subgraph` feature is enabled THEN SubgraphStore records MUST be persisted using the same page mechanism.

**R-PERSIST-051** [State-Driven]
IF `hypergraph` feature is enabled THEN HyperEdgeStore records MUST be persisted.

**R-PERSIST-052** [State-Driven]
IF `temporal-core` feature is enabled THEN VersionStore history MUST be persisted.

**R-PERSIST-053** [State-Driven]
IF `plugin` feature is enabled THEN plugin-created data MUST be persisted (plugins themselves are registered at runtime).

---

## 4. Implementation Plan

### Phase 1: Record Serialization (cypherlite-storage)
- Implement `NodeRecord::serialize()` / `NodeRecord::deserialize()` into byte arrays
- Implement `RelationshipRecord::serialize()` / `RelationshipRecord::deserialize()`
- Extend `PropertyStore` serialization for full PropertyValue round-trip
- Add page header struct with type, count, free offset, overflow pointer
- Unit tests: serialize → deserialize round-trip for all record types

### Phase 2: Page-Based Write Path
- Extend `StorageEngine::create_node()` to serialize record → allocate page → WAL write
- Extend `StorageEngine::create_edge()` similarly
- Extend `update_node()` / `delete_node()` / `update_edge_properties()` / `delete_edge()`
- Handle overflow pages for large records
- Extend database header with root page pointers

### Phase 3: Startup Load Path
- Implement `StorageEngine::load_nodes_from_pages()` — scan node pages, deserialize, populate BTreeMap
- Implement `StorageEngine::load_edges_from_pages()` — same for edges
- Call from `StorageEngine::open()` after WAL recovery
- Rebuild in-memory indexes from loaded data

### Phase 4: Catalog Persistence
- Reserve page(s) for catalog data
- Serialize/deserialize catalog on open/close
- Update catalog page through WAL on new label/property/reltype registration

### Phase 5: Feature-Gated Stores
- SubgraphStore persistence (cfg subgraph)
- HyperedgeStore persistence (cfg hypergraph)
- VersionStore persistence (cfg temporal-core)

### Phase 6: Integration Tests
- Close/reopen persistence test (the KEY test)
- Crash recovery test (kill process, reopen)
- Large dataset persistence test (10K+ nodes)
- All existing tests must still pass

---

## 5. File Impact

| File | Change Type | Description |
|------|------------|-------------|
| `crates/cypherlite-core/src/lib.rs` | Modify | Add Serialize/Deserialize for NodeRecord, EdgeRecord |
| `crates/cypherlite-storage/src/btree/mod.rs` | Modify | Integrate page backing into BTree |
| `crates/cypherlite-storage/src/btree/node_store.rs` | Modify | Add page write on create/update/delete |
| `crates/cypherlite-storage/src/btree/edge_store.rs` | Modify | Same as node_store |
| `crates/cypherlite-storage/src/btree/property_store.rs` | Modify | Complete serialize/deserialize for all PropertyValue types |
| `crates/cypherlite-storage/src/lib.rs` | Modify | Integrate WAL writes in CRUD, add load_from_pages in open() |
| `crates/cypherlite-storage/src/page/mod.rs` | Modify | Add DataPageHeader, page type constants |
| `crates/cypherlite-storage/src/page/page_manager.rs` | Modify | Extend header with root page pointers |
| `crates/cypherlite-storage/src/catalog/mod.rs` | Modify | Wire save()/load() into open/close cycle |
| `crates/cypherlite-storage/src/version/mod.rs` | Modify | Add persistence (cfg temporal-core) |
| `crates/cypherlite-storage/src/subgraph/mod.rs` | Modify | Add persistence (cfg subgraph) |
| `crates/cypherlite-storage/src/hyperedge/mod.rs` | Modify | Add persistence (cfg hypergraph) |

---

## 6. Risks

| Risk | Impact | Probability | Mitigation |
|------|--------|-------------|-----------|
| Performance regression (WAL write on every mutation) | High | Medium | Batch WAL writes, buffer pool optimization |
| Page format breaking change | High | Low | New header field = v2 format, add migration path |
| Overflow page complexity | Medium | Medium | Start with max record size limit, add overflow in Phase 2 |
| Existing test breakage | High | Medium | Run tests continuously during development |
| File format not backward compatible | Medium | High | Bump header version to 5, detect old format on open |

---

## 7. Definition of Done

- [ ] `db.close()` → `db.open()` preserves ALL created nodes and edges
- [ ] WAL crash recovery restores committed data
- [ ] Catalog persists across restarts (labels, property keys, rel types)
- [ ] All 1,490 existing tests pass
- [ ] New persistence tests pass (close/reopen, crash recovery, large dataset)
- [ ] Examples (basic_crud, knowledge_graph) work across restarts
- [ ] Feature-gated stores (subgraph, hyperedge, version) persist when enabled
- [ ] `cargo test --workspace --all-features` passes with 85%+ coverage
- [ ] Performance: < 2x slowdown for write operations vs current in-memory-only
