---
id: SPEC-DB-005
type: plan
version: "0.5.0"
status: draft
created: "2026-03-11"
updated: "2026-03-11"
author: epsilondelta
priority: P0
tags: [temporal-edge, feature-flags, edge-validity, edge-index]
---

# SPEC-DB-005 Implementation Plan: Temporal Edge Validity & Feature Flags (v0.5)

---

## HISTORY

| Version | Date | Changes |
|---------|------|---------|
| 0.5.0 | 2026-03-11 | Initial implementation plan |

---

## 1. Technology Stack

| Technology | Version | Purpose | Rationale |
|------------|---------|---------|-----------|
| Cargo feature flags | N/A | Modular temporal compilation | SQLite model; zero runtime overhead |
| Existing BTree | N/A | Edge property index storage | Reuse proven Phase 1 B-tree |
| Existing PropertyIndex | N/A | Edge property index structure | Same pattern as node indexes |
| Existing Expand operator | N/A | Temporal filter injection point | Minimal change to existing traversal |

---

## 2. Task Decomposition

### Group AA: Feature Flags Foundation (4 tasks)

| Task | Files Modified | Dependencies | Complexity |
|------|---------------|--------------|------------|
| AA-T1: Add feature flags to all Cargo.toml files | Workspace + 3 crate Cargo.toml | None | Low |
| AA-T2: Add feature_flags field to DatabaseHeader | `cypherlite-storage/src/page/mod.rs` | None | Medium |
| AA-T3: Implement v2->v3 header migration | `cypherlite-storage/src/page/page_manager.rs` | AA-T2 | Medium |
| AA-T4: Add feature compatibility check on open | `cypherlite-storage/src/lib.rs` | AA-T2, AA-T3 | Medium |

### Group BB: Edge Temporal Properties (5 tasks)

| Task | Files Modified | Dependencies | Complexity |
|------|---------------|--------------|------------|
| BB-T1: Add _valid_from/_valid_to system property registration | `cypherlite-storage/src/catalog/` | AA-T1 | Low |
| BB-T2: Auto-inject _created_at/_updated_at on edge CREATE | `cypherlite-query/src/executor/operators/create.rs` | BB-T1 | Medium |
| BB-T3: Auto-inject _valid_from on edge CREATE | `cypherlite-query/src/executor/operators/create.rs` | BB-T1 | Low |
| BB-T4: Allow user SET of _valid_from/_valid_to | `cypherlite-query/src/executor/operators/set_props.rs` | BB-T1 | Low |
| BB-T5: Update _updated_at on edge SET | `cypherlite-query/src/executor/operators/set_props.rs` | BB-T2 | Low |

### Group CC: Edge Property Indexes (5 tasks)

| Task | Files Modified | Dependencies | Complexity |
|------|---------------|--------------|------------|
| CC-T1: Create EdgeIndexManager struct | `cypherlite-storage/src/index/edge_index.rs` [NEW] | AA-T1 | Medium |
| CC-T2: Integrate EdgeIndexManager into StorageEngine | `cypherlite-storage/src/lib.rs` | CC-T1 | Medium |
| CC-T3: Parse CREATE/DROP INDEX for relationship types | `cypherlite-query/src/parser/` | CC-T1 | Medium |
| CC-T4: Execute CREATE/DROP INDEX for edges | `cypherlite-query/src/executor/` | CC-T2, CC-T3 | Medium |
| CC-T5: Auto-update edge indexes on edge CREATE/SET/DELETE | `cypherlite-storage/src/lib.rs` | CC-T2 | Medium |

### Group DD: AT TIME / BETWEEN TIME for Edges (6 tasks)

| Task | Files Modified | Dependencies | Complexity |
|------|---------------|--------------|------------|
| DD-T1: Add TemporalFilter struct | `cypherlite-query/src/executor/operators/` | None | Low |
| DD-T2: Add temporal filter to Expand operator | `cypherlite-query/src/executor/operators/expand.rs` | DD-T1 | Medium |
| DD-T3: Add temporal filter to VarLengthExpand | `cypherlite-query/src/executor/operators/var_length_expand.rs` | DD-T1 | High |
| DD-T4: Planner: pass temporal predicate to Expand | `cypherlite-query/src/planner/mod.rs` | DD-T1 | Medium |
| DD-T5: AT TIME filters nodes AND edges | `cypherlite-query/src/executor/operators/temporal_scan.rs` | DD-T2 | Medium |
| DD-T6: BETWEEN TIME filters nodes AND edges | `cypherlite-query/src/executor/operators/temporal_scan.rs` | DD-T2 | Medium |

### Group EE: Temporal Path Queries (3 tasks)

| Task | Files Modified | Dependencies | Complexity |
|------|---------------|--------------|------------|
| EE-T1: VarLengthExpand AT TIME temporal continuity | `cypherlite-query/src/executor/operators/var_length_expand.rs` | DD-T3 | High |
| EE-T2: VarLengthExpand BETWEEN TIME overlap check | `cypherlite-query/src/executor/operators/var_length_expand.rs` | DD-T3 | High |
| EE-T3: Integration tests for temporal paths | Tests | EE-T1, EE-T2 | Medium |

### Group FF: Quality & Migration (5 tasks)

| Task | Files Modified | Dependencies | Complexity |
|------|---------------|--------------|------------|
| FF-T1: Clippy clean pass | Workspace-wide | All groups | Low |
| FF-T2: Proptest temporal edge invariants | Test files | All groups | Medium |
| FF-T3: Criterion benchmarks temporal edge filtering | Bench files | All groups | Medium |
| FF-T4: v2->v3 migration tests | Test files | AA-T3 | Medium |
| FF-T5: Version bump to 0.5.0 | Cargo.toml files | All groups | Low |

---

## 3. Implementation Order

```
Phase 5a: Foundation (Groups AA)
  AA-T1 -> AA-T2 -> AA-T3 -> AA-T4

Phase 5b: Edge Temporal + Indexes (Groups BB, CC -- parallel)
  BB-T1 -> BB-T2 -> BB-T3 -> BB-T4 -> BB-T5
  CC-T1 -> CC-T2 -> CC-T3 -> CC-T4 -> CC-T5

Phase 5c: Temporal Query Extension (Groups DD, EE -- sequential)
  DD-T1 -> DD-T2 -> DD-T3 -> DD-T4 -> DD-T5 -> DD-T6
  EE-T1 -> EE-T2 -> EE-T3

Phase 5d: Quality (Group FF)
  FF-T1, FF-T2, FF-T3, FF-T4 (parallel) -> FF-T5 (last)
```

---

## 4. File Impact Analysis

### cypherlite-core (2 files)
| File | Change Type | Impact |
|------|------------|--------|
| `Cargo.toml` | Minor | Feature flag definitions |
| `src/types.rs` | Minor | cfg attributes on temporal types |

### cypherlite-storage (7 files)
| File | Change Type | Impact |
|------|------------|--------|
| `Cargo.toml` | Minor | Feature flag definitions |
| `src/page/mod.rs` | Medium | DatabaseHeader v3, feature_flags field |
| `src/page/page_manager.rs` | Medium | v2->v3 migration |
| `src/index/edge_index.rs` | **New file** | EdgeIndexManager |
| `src/index/mod.rs` | Medium | Export edge index |
| `src/lib.rs` | Major | EdgeIndexManager integration, feature check |
| `src/catalog/mod.rs` | Minor | Register _valid_from, _valid_to |

### cypherlite-query (8 files)
| File | Change Type | Impact |
|------|------------|--------|
| `Cargo.toml` | Minor | Feature flag definitions |
| `src/executor/operators/expand.rs` | Major | Temporal filter in edge traversal |
| `src/executor/operators/var_length_expand.rs` | Major | Temporal continuity logic |
| `src/executor/operators/create.rs` | Medium | Edge timestamp injection |
| `src/executor/operators/set_props.rs` | Medium | Edge _updated_at, _valid_from/_valid_to |
| `src/executor/operators/temporal_scan.rs` | Medium | Node+edge combined temporal scanning |
| `src/planner/mod.rs` | Medium | Pass temporal predicate to Expand |
| `src/parser/clause.rs` | Minor | Edge index syntax |

**Total files modified**: ~17
**New files**: 1

---

## 5. Risk Assessment

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| Feature flag cfg complexity grows | Medium | Medium | Document all cfg boundaries; CI tests all combinations |
| Expand performance regression from temporal check | Low | High | O(1) property lookup; benchmark before/after |
| v2->v3 migration data loss | Low | Critical | Comprehensive round-trip tests |
| Edge index storage overhead | Medium | Low | Indexes are opt-in; not auto-created |
