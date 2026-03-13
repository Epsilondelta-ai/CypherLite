---
id: SPEC-DB-007
version: "0.7.0"
status: completed
created: "2026-03-11"
updated: "2026-03-12"
completed: "2026-03-12"
author: epsilondelta
priority: P2
tags: [hypergraph, hyperedge, temporal-reference, multi-node-edge, graph-entity]
lifecycle: spec-anchored
depends_on: [SPEC-DB-006]
---

# SPEC-DB-007: CypherLite Phase 7 - Native Hyperedges (v0.7)

> Introduce native hyperedge support where a single edge connects arbitrary sets of nodes, subgraphs, or temporal node references. Add HyperEdgeStore, HYPEREDGE syntax, and temporal reference semantics (Node AT TIME) for connecting entities across time dimensions.

---

## HISTORY

| Version | Date | Changes |
|---------|------|---------|
| 0.7.0 | 2026-03-11 | Initial SPEC based on temporal hypergraph analysis |
| 0.7.0-r1 | 2026-03-12 | Research-backed revision: corrected header v5 layout, generalized reverse index, added HyperEdge GraphEntity variant, INVOLVES pattern reference, 3-phase implementation plan |
| 0.7.0-r2 | 2026-03-12 | Added Section 9: Impact analysis on existing SPECs (DB-001~006), exhaustive match audit (6 locations), wildcard arm review (24 locations), backward compatibility guarantee |

---

## 1. Environment

### 1.1 Feature Flag

```toml
hypergraph = ["subgraph"]  # Requires v0.6 subgraph support
```

### 1.2 Crate Structure

- **Extended crate**: `cypherlite-core` -- HyperEdgeId, HyperEdgeRecord, TemporalRef types
- **New module**: `cypherlite-storage/src/hyperedge/` -- HyperEdgeStore, reverse index
- **Extended crate**: `cypherlite-query` -- HYPEREDGE syntax, HyperEdgeScan operators

---

## 2. Requirements (EARS Format)

### Group LL: HyperEdge Storage

**LL-001**: When the `hypergraph` feature is enabled, the system SHALL provide a HyperEdgeStore backed by BTree<u64, HyperEdgeRecord>.

**LL-002**: The HyperEdgeRecord SHALL contain:
- `id: HyperEdgeId`
- `rel_type_id: u32`
- `sources: Vec<GraphEntity>` (source participants)
- `targets: Vec<GraphEntity>` (target participants)
- `properties: Vec<(u32, PropertyValue)>`

**LL-003**: GraphEntity SHALL be extended to include hyperedge and temporal reference variants:
```rust
pub enum GraphEntity {
    Node(NodeId),
    Subgraph(SubgraphId),
    #[cfg(feature = "hypergraph")]
    HyperEdge(HyperEdgeId),       // hyperedge as participant (tag byte 2)
    TemporalRef(NodeId, i64),     // node at specific timestamp (tag byte 3)
}
```
> Reference: Current GraphEntity at `cypherlite-core/src/types.rs` lines 55-63 uses binary tag byte serialization (0=Node, 1=Subgraph). HyperEdge adds tag byte 2, TemporalRef adds tag byte 3.

**LL-004**: The system SHALL maintain a reverse index: `BTreeMap<u64, Vec<u64>>` (participant_raw_id -> hyperedge_ids) for efficient lookup of all hyperedges containing a given entity. The index uses raw u64 keys because participants can be Nodes, Subgraphs, or TemporalRefs.
> Reference: Follows the `MembershipIndex` dual-BTreeMap pattern at `cypherlite-storage/src/subgraph/membership.rs` (224 lines) which uses `BTreeMap<u64, Vec<u64>>` for both forward and reverse lookups with idempotent add and automatic cleanup of empty lists.

**LL-005**: The DatabaseHeader v5 SHALL include `hyperedge_root_page: u64` (bytes 64-71) and `next_hyperedge_id: u64` (bytes 72-79), both using u64 auto-increment pattern consistent with SubgraphStore.
> Reference: Current DatabaseHeader at `cypherlite-storage/src/page/mod.rs` lines 115-146. Subgraph fields occupy bytes 48-55 (`subgraph_root_page`) and 56-63 (`next_subgraph_id`). Bytes 64-79 are available for v5 extension. Feature flag `FLAG_HYPERGRAPH = 1 << 3` (Bit 3) extends existing flag constants at lines 149-155.

### Group MM: HYPEREDGE Syntax

**MM-001**: The system SHALL support creating hyperedges:
```cypher
CREATE HYPEREDGE h:GroupMigration
  FROM (node1, node2, node3)
  TO (node4)
  SET h.date = datetime('2026-03-10')
```

**MM-002**: The system SHALL support temporal references in hyperedge creation:
```cypher
CREATE HYPEREDGE h:TemporalShift
  FROM (person AT TIME datetime('2026-01'), city1)
  TO (person AT TIME datetime('2026-02'), city2)
  SET h.event = 'relocation'
```

**MM-003**: The system SHALL support querying hyperedges:
```cypher
MATCH HYPEREDGE (h:GroupMigration)
WHERE h.date > datetime('2026-01-01')
RETURN h
```

**MM-004**: The system SHALL support finding hyperedges containing a specific node via the `:INVOLVES` virtual relationship:
```cypher
MATCH HYPEREDGE (h)-[:INVOLVES]->(person:Person {name: 'Alice'})
RETURN h
```
> The `:INVOLVES` virtual relationship follows the same pattern as `:CONTAINS` in SPEC-DB-006 (SubgraphStore). It is derived from the reverse index at query time, not stored as a physical edge. Reference: `:CONTAINS` implementation at SPEC-DB-006 HH-004/HH-005, using `MembershipIndex.memberships()` for reverse lookup.

### Group NN: Temporal References

**NN-001**: A TemporalRef(NodeId, timestamp) SHALL resolve to the version of that node at the specified timestamp using VersionStore.
> Reference: VersionStore at `cypherlite-storage/src/version/mod.rs` (260 lines) provides `get_version_chain(entity_id)` and snapshot per `(entity_id, version_seq)`. Resolution walks the version chain to find the version created at or before the target timestamp.

**NN-002**: When a hyperedge contains a TemporalRef, accessing properties of that reference SHALL return the node's properties at the referenced time.

**NN-003**: TemporalRef resolution SHALL be lazy: only resolved when properties are accessed, not at hyperedge creation time.

### Group OO: HyperEdge Traversal

**OO-001**: The system SHALL support traversing from a node to its connected hyperedges via the reverse index.

**OO-002**: The system SHALL support traversing from a hyperedge to its member nodes/subgraphs/temporal refs.

**OO-003**: The system SHALL support filtering hyperedges by type and properties during traversal.

### Group PP: Quality

**PP-001**: All SPEC-DB-005 and SPEC-DB-006 tests SHALL continue to pass.
**PP-002**: Proptest invariants for hyperedge membership and temporal reference resolution.
**PP-003**: Integration tests for HYPEREDGE creation, querying, and traversal.
**PP-004**: Criterion benchmarks for hyperedge operations.
**PP-005**: Version bump to 0.7.0.

---

## 3. Non-Goals for v0.7

- Hyperedge-to-hyperedge relationships (meta-hypergraph)
- Hyperedge pattern matching in variable-length paths
- Hyperedge property indexes
- Hyperedge participation in MERGE operations
- Weighted hyperedges for graph algorithms

---

## 4. Architecture Design

### 4.1 HyperEdgeRecord & GraphEntity Extension

```rust
#[cfg(feature = "hypergraph")]
pub struct HyperEdgeId(pub u64);

#[cfg(feature = "hypergraph")]
pub struct HyperEdgeRecord {
    pub id: HyperEdgeId,
    pub rel_type_id: u32,
    pub sources: Vec<GraphEntity>,
    pub targets: Vec<GraphEntity>,
    pub properties: Vec<(u32, PropertyValue)>,
}

// Extended GraphEntity enum (cypherlite-core/src/types.rs)
// Current: Node(NodeId), Subgraph(SubgraphId) -- cfg(feature = "subgraph")
// v0.7 adds HyperEdge + TemporalRef variants
#[cfg(feature = "subgraph")]
#[derive(Debug, Clone, PartialEq)]
pub enum GraphEntity {
    Node(NodeId),                       // tag byte 0
    Subgraph(SubgraphId),              // tag byte 1
    #[cfg(feature = "hypergraph")]
    HyperEdge(HyperEdgeId),           // tag byte 2
    TemporalRef(NodeId, i64),          // tag byte 3 -- node at timestamp
}
```
> Reference: Existing GraphEntity at `cypherlite-core/src/types.rs` lines 55-63. Serialization uses binary tag byte approach (0=Node, 1=Subgraph) with `#[serde(default)]` for backward compatibility. Vec<GraphEntity> serialization follows the same pattern as Vec<(u32, PropertyValue)> using bincode (see types.rs lines 438-458).

### 4.2 HyperEdgeStore

```rust
#[cfg(feature = "hypergraph")]
pub struct HyperEdgeStore {
    /// Storage: hyperedge_id -> HyperEdgeRecord (in-memory, like SubgraphStore)
    records: BTreeMap<u64, HyperEdgeRecord>,
    /// Reverse index: participant_raw_id -> [hyperedge_ids]
    /// Uses u64 keys because participants can be Node, Subgraph, or TemporalRef
    reverse_index: BTreeMap<u64, Vec<u64>>,
    /// Next available hyperedge ID (u64 auto-increment)
    next_id: u64,
}
```

> **Reference Implementation**: Follows the `SubgraphStore` pattern at `cypherlite-storage/src/subgraph/mod.rs` (87 lines):
> - `BTreeMap<u64, Record>` with auto-increment ID allocation via `next_id: u64`
> - CRUD API: `new(start_id)`, `create()`, `get()`, `delete()`, `all()`
> - StorageEngine integration: `#[cfg(feature = "hypergraph")] hyperedge_store: HyperEdgeStore`
> - Header-synced: `next_hyperedge_id` persisted to DatabaseHeader on create
>
> Reverse index follows `MembershipIndex` dual-BTreeMap pattern at `cypherlite-storage/src/subgraph/membership.rs`:
> - Idempotent add (duplicate is no-op)
> - Automatic cleanup of empty Vec entries on remove
> - Bidirectional: forward (hyperedge -> participants) and reverse (participant -> hyperedges)

### 4.3 DatabaseHeader v5

```
// Existing v4 layout (for context):
// Bytes 0-3:   magic (u32)
// Bytes 4-7:   version (u32) -- v5 when hypergraph enabled
// Bytes 8-11:  page_count (u32)
// Bytes 12-15: root_node_page (u32)
// Bytes 16-19: root_edge_page (u32)
// Bytes 20-27: next_node_id (u64)
// Bytes 28-35: next_edge_id (u64)
// Bytes 36-43: version_store_root_page (u64, v2+)
// Bytes 44-47: feature_flags (u32, v3+)
// Bytes 48-55: subgraph_root_page (u64, v4+)
// Bytes 56-63: next_subgraph_id (u64, v4+)

// New v5 fields:
Bytes 64-71: hyperedge_root_page (u64, v5)  // root page for HyperEdgeStore B-tree
Bytes 72-79: next_hyperedge_id (u64, v5)    // auto-increment counter (u64, NOT u32)

// Feature flag addition:
FLAG_HYPERGRAPH = 1 << 3  // Bit 3 (extends FLAG_SUBGRAPH at Bit 2)

// Format version:
#[cfg(feature = "hypergraph")]
pub const FORMAT_VERSION: u32 = 5;
```
> Reference: Current header serialization at `cypherlite-storage/src/page/mod.rs` lines 194-214 (`to_page()`) and 218-276 (`from_page()`). Auto-migration pattern: `if version >= 5 { read fields } else { 0 }` consistent with v3->v4 migration for subgraph fields.

### 4.4 Query Pipeline

```
Lexer:    HYPEREDGE keyword (+ reuse FROM/TO/AT/TIME from P4/P6)
Parser:   CreateHyperedgeClause { variable, labels, properties, sources, targets }
          MatchHyperedgeClause { variable, labels, filter }
Planner:  CreateHyperedgeOp { variable, properties, sources, targets }
          HyperEdgeScan { variable, filter }
Executor: execute_create_hyperedge_op() -- create record + update reverse index
          execute_hyperedge_scan() -- iterate HyperEdgeStore with filter
```

> **Reference Pipeline**: CREATE SNAPSHOT at `cypherlite-query/src/`:
> - Lexer: `lexer/mod.rs` lines 128-142 -- keyword pattern `#[regex("(?i)hyperedge", priority = 10)]`
> - Parser: `parser/mod.rs` line 68 -- `Clause::CreateSnapshot(parser.parse_create_snapshot_clause()?)` pattern
> - AST: `parser/ast.rs` lines 256-274 -- `CreateSnapshotClause` struct as template
> - Planner: `planner/mod.rs` lines 8-175 -- `LogicalPlan::CreateSnapshotOp` as template
> - Executor: `executor/mod.rs` lines 96-150 -- dispatch via `match plan { ... }`, `Value::Subgraph` pattern for `Value::Hyperedge`

---

## 5. Example Scenario: Group Migration with Temporal References

```cypher
-- Multiple people moved from Busan to Seoul
CREATE (철수:Person {name: '철수'})
CREATE (민수:Person {name: '민수'})
CREATE (영희:Person {name: '영희'})
CREATE (부산:City {name: '부산'}), (서울:City {name: '서울'})

-- Create a hyperedge representing group migration event
CREATE HYPEREDGE h:GroupMigration
  FROM (철수 AT TIME datetime('2026-03-04'), 민수 AT TIME datetime('2026-03-04'), 부산)
  TO (철수 AT TIME datetime('2026-03-10'), 민수 AT TIME datetime('2026-03-10'), 서울)
  SET h.date = datetime('2026-03-10'),
      h.reason = 'Office relocation',
      h.participants = 2

-- Query: Find all migration events involving 철수
MATCH HYPEREDGE (h:GroupMigration)-[:INVOLVES]->(p:Person {name: '철수'})
RETURN h.date, h.reason

-- Query: Get the FROM state of a migration
MATCH HYPEREDGE (h:GroupMigration {date: datetime('2026-03-10')})
RETURN h.sources, h.targets
```

---

## 6. Competitive Positioning

| System | Regular Edges | Temporal Edges | Subgraphs | Hyperedges | Temporal Refs |
|--------|:---:|:---:|:---:|:---:|:---:|
| Neo4j | Y | N | N | N | N |
| Amazon Neptune | Y | N | Named Graphs (RDF) | N | N |
| TigerGraph | Y | N | N | N | N |
| AeonG (academic) | Y | Y | N | N | N |
| **CypherLite v0.7** | **Y** | **Y** | **Y** | **Y** | **Y** |

CypherLite v0.7 would be the first graph database (embedded or otherwise) to natively support temporal hyperedges with temporal node references.

---

## 7. Risk Assessment

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| Variable-size HyperEdgeRecord serialization | Medium | Medium | Reuse Vec serialization pattern from properties (bincode, types.rs lines 438-458) |
| Reverse index memory overhead | Medium | Medium | Lazy loading; only build when queried. Follow MembershipIndex cleanup pattern |
| HYPEREDGE syntax Cypher deviation | High | Low | Document as CypherLite extension; prefix with keyword |
| TemporalRef resolution performance | Medium | Medium | Cache resolved versions; lazy resolution via VersionStore chain walk |
| GraphEntity enum expansion (4 variants) | Low | Medium | Use extensible tagged serialization (tag bytes 0-3). Room for future variants |
| In-memory store data loss on restart | High | Medium | Consistent with SubgraphStore limitation. Document as known limitation; plan persistence in SPEC-DB-008 |

---

## 8. Implementation Phases

### Phase 7a+7b: Core Storage (Priority: Primary Goal)

**Scope**: Types in cypherlite-core, HyperEdgeStore + reverse index + header v5 in cypherlite-storage, serialization.

**Requirements covered**: LL-001, LL-002, LL-003, LL-004, LL-005

**File impact**:

| File | Change Type | Description |
|------|------------|-------------|
| `cypherlite-core/src/types.rs` | Extend | Add `HyperEdgeId(pub u64)`, `HyperEdgeRecord` struct, extend `GraphEntity` with `HyperEdge(HyperEdgeId)` + `TemporalRef(NodeId, i64)` variants. Add tag byte serialization (2=HyperEdge, 3=TemporalRef). Unit tests in `#[cfg(feature = "hypergraph")] mod hypergraph_tests`. |
| `cypherlite-storage/src/hyperedge/mod.rs` | **New** | `HyperEdgeStore` with `BTreeMap<u64, HyperEdgeRecord>`, `next_id: u64` auto-increment, CRUD API following `SubgraphStore` pattern (`cypherlite-storage/src/subgraph/mod.rs`, 87 lines). |
| `cypherlite-storage/src/hyperedge/reverse_index.rs` | **New** | Reverse index `BTreeMap<u64, Vec<u64>>` (participant -> hyperedge_ids) following `MembershipIndex` pattern (`cypherlite-storage/src/subgraph/membership.rs`, 224 lines). Idempotent add, automatic cleanup on remove. |
| `cypherlite-storage/src/lib.rs` | Extend | Add `#[cfg(feature = "hypergraph")] pub mod hyperedge;` import, `hyperedge_store: HyperEdgeStore` field in `StorageEngine`, CRUD public API (`create_hyperedge()`, `get_hyperedge()`, `delete_hyperedge()`, `scan_hyperedges()`), header sync for `next_hyperedge_id`. |
| `cypherlite-storage/src/page/mod.rs` | Extend | `FORMAT_VERSION = 5` when `cfg(feature = "hypergraph")`. Add `hyperedge_root_page: u64` (bytes 64-71), `next_hyperedge_id: u64` (bytes 72-79) to `DatabaseHeader`. Add `FLAG_HYPERGRAPH = 1 << 3`. Extend `to_page()`, `from_page()` with v5 auto-migration. |

### Phase 7c+7d: Query Support (Priority: Secondary Goal)

**Scope**: Lexer HYPEREDGE keyword, parser, AST nodes, planner HyperEdgeScan, executor.

**Requirements covered**: MM-001, MM-002, MM-003, MM-004, OO-001, OO-002, OO-003

**File impact**:

| File | Change Type | Description |
|------|------------|-------------|
| `cypherlite-query/src/lexer/mod.rs` | Extend | Add `#[regex("(?i)hyperedge", priority = 10)] Hyperedge` keyword (reuse existing FROM/TO/AT/TIME tokens from P4/P6). |
| `cypherlite-query/src/parser/ast.rs` | Extend | Add `#[cfg(feature = "hypergraph")] CreateHyperedge(CreateHyperedgeClause)` variant to `Clause` enum. Add `CreateHyperedgeClause { variable, labels, properties, sources: Vec<Expression>, targets: Vec<Expression> }`. Add `MatchHyperedgeClause`. |
| `cypherlite-query/src/parser/mod.rs` | Extend | Add `parse_create_hyperedge_clause()` method following `parse_create_snapshot_clause()` pattern (line 68). Detect `CREATE HYPEREDGE` keyword sequence, parse `FROM (...) TO (...)` participant lists with AT TIME support. |
| `cypherlite-query/src/planner/mod.rs` | Extend | Add `#[cfg(feature = "hypergraph")] CreateHyperedgeOp { variable, properties, sources, targets }` and `HyperEdgeScan { variable, filter }` to `LogicalPlan` enum. Planning logic: detect hyperedge patterns, evaluate source/target expressions. |
| `cypherlite-query/src/executor/mod.rs` | Extend | Add `#[cfg(feature = "hypergraph")] Hyperedge(cypherlite_core::HyperEdgeId)` to `Value` enum. Add dispatch arms for `CreateHyperedgeOp` and `HyperEdgeScan`. Implement `:INVOLVES` virtual relationship resolution via reverse index (same pattern as `:CONTAINS` in SubgraphScan). |

### Phase 7e: Temporal References + Quality (Priority: Final Goal)

**Scope**: TemporalRef resolution via VersionStore, proptest, benchmarks, integration tests, version bump to 0.7.0.

**Requirements covered**: NN-001, NN-002, NN-003, PP-001 through PP-005

**File impact**:

| File | Change Type | Description |
|------|------------|-------------|
| `cypherlite-query/src/executor/mod.rs` | Extend | TemporalRef lazy resolution: when properties of a `TemporalRef(node_id, timestamp)` participant are accessed, call `VersionStore::get_version_chain(node_id)` and find the version at or before `timestamp`. Cache resolved versions per execution context. |
| `cypherlite-storage/src/version/mod.rs` | Read-only reference | Existing API: `get_version_chain(entity_id) -> Vec<(u64, &VersionRecord)>`, `snapshot_node()`, `get_version()`. No modification needed; used as resolution backend. |
| `cypherlite-storage/tests/proptest_hypergraph.rs` | **New** | Proptest invariants: reverse index consistency (every participant in a HyperEdgeRecord appears in reverse index), create-get-delete roundtrip, GraphEntity serialization roundtrip. Follow pattern at `cypherlite-storage/tests/proptest_storage.rs` (500+ lines). |
| `cypherlite-query/tests/hypergraph.rs` | **New** | Integration tests: CREATE HYPEREDGE, MATCH HYPEREDGE, `:INVOLVES` traversal, temporal reference resolution, property access on TemporalRef. Follow pattern at `cypherlite-query/tests/subgraph.rs`. |
| `cypherlite-query/benches/hypergraph.rs` | **New** | Criterion benchmarks: hyperedge creation (1000 hyperedges), reverse index lookup, HyperEdgeScan with filter. `required-features = ["hypergraph"]`. Follow pattern at `cypherlite-query/benches/subgraph.rs`. |
| `Cargo.toml` (all crates) | Extend | Version bump to 0.7.0. No feature flag changes needed (hypergraph already defined in all three crate Cargo.toml files). |

### Phase Dependencies

```
Phase 7a+7b (Core Storage)
    |
    +---> Phase 7c+7d (Query Support) -- depends on HyperEdgeStore API
    |
    +---> Phase 7e (Temporal + Quality) -- depends on query pipeline
                                           + VersionStore (already exists)
```

> All phases gated behind `#[cfg(feature = "hypergraph")]`. Feature flag chain already defined: `temporal-core -> temporal-edge -> subgraph -> hypergraph` in all three crate Cargo.toml files.

---

## 9. Impact Analysis on Existing SPECs

### 9.1 Impact Matrix

| SPEC | Version | Impact | Affected Files | Details |
|------|---------|:------:|:-:|---------|
| DB-001 (Core Storage) | v0.1 | LOW | 2 | `types.rs` GraphEntity extension, `page/mod.rs` Header v5 — all behind `#[cfg(feature = "hypergraph")]`, no change to existing code |
| DB-002 (Basic Cypher) | v0.2 | LOW | 3 | Clause enum, Token enum, execute() dispatch — feature-gated new arms only, existing arms unchanged |
| DB-003 (Advanced Query) | v0.3 | LOW | 2 | `eval_property_access()`, LogicalPlan dispatch — new variant arms added only |
| DB-004 (Temporal Core) | v0.4 | LOW | 1 | VersionStore used as read-only reference for TemporalRef resolution. Existing API unchanged |
| DB-005 (Temporal Edge) | v0.5 | NONE | 0 | Edge temporal properties and hyperedges are independent. RelationshipRecord does not need HyperEdge endpoint support (separate store) |
| DB-006 (Subgraph) | v0.6 | MEDIUM | 1 | GraphEntity enum directly extended — existing `match` patterns need new arms when `hypergraph` feature enabled |

### 9.2 Feature Gate Isolation

All SPEC-DB-007 changes are behind `#[cfg(feature = "hypergraph")]`:

- `hypergraph` OFF → existing code 100% identical (excluded from compilation)
- `hypergraph` ON → new match arms added to existing patterns

Existing tests (306 on default, 1,144 with all features) are unaffected when `hypergraph` feature is disabled.

### 9.3 Exhaustive Match Modifications Required (hypergraph ON)

| # | File | Pattern | Current Arms | New Arms |
|---|------|---------|:---:|----------|
| 1 | `executor/mod.rs::execute()` | LogicalPlan match | 26 | +2 (HyperEdgeScan, CreateHyperedge) |
| 2 | `executor/eval.rs::eval_property_access()` | Value match | 5 | +1 (Hyperedge) |
| 3 | `executor/mod.rs::TryFrom<Value>` | Value match | 3 reject | +1 (Hyperedge reject) |
| 4 | `planner/mod.rs::plan_clause()` | Clause match | 12 | +1 (CreateHyperedge) |
| 5 | `types.rs::from_entities()` | GraphEntity match | 2 | +2 (HyperEdge, TemporalRef) |
| 6 | `types.rs::start_entity()/end_entity()` | GraphEntity match | 2 | +2 (HyperEdge, TemporalRef) |

### 9.4 Wildcard Arms Review (24 locations)

Files with `_ =>` catch-all arms that will silently handle new Value variants:
- `create.rs`, `delete.rs`, `expand.rs`, `merge.rs`, `temporal_scan.rs`
- Phase 7c+7d must review all wildcard arms and add explicit Hyperedge error handling where appropriate.

### 9.5 Backward Compatibility Guarantee

| Condition | Verdict |
|-----------|---------|
| Existing SPEC compatibility | SAFE — cfg(feature) gate fully isolates changes |
| Compilation (default features) | SAFE — identical to current codebase |
| Compilation (subgraph feature) | SAFE — hypergraph code excluded |
| Test compatibility (PP-001) | SAFE — existing 306 tests unaffected |
| DatabaseHeader migration | SAFE — v4 → v5 auto-migration follows v3 → v4 pattern |

---

## 10. 구현 노트 (Implementation Notes)

### 10.1 구현 요약

SPEC-DB-007에 정의된 네이티브 하이퍼엣지 기능이 원래 명세의 범위에서 벗어남 없이 완전히 구현되었습니다.

### 10.2 구현 단계

**Phase 7a+7b — 코어 스토리지**
- `HyperEdgeStore`: BTreeMap 기반, 자동 증분 ID (u64) 방식으로 구현
- `HyperEdgeReverseIndex`: 엔티티 → 하이퍼엣지 양방향 매핑
- `DatabaseHeader v5`: `hyperedge_root_page`, `next_hyperedge_id` 필드 추가
- `GraphEntity` 열거형 확장: `HyperEdge(HyperEdgeId)`, `TemporalRef { node_id, timestamp }` 변형 추가
- `HyperEdgeId`, `HyperEdgeRecord` 핵심 타입 정의 (`cypherlite-core`)

**Phase 7c+7d — 쿼리 지원**
- Lexer: `HYPEREDGE`, `INVOLVES`, `AT TIME` 키워드 토큰 추가
- Parser: `CREATE HYPEREDGE`, `MATCH HYPEREDGE ... INVOLVES` 구문 파싱
- Planner: `CreateHyperedge`, `HyperEdgeScan` 논리 계획 노드 생성
- Executor: `HyperEdgeScanOperator` 구현, 가상 `:INVOLVES` 관계 확장
- `TemporalRef` 지연 해결: `VersionStore` 체인 워크를 통한 과거 시점 노드 참조

**Phase 7e — 시간 차원 및 품질**
- `TemporalRef` 완전 통합: `Node AT TIME timestamp` 구문으로 하이퍼엣지 내 시간 참조 지원
- proptest 기반 속성 기반 테스트 추가
- criterion 벤치마크 스위트 업데이트
- 통합 테스트 보강

### 10.3 테스트 결과

| 항목 | 결과 |
|------|------|
| 전체 테스트 수 | 1,241개 |
| 통과율 | 100% |
| 코드 커버리지 | 93.56% |
| 목표 커버리지 | 85% |

### 10.4 버전 및 범위

- **릴리즈 버전**: v0.7.0
- **범위 이탈**: 없음 — 원래 SPEC 명세와 완전히 일치하는 구현 완료
