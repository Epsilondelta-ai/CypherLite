# SPEC-PERSIST-001 Research: Data Persistence Gap Analysis

## Critical Finding

ALL graph data (nodes, edges, properties, catalog, version history, indexes) exists ONLY in memory.
Closing and reopening the database loses all data. The existing test at lib.rs:971 confirms this:
```rust
// Node data is in-memory B-tree, so it won't persist across restarts
assert_eq!(engine.node_count(), 0); // in-memory only for Phase 1
```

## Component Persistence Status

| Component | Data Structure | Persisted? | Mechanism |
|-----------|---------------|-----------|-----------|
| NodeStore | BTreeMap<u64, NodeRecord> | NO | In-memory only |
| EdgeStore | BTreeMap<u64, RelationshipRecord> | NO | In-memory only |
| Catalog | BTreeMap<String, u32> x3 | NO | Has save()/load() but never called |
| VersionStore | BTreeMap<(u64,u64), VersionRecord> | NO | In-memory only |
| IndexManager | HashMap of BTreeMap indexes | NO | In-memory only |
| SubgraphStore | BTreeMap<u64, SubgraphRecord> | NO | In-memory only |
| HyperedgeStore | BTreeMap<u64, HyperEdgeRecord> | NO | In-memory only |
| Database Header | Fixed struct | YES | PageManager read/write |
| Free Space Map | Bitmap in header | YES | PageManager |
| WAL | Frame log file | YES | .cyl-wal file (but never populated by queries) |

## Existing Infrastructure That Works

1. **PageManager**: read_page/write_page/allocate_page - functional
2. **WAL Writer**: write_frame/commit/reset - functional
3. **WAL Reader**: index_frame/lookup - functional
4. **Checkpoint**: copies WAL frames to main file - functional
5. **Recovery**: replays WAL on startup - functional
6. **PropertyStore**: has serialize/deserialize for PropertyValue - EXISTS but unused
7. **Catalog**: has save()/load() with bincode - EXISTS but never called

## The Missing Link

StorageEngine.create_node() → NodeStore.create_node() → BTreeMap.insert()
                                                          ↑ STOPS HERE
                                                     Never reaches:
                                                     - page serialization
                                                     - WAL write
                                                     - disk I/O

## Design Constraints

- PAGE_SIZE = 4096 bytes (fixed)
- NodeRecord: variable size (labels vec + properties vec)
- EdgeRecord: variable size (properties vec)
- Must maintain backward compatibility with existing .cyl header format
- Must not break existing 1,490 tests
- Must work with all feature flags (temporal, subgraph, hypergraph, plugin)
