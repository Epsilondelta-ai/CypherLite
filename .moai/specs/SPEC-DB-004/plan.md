---
id: SPEC-DB-004
type: plan
version: "0.4.0"
status: draft
created: "2026-03-10"
updated: "2026-03-10"
author: epsilondelta
priority: P0
tags: [temporal, datetime, versioning, at-time, version-store, temporal-index]
---

# SPEC-DB-004 Implementation Plan: CypherLite Phase 4 - Temporal Dimension

---

## HISTORY

| Version | Date | Changes |
|---------|------|---------|
| 0.4.0 | 2026-03-10 | Initial implementation plan based on research analysis |

---

## 1. Technology Stack

| Technology | Version | Purpose | Rationale |
|------------|---------|---------|-----------|
| Existing BTree (cypherlite-storage) | N/A | VersionStore backing structure | Reuse proven Phase 1 B-tree; stores version records keyed by (entity_id, seq) |
| Existing PropertyIndex (cypherlite-storage) | N/A | Temporal index on _created_at | Reuse Phase 3 PropertyIndex with BTreeMap; no new index infrastructure needed |
| Existing Logos lexer (cypherlite-query) | 0.14 | AT/TIME/BETWEEN token recognition | Extend existing token enum; zero additional dependencies |
| Manual ISO 8601 parser | N/A | datetime() string parsing | Avoid chrono dependency; keep binary small; parse subset of ISO 8601 |
| bincode (existing) | N/A | DateTime serialization | PropertyValue already uses bincode; DateTime(i64) is trivially serializable |

### Rejected Alternatives

| Alternative | Rejection Rationale |
|-------------|---------------------|
| `chrono` crate for DateTime | Adds ~50KB to binary; only need subset of ISO 8601 parsing; manual parsing is simpler for our format subset |
| `time` crate for DateTime | Similar to chrono; overkill for millis-since-epoch representation |
| Delta-based version compression | Complexity disproportionate to v0.4 scope; full-copy simpler to implement and debug |
| Anchor+Delta versioning (design doc) | Deferred to v0.5+; full-copy provides correct semantics first, optimize later |
| Interval tree for temporal index | BTreeMap on _created_at sufficient for v0.4; interval tree deferred to v0.5+ |
| Separate temporal B-tree structure | PropertyIndex reuse avoids new data structure; _created_at is just a property |

---

## 2. Crate Structure (Phase 4 Additions)

```
crates/
  cypherlite-core/
    src/
      types.rs                  + PropertyValue::DateTime(i64) variant
                                + DateTime parsing/formatting utility functions
                                + PropertyValueKey::DateTime ordering

  cypherlite-storage/
    src/
      version/                  [NEW] Version storage module
        mod.rs                  VersionStore struct
                                - new(page_manager) -> VersionStore
                                - snapshot_node(node_id, record) -> Result<u64>
                                - snapshot_edge(edge_id, record) -> Result<u64>
                                - get_node_version_at(node_id, timestamp) -> Option<NodeRecord>
                                - get_node_versions_between(node_id, start, end) -> Vec<NodeRecord>
                                - get_version_count(entity_id) -> u64
      btree/
        node_store.rs           + pre_update_hook integration point
        edge_store.rs           + pre_update_hook integration point
      page/
        mod.rs                  + DatabaseHeader v2 fields
                                + format_version migration logic
      index/
        mod.rs                  + auto-create temporal index on _created_at
      lib.rs                    + StorageEngine temporal API methods
                                + automatic _created_at/_updated_at injection
                                + DatabaseConfig temporal fields

  cypherlite-query/
    src/
      lexer/
        mod.rs                  + Token::At, Token::Time, Token::Between, Token::History
      parser/
        ast.rs                  + TemporalPredicate enum
                                + MatchClause.temporal_predicate field
        mod.rs                  + parse_temporal_predicate()
      semantic/
        mod.rs                  + validate_temporal_predicate()
      planner/
        mod.rs                  + LogicalPlan::AsOfScan
                                + LogicalPlan::TemporalRangeScan
      executor/
        mod.rs                  + execute_as_of_scan()
                                + execute_temporal_range_scan()
        value.rs                + Value::DateTime(i64) variant
      functions.rs              + datetime() built-in function
                                + now() built-in function
```

---

## 3. Task Decomposition

### Group U: DateTime Foundation

**Priority**: Primary Goal (all other groups depend on this)

| Task | Files Modified | Dependencies | Complexity |
|------|---------------|--------------|------------|
| U-T1: Add PropertyValue::DateTime(i64) variant | `cypherlite-core/src/types.rs` | None | Low |
| U-T2: Update PropertyValueKey ordering for DateTime | `cypherlite-core/src/types.rs` | U-T1 | Low |
| U-T3: Update bincode serialization (discriminant 7) | `cypherlite-core/src/types.rs` | U-T1 | Low |
| U-T4: Add Display/Debug formatting for DateTime | `cypherlite-core/src/types.rs` | U-T1 | Low |
| U-T5: Add ISO 8601 parser (manual, no deps) | `cypherlite-core/src/types.rs` | U-T1 | Medium |
| U-T6: Add Value::DateTime variant to query executor | `cypherlite-query/src/executor/value.rs` | U-T1 | Low |
| U-T7: Register datetime()/now() built-in functions | `cypherlite-query/src/executor/mod.rs` or `functions.rs` | U-T5, U-T6 | Medium |
| U-T8: DateTime comparison in expression evaluator | `cypherlite-query/src/executor/mod.rs` | U-T6 | Low |

### Group V: Timestamp Tracking

**Priority**: Secondary Goal (depends on Group U)

| Task | Files Modified | Dependencies | Complexity |
|------|---------------|--------------|------------|
| V-T1: Add DatabaseConfig temporal fields | `cypherlite-storage/src/lib.rs` | None | Low |
| V-T2: Inject _created_at on CREATE path | `cypherlite-storage/src/lib.rs`, `btree/node_store.rs` | U-T1, V-T1 | Medium |
| V-T3: Inject _updated_at on SET path | `cypherlite-storage/src/lib.rs` | U-T1, V-T1 | Medium |
| V-T4: System property write protection | `cypherlite-query/src/executor/mod.rs` | V-T2 | Low |
| V-T5: Timestamp opt-out via config | `cypherlite-storage/src/lib.rs` | V-T1, V-T2, V-T3 | Low |

### Group W: Version Storage

**Priority**: Secondary Goal (depends on Group V)

| Task | Files Modified | Dependencies | Complexity |
|------|---------------|--------------|------------|
| W-T1: Create version/ module with VersionStore | `cypherlite-storage/src/version/mod.rs` [NEW] | None | High |
| W-T2: Extend DatabaseHeader with version_store_root_page | `cypherlite-storage/src/page/mod.rs` | None | Medium |
| W-T3: Format version migration (v1 -> v2) | `cypherlite-storage/src/page/mod.rs` | W-T2 | Medium |
| W-T4: Pre-update snapshot hook in node_store | `cypherlite-storage/src/btree/node_store.rs` | W-T1 | Medium |
| W-T5: Pre-update snapshot hook in edge_store | `cypherlite-storage/src/btree/edge_store.rs` | W-T1 | Medium |
| W-T6: Version chain traversal API | `cypherlite-storage/src/version/mod.rs` | W-T1 | Medium |
| W-T7: Version storage opt-out via config | `cypherlite-storage/src/lib.rs` | W-T1, V-T1 | Low |

### Group X: AT TIME Query Syntax

**Priority**: Tertiary Goal (depends on Groups U, V, W)

| Task | Files Modified | Dependencies | Complexity |
|------|---------------|--------------|------------|
| X-T1: Add AT/TIME/BETWEEN/HISTORY tokens | `cypherlite-query/src/lexer/mod.rs` | None | Low |
| X-T2: Define TemporalPredicate AST enum | `cypherlite-query/src/parser/ast.rs` | None | Low |
| X-T3: Parse AT TIME clause in MATCH | `cypherlite-query/src/parser/mod.rs` | X-T1, X-T2 | Medium |
| X-T4: Semantic validation of temporal predicates | `cypherlite-query/src/semantic/mod.rs` | X-T2, U-T7 | Medium |
| X-T5: AsOfScan logical plan operator | `cypherlite-query/src/planner/mod.rs` | X-T2 | Medium |
| X-T6: AsOfScan executor implementation | `cypherlite-query/src/executor/mod.rs` | X-T5, W-T6 | High |

### Group Y: Temporal Range Queries

**Priority**: Tertiary Goal (depends on Group X)

| Task | Files Modified | Dependencies | Complexity |
|------|---------------|--------------|------------|
| Y-T1: Parse BETWEEN TIME ... AND ... clause | `cypherlite-query/src/parser/mod.rs` | X-T1, X-T2 | Medium |
| Y-T2: TemporalRangeScan logical plan operator | `cypherlite-query/src/planner/mod.rs` | X-T2, Y-T1 | Medium |
| Y-T3: TemporalRangeScan executor | `cypherlite-query/src/executor/mod.rs` | Y-T2, W-T6 | High |
| Y-T4: Temporal index on _created_at | `cypherlite-storage/src/index/mod.rs` | V-T2 | Medium |
| Y-T5: Planner integration with temporal index | `cypherlite-query/src/planner/mod.rs` | Y-T4 | Medium |

### Group Z: Quality Finalization

**Priority**: Final Goal (depends on all above)

| Task | Files Modified | Dependencies | Complexity |
|------|---------------|--------------|------------|
| Z-T1: Clippy clean pass | Workspace-wide | All groups | Low |
| Z-T2: Proptest temporal invariants | Test files | All groups | Medium |
| Z-T3: Criterion benchmarks | Bench files | All groups | Medium |
| Z-T4: Version bump to 0.4.0 | `Cargo.toml` (workspace) | All groups | Low |
| Z-T5: Integration test suite | Test files | All groups | Medium |

---

## 4. Implementation Order

```
Phase 4a: Foundation (Group U)
  U-T1 -> U-T2 -> U-T3 -> U-T4 -> U-T5 -> U-T6 -> U-T7 -> U-T8
  (all sequential; each builds on DateTime variant)

Phase 4b: Timestamp + Version (Groups V, W -- partial parallel)
  V-T1 (config) ─────────────────────> V-T2 (create) -> V-T3 (set) -> V-T4 (protection) -> V-T5 (opt-out)
  W-T2 (header) -> W-T3 (migration) ─┐
  W-T1 (store) ──────────────────────> W-T4 (node hook) -> W-T5 (edge hook) -> W-T6 (traversal) -> W-T7 (opt-out)

Phase 4c: Temporal Queries (Groups X, Y -- sequential)
  X-T1 -> X-T2 -> X-T3 -> X-T4 -> X-T5 -> X-T6
                                           |
  Y-T1 -> Y-T2 -> Y-T3                    |
  Y-T4 (temporal index, parallel with X) ---> Y-T5 (planner integration)

Phase 4d: Quality (Group Z)
  Z-T1, Z-T2, Z-T3, Z-T5 (parallel) -> Z-T4 (version bump, last)
```

---

## 5. File Impact Analysis

### cypherlite-core (3 files)

| File | Change Type | Impact |
|------|------------|--------|
| `src/types.rs` | Major extension | +DateTime variant, +parsing, +formatting, +ordering |
| `src/lib.rs` | Minor | Re-export DateTime utilities |
| `src/error.rs` | Minor | +InvalidDateTimeFormat error variant |

### cypherlite-storage (8 files)

| File | Change Type | Impact |
|------|------------|--------|
| `src/version/mod.rs` | **New file** | VersionStore struct, full implementation |
| `src/page/mod.rs` | Medium | DatabaseHeader v2, format migration |
| `src/btree/node_store.rs` | Medium | Pre-update snapshot hook |
| `src/btree/edge_store.rs` | Medium | Pre-update snapshot hook |
| `src/index/mod.rs` | Medium | Auto-create temporal index |
| `src/lib.rs` | Major | DatabaseConfig, temporal APIs, timestamp injection |
| `src/catalog/mod.rs` | Minor | Register _created_at, _updated_at property names |
| `src/mod.rs` or `src/lib.rs` | Minor | Module declaration for version/ |

### cypherlite-query (8 files)

| File | Change Type | Impact |
|------|------------|--------|
| `src/lexer/mod.rs` | Minor | +4 tokens (AT, TIME, BETWEEN, HISTORY) |
| `src/parser/ast.rs` | Medium | +TemporalPredicate enum, +MatchClause field |
| `src/parser/mod.rs` | Medium | +parse_temporal_predicate() |
| `src/semantic/mod.rs` | Medium | +temporal validation |
| `src/planner/mod.rs` | Medium | +AsOfScan, +TemporalRangeScan operators |
| `src/executor/mod.rs` | Major | +temporal scan execution, +datetime/now functions |
| `src/executor/value.rs` | Minor | +Value::DateTime variant |
| `src/functions.rs` (if exists) | Medium | +datetime(), +now() registration |

### Workspace root (1 file)

| File | Change Type | Impact |
|------|------------|--------|
| `Cargo.toml` | Minor | Version bump to 0.4.0 |

**Total files modified**: ~20
**New files**: 1 (`version/mod.rs`)

---

## 6. Risk Assessment

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| Bincode serialization breaking change from new variant | Medium | High | Use tagged enum with explicit discriminants; add migration test from v0.3 data |
| Full-copy versioning causes excessive storage growth | Medium | Medium | Document storage overhead in benchmarks; defer delta compression to v0.5 |
| ISO 8601 parser edge cases (timezones, leap seconds) | Medium | Low | Support only common subset (YYYY-MM-DD, T, Z, +HH:MM); reject ambiguous formats |
| DatabaseHeader v1->v2 migration corrupts existing data | Low | Critical | Write comprehensive round-trip tests; version_store_root_page=0 means "not allocated" |
| AT TIME query performance with long version chains | Medium | Medium | Temporal index on _created_at provides O(log n) filtering; benchmark with 1000 versions |
| WASM compatibility of system clock for now() | Low | Low | Allow now() to return 0 in WASM builds; document limitation |
| Backward compatibility: v0.3 databases opening in v0.4 | Medium | High | Auto-migrate header on open; preserve all existing data; add format_version check |

---

## 7. Architecture Design Direction

### 7.1 Layered Temporal Architecture

```
Layer 4: Query Layer (AT TIME / BETWEEN TIME)
    |
Layer 3: Logical Plan (AsOfScan / TemporalRangeScan)
    |
Layer 2: Storage API (get_node_at_time / get_versions_between)
    |
Layer 1: VersionStore (B-tree keyed by (entity_id, seq))
    |
Layer 0: Page Manager (existing)
```

### 7.2 Design Principles

1. **Current-state first**: No performance regression for non-temporal queries. Current state stays in primary NodeStore/EdgeStore.
2. **Lazy allocation**: VersionStore B-tree root page allocated only on first version snapshot, not on database creation.
3. **Minimal surface area**: Temporal features are additive. Removing `AT TIME` from a query returns current state.
4. **Property-based timestamps**: `_created_at`/`_updated_at` are regular properties, not structural fields. This avoids NodeRecord schema changes.
5. **Full-copy simplicity**: v0.4 prioritizes correctness over storage efficiency. Each version is a complete snapshot.

### 7.3 Non-Goals for v0.4

- Bitemporal queries (valid time + transaction time)
- Temporal relationship validity periods
- HISTORY() aggregate function
- Delta compression for version chains
- Interval tree index
- Temporal path traversal (traverse graph at a point in time following relationships)
