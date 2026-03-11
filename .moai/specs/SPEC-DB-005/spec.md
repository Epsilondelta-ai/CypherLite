---
id: SPEC-DB-005
version: "0.5.0"
status: draft
created: "2026-03-11"
updated: "2026-03-11"
author: epsilondelta
priority: P0
tags: [temporal-edge, feature-flags, edge-validity, at-time-edge, temporal-filter]
lifecycle: spec-anchored
depends_on: [SPEC-DB-004]
---

# SPEC-DB-005: CypherLite Phase 5 - Temporal Edge Validity & Feature Flags (v0.5)

> Extend CypherLite's temporal model to edges with native validity periods (_valid_from, _valid_to), introduce Cargo feature flags for modular temporal capabilities, add edge property indexes, and extend AT TIME / BETWEEN TIME semantics to filter both nodes and edges.

---

## HISTORY

| Version | Date | Changes |
|---------|------|---------|
| 0.5.0 | 2026-03-11 | Initial SPEC based on temporal hypergraph analysis |

---

## 1. Environment

### 1.1 System Environment

- **Language**: Rust 1.84+ (Edition 2021)
- **MSRV**: 1.84
- **Target platforms**: Linux (x86_64), macOS (x86_64, aarch64), Windows (x86_64), WASM
- **Execution model**: Synchronous, single-threaded, WASM-compatible

### 1.2 Crate Structure

- **Extended crate**: `cypherlite-core` -- Edge temporal types, feature flag conditional compilation
- **Extended crate**: `cypherlite-storage` -- Edge property indexes, temporal edge filtering in EdgeStore
- **Extended crate**: `cypherlite-query` -- AT TIME/BETWEEN TIME extended to edges, Expand operator temporal filtering
- **New external dependencies**: None

### 1.3 Feature Flags Architecture (NEW)

```toml
[features]
default = ["temporal-core"]
temporal-core = []                     # v0.4: DateTime, VersionStore, AT TIME for nodes
temporal-edge = ["temporal-core"]      # v0.5: Edge validity periods, AT TIME for edges
subgraph = ["temporal-edge"]           # v0.6: Subgraph entities, SNAPSHOT syntax
hypergraph = ["subgraph"]             # v0.7: Native hyperedges, HYPEREDGE syntax
full-temporal = ["hypergraph"]         # All temporal features
```

### 1.4 Database Header Feature Flags

```
Offset 44-47: feature_flags (u32 bitmask)
  bit 0: temporal-core (v0.4)
  bit 1: temporal-edge (v0.5)
  bit 2: subgraph (v0.6)
  bit 3: hypergraph (v0.7)
```

Compatibility rules:
- Opening a DB with features not compiled in: clear error message
- Opening a DB without features that are compiled in: OK (features unused)

---

## 2. Requirements (EARS Format)

### Group AA: Feature Flags Foundation

**AA-001**: When the `temporal-core` feature flag is enabled, the system SHALL compile all v0.4 temporal functionality (DateTime, VersionStore, AT TIME for nodes).

**AA-002**: When the `temporal-edge` feature flag is enabled, the system SHALL compile temporal edge validity functionality on top of `temporal-core`.

**AA-003**: When a database file is opened, the system SHALL read the `feature_flags` field from the DatabaseHeader and verify that all required features are compiled in.

**AA-004**: If a database file requires features not present in the current build, the system SHALL return a descriptive error: "Database requires feature '{name}'. Recompile with features = [\"{name}\"]".

**AA-005**: When writing to a database, the system SHALL set the appropriate feature flag bits in the DatabaseHeader based on compiled features.

### Group BB: Edge Temporal Properties

**BB-001**: When the `temporal-edge` feature is enabled and a relationship is created, the system SHALL accept optional `_valid_from` and `_valid_to` DateTime properties.

**BB-002**: When a relationship is created with `_valid_from` but without `_valid_to`, the system SHALL treat the edge as currently valid (open-ended validity).

**BB-003**: When a relationship is created without `_valid_from`, the system SHALL auto-inject `_valid_from` as the current timestamp (same as `_created_at` for nodes).

**BB-004**: The system SHALL auto-inject `_created_at` and `_updated_at` system properties on edges, consistent with node timestamp behavior.

**BB-005**: When a user attempts to directly SET `_valid_from` or `_valid_to` on an edge, the system SHALL allow it (unlike `_created_at`/`_updated_at` which are system-managed).

### Group CC: Edge Property Indexes

**CC-001**: The system SHALL support property indexes on edges, keyed by `(rel_type_id, prop_key_id)`.

**CC-002**: When `CREATE INDEX ON :REL_TYPE(prop_name)` is executed with a relationship type, the system SHALL create an edge property index.

**CC-003**: The edge property index SHALL support range queries using the existing PropertyValueKey ordering.

**CC-004**: When an edge property is modified via SET, the system SHALL automatically update all applicable edge indexes.

**CC-005**: When `DROP INDEX ON :REL_TYPE(prop_name)` is executed, the system SHALL remove the edge property index.

### Group DD: AT TIME / BETWEEN TIME for Edges

**DD-001**: When `MATCH (n)-[r:TYPE]->(m) AT TIME <expr>` is executed, the system SHALL filter edges such that only edges where `_valid_from <= timestamp` AND (`_valid_to IS NULL` OR `_valid_to > timestamp`) are returned.

**DD-002**: When `MATCH (n)-[r:TYPE]->(m) BETWEEN TIME <start> AND <end>` is executed, the system SHALL return all edges whose validity period overlaps with [start, end].

**DD-003**: When AT TIME is applied, both nodes AND edges SHALL be filtered by temporal validity.

**DD-004**: When an edge has no `_valid_from` property (pre-v0.5 data), the system SHALL treat it as always valid (backward compatibility).

**DD-005**: The Expand operator SHALL accept an optional temporal filter parameter and apply it during edge traversal.

**DD-006**: The VarLengthExpand operator SHALL accept an optional temporal filter and only traverse edges valid at the specified time.

### Group EE: Temporal Path Queries

**EE-001**: When a variable-length path query includes AT TIME, the system SHALL only traverse edges that are valid at the specified timestamp.

**EE-002**: When a variable-length path query includes BETWEEN TIME, the system SHALL only traverse edges whose validity period overlaps with the specified range.

**EE-003**: The system SHALL support temporal continuity in paths: for a path (n1)-[r1]->(n2)-[r2]->(n3) with AT TIME T, all edges r1 and r2 must be valid at time T.

### Group FF: Quality & Migration

**FF-001**: The system SHALL pass `cargo clippy --workspace --all-targets -- -D warnings` with zero warnings.

**FF-002**: The system SHALL include proptest invariants for temporal edge validity.

**FF-003**: The system SHALL include criterion benchmarks for temporal edge filtering.

**FF-004**: Existing databases (v0.4, FORMAT_VERSION=2) SHALL open without error, with `feature_flags=0x01` (temporal-core only) auto-set.

**FF-005**: The system SHALL bump all crate versions to 0.5.0.

---

## 3. Non-Goals for v0.5

- Subgraph entities (deferred to SPEC-DB-006)
- Hyperedge support (deferred to SPEC-DB-007)
- Bitemporal queries (valid time + transaction time)
- Temporal relationship creation via MERGE
- Edge version history in VersionStore (edge _created_at/_updated_at suffice)

---

## 4. Architecture Design

### 4.1 Feature Flag Conditional Compilation

```rust
// cypherlite-storage/src/lib.rs
pub struct StorageEngine {
    // Always present
    node_store: NodeStore,
    edge_store: EdgeStore,
    version_store: VersionStore,
    index_manager: IndexManager,

    #[cfg(feature = "temporal-edge")]
    edge_index_manager: EdgeIndexManager,  // NEW
}
```

### 4.2 Edge Index Manager

```rust
// cypherlite-storage/src/index/edge_index.rs
pub struct EdgeIndexManager {
    indexes: HashMap<(u32, u32), PropertyIndex>,  // (rel_type_id, prop_key_id)
}
```

Reuses existing PropertyIndex infrastructure. Only difference is key space is `(rel_type_id, prop_key_id)` instead of `(label_id, prop_key_id)`.

### 4.3 Temporal Edge Filtering in Expand

```rust
// cypherlite-query/src/executor/operators/expand.rs
fn execute_expand(
    // ... existing params ...
    #[cfg(feature = "temporal-edge")]
    temporal_filter: Option<TemporalFilter>,
) {
    let edges = engine.get_edges_for_node(src_node_id);
    for edge in edges {
        if let Some(tid) = rel_type_id {
            if edge.rel_type_id != tid { continue; }
        }
        #[cfg(feature = "temporal-edge")]
        if let Some(ref tf) = temporal_filter {
            if !is_edge_temporally_valid(&edge, tf, engine) { continue; }
        }
        // ... rest of expand logic
    }
}
```

### 4.4 DatabaseHeader v3

```
Bytes 0-3:   magic ("CYLT")
Bytes 4-7:   format_version (3)
Bytes 8-11:  page_count
Bytes 12-19: root_node_page
Bytes 20-27: root_edge_page
Bytes 28-31: next_node_id
Bytes 32-35: next_edge_id
Bytes 36-43: version_store_root_page (v0.4)
Bytes 44-47: feature_flags (v0.5, u32 bitmask)
Bytes 48-55: edge_index_root_page (v0.5)
```

Migration: v2 -> v3 auto-migration on open, feature_flags defaults to 0x01.

---

## 5. Task Decomposition

### Group AA: Feature Flags Foundation (4 tasks)
### Group BB: Edge Temporal Properties (5 tasks)
### Group CC: Edge Property Indexes (5 tasks)
### Group DD: AT TIME / BETWEEN TIME for Edges (6 tasks)
### Group EE: Temporal Path Queries (3 tasks)
### Group FF: Quality & Migration (5 tasks)

**Total: 28 tasks, ~20 files modified**

---

## 6. Risk Assessment

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| Feature flag cfg complexity | Medium | Medium | Minimize cfg boundaries; test with all flag combinations |
| Edge index storage growth | Medium | Low | Indexes are opt-in via CREATE INDEX |
| Backward compatibility (v0.4 DB) | Medium | High | Auto-migration with feature_flags=0x01 default |
| Expand operator performance regression | Low | High | Benchmark before/after; temporal filter is O(1) check |
| VarLengthExpand temporal correctness | Medium | Medium | Proptest with temporal path invariants |
