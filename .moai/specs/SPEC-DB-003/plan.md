---
id: SPEC-DB-003
type: plan
version: "1.0.0"
status: draft
created: "2026-03-10"
updated: "2026-03-10"
author: epsilondelta
priority: P1
tags: [advanced-query, with, merge, optional-match, unwind, variable-length-paths, indexing, optimization]
---

# SPEC-DB-003 Implementation Plan: CypherLite Phase 3 - Advanced Query Features

---

## HISTORY

| Version | Date | Changes |
|---------|------|---------|
| 1.0.0 | 2026-03-10 | Initial SPEC creation based on deep research analysis |

---

## 1. Technology Stack

| Technology | Version | Purpose | Rationale |
|------------|---------|---------|-----------|
| Existing B+Tree (cypherlite-storage) | N/A | Property index backing store | Reuse proven Phase 1 B+Tree; no new dependencies needed |
| Hand-written recursive descent parser | N/A | UNWIND + variable-length path parsing | Extend existing parser; consistent with Phase 2 approach |
| Rule-based query planner | N/A | Index-aware plan selection | Evolve Phase 2 planner with index scan rules |
| Volcano iterator model | N/A | New operator execution | Consistent with Phase 2 executor architecture |

### Rejected Alternatives

| Alternative | Rejection Rationale |
|-------------|---------------------|
| External index library (tantivy, etc.) | Overkill for property indexes; B+Tree already exists in-crate |
| Cost-based optimizer | Insufficient statistics infrastructure in Phase 3; rule-based sufficient for single-property indexes |
| Hash index for equality lookups | B+Tree supports both equality and range; hash would require separate structure for range queries |
| `petgraph` for path traversal | Adds external dependency; BFS/DFS on existing adjacency list is simpler and avoids data duplication |

---

## 2. Crate Structure (Phase 3 Additions)

```
crates/
  cypherlite-storage/
    src/
      index/                 [NEW] Property index module
        mod.rs               PropertyIndex struct, IndexManager
        btree_index.rs       B+Tree-backed property index implementation
      catalog/
        mod.rs               + get_label_id(), get_type_id() read-only lookups
                             + index definition storage
      lib.rs                 + find_node(), find_edge(), index scan APIs

  cypherlite-query/
    src/
      lexer/
        mod.rs               + UNWIND keyword token
      parser/
        ast.rs               + UnwindClause, MergeClause ON MATCH/ON CREATE
                             + RelationshipPattern.min_hops/max_hops
        clause.rs            + parse_unwind_clause(), extend parse_merge_clause()
        pattern.rs           + variable-length path syntax (*N..M)
      semantic/
        mod.rs               + WITH scope reset, UNWIND variable binding
                             + OPTIONAL MATCH null-variable handling
                             + MERGE pattern validation
      planner/
        mod.rs               + LogicalPlan::With, Unwind, MergeOp, OptionalExpand, VarLengthExpand, IndexScan
        optimize.rs          + index scan selection, LIMIT pushdown, constant folding, projection pruning
      executor/
        operators/
          with.rs            [NEW] WithOp (projection + scope barrier)
          unwind.rs          [NEW] UnwindOp (list expansion)
          optional_expand.rs [NEW] OptionalExpandOp (left join)
          merge.rs           [NEW] MergeOp (match-or-create)
          var_length_expand.rs [NEW] VarLengthExpandOp (BFS traversal)
          index_scan.rs      [NEW] IndexScanOp (property index lookup)
      api/
        mod.rs               + CREATE INDEX / DROP INDEX DDL support
```

---

## 3. Task Decomposition

### Phase 3a: Core Clauses (Foundation)

#### Group L: WITH Clause (scope barrier -- enables multi-clause pipeline queries)

| Task | Description | Crate | Files | Dependencies | Est. Tests | Priority |
|------|------------|-------|-------|--------------|------------|----------|
| TASK-060 | Implement WITH scope reset in `SemanticAnalyzer`: after WITH, only projected variables survive in scope | query | `semantic/mod.rs` | None | ~12 | High |
| TASK-061 | Add `LogicalPlan::With { source, items, where_clause, distinct }` variant to planner | query | `planner/mod.rs` | TASK-060 | ~8 | High |
| TASK-062 | Implement `WithOp` executor operator: reuse `ProjectOp` logic with scope variable filtering; support WITH WHERE | query | `executor/operators/with.rs`, `executor/mod.rs` | TASK-061 | ~15 | High |
| TASK-063 | Implement WITH + aggregation: `WITH a, count(*) AS cnt` reusing `AggregateOp` with scope barrier | query | `executor/operators/with.rs`, `executor/operators/aggregate.rs` | TASK-062 | ~10 | High |
| TASK-064 | Implement WITH DISTINCT: deduplication before passing to next clause | query | `executor/operators/with.rs` | TASK-062 | ~6 | Medium |
| TASK-065 | Integration tests: `MATCH...WITH...RETURN`, `WITH WHERE`, `WITH DISTINCT`, `WITH + aggregation` | query | `tests/` | TASK-062 | ~20 | High |

#### Group M: UNWIND Clause (list expansion)

| Task | Description | Crate | Files | Dependencies | Est. Tests | Priority |
|------|------------|-------|-------|--------------|------------|----------|
| TASK-066 | Add `Unwind` keyword token to lexer (verify if already reserved, add if not) | query | `lexer/mod.rs` | None | ~3 | High |
| TASK-067 | Add `Clause::Unwind(UnwindClause)` AST node with `{ expr: Expression, variable: String }` | query | `parser/ast.rs` | TASK-066 | ~2 | High |
| TASK-068 | Implement `parse_unwind_clause()`: `UNWIND expr AS variable` | query | `parser/clause.rs` | TASK-067 | ~8 | High |
| TASK-069 | Add UNWIND variable binding in semantic analyzer | query | `semantic/mod.rs` | TASK-068 | ~5 | High |
| TASK-070 | Add `LogicalPlan::Unwind { source, expr, variable }` variant | query | `planner/mod.rs` | TASK-069 | ~4 | High |
| TASK-071 | Implement `UnwindOp` executor: for each source record, evaluate expr to list, emit one row per element | query | `executor/operators/unwind.rs`, `executor/mod.rs` | TASK-070 | ~12 | High |
| TASK-072 | Handle edge cases: UNWIND on empty list (produces zero rows), UNWIND on non-list (error), UNWIND on NULL (zero rows) | query | `executor/operators/unwind.rs` | TASK-071 | ~8 | High |
| TASK-073 | Integration tests: `UNWIND [1,2,3] AS x RETURN x`, `UNWIND n.friends AS f`, empty list, NULL list | query | `tests/` | TASK-071 | ~12 | High |

#### Group N: OPTIONAL MATCH (left join semantics)

| Task | Description | Crate | Files | Dependencies | Est. Tests | Priority |
|------|------------|-------|-------|--------------|------------|----------|
| TASK-074 | Add OPTIONAL MATCH handling in semantic analyzer: register variables as potentially-null | query | `semantic/mod.rs` | None | ~6 | High |
| TASK-075 | Add `LogicalPlan::OptionalExpand` variant (or flag on Expand) in planner | query | `planner/mod.rs` | TASK-074 | ~6 | High |
| TASK-076 | Implement `OptionalExpandOp`: execute inner plan; if empty result for source record, emit record with NULL bindings for unmatched variables | query | `executor/operators/optional_expand.rs`, `executor/mod.rs` | TASK-075 | ~15 | High |
| TASK-077 | Handle NULL propagation in expressions: ensure WHERE, property access, and aggregations handle NULL from OPTIONAL MATCH correctly | query | `executor/eval.rs` | TASK-076 | ~10 | High |
| TASK-078 | Integration tests: `MATCH (a) OPTIONAL MATCH (a)-[:KNOWS]->(b) RETURN a, b`, NULL propagation, chained OPTIONAL MATCH | query | `tests/` | TASK-076 | ~15 | High |

**Phase 3a Total: 19 tasks (TASK-060 ~ TASK-078), ~177 estimated tests**

---

### Phase 3b: MERGE + Indexing

#### Group O: Storage API Extensions for MERGE

| Task | Description | Crate | Files | Dependencies | Est. Tests | Priority |
|------|------------|-------|-------|--------------|------------|----------|
| TASK-079 | Add `Catalog::get_label_id(name) -> Option<u32>` and `Catalog::get_type_id(name) -> Option<u32>` read-only lookups | storage | `catalog/mod.rs` | None | ~6 | High |
| TASK-080 | Implement `StorageEngine::find_node(label_ids, properties) -> Option<NodeId>`: scan nodes by label, filter by property equality | storage | `lib.rs` | TASK-079 | ~10 | High |
| TASK-081 | Implement `StorageEngine::find_edge(start, end, type_id) -> Option<EdgeId>`: lookup edge by endpoints and type | storage | `lib.rs` | TASK-079 | ~8 | High |
| TASK-082 | Unit tests for find_node (match, no-match, multi-label) and find_edge (match, no-match, multiple edges) | storage | `tests/` | TASK-080, TASK-081 | ~12 | High |

#### Group P: MERGE Clause Implementation

| Task | Description | Crate | Files | Dependencies | Est. Tests | Priority |
|------|------------|-------|-------|--------------|------------|----------|
| TASK-083 | Extend `MergeClause` AST: add `on_match: Vec<SetItem>` and `on_create: Vec<SetItem>` fields | query | `parser/ast.rs` | None | ~3 | High |
| TASK-084 | Extend `parse_merge_clause()` to parse `ON MATCH SET ...` and `ON CREATE SET ...` actions | query | `parser/clause.rs` | TASK-083 | ~8 | High |
| TASK-085 | Add MERGE pattern validation in semantic analyzer: verify MERGE patterns have sufficient property constraints for matching | query | `semantic/mod.rs` | TASK-084 | ~6 | High |
| TASK-086 | Add `LogicalPlan::MergeOp { source, pattern, on_match, on_create }` variant | query | `planner/mod.rs` | TASK-085 | ~4 | High |
| TASK-087 | Implement `MergeOp` executor (basic): 1) Try find_node/find_edge match 2) If not found, create 3) Bind result to variable | query | `executor/operators/merge.rs`, `executor/mod.rs` | TASK-086, TASK-080, TASK-081 | ~15 | High |
| TASK-088 | Implement MERGE ON MATCH SET / ON CREATE SET actions in executor | query | `executor/operators/merge.rs` | TASK-087 | ~10 | High |
| TASK-089 | MERGE atomicity: ensure match-then-create sequence is atomic within a write transaction | query | `executor/operators/merge.rs` | TASK-087 | ~6 | High |
| TASK-090 | Integration tests: MERGE create-if-not-exists, MERGE idempotency, MERGE with ON MATCH/ON CREATE, MERGE relationship | query | `tests/` | TASK-088 | ~20 | High |

#### Group Q: Property Index Infrastructure

| Task | Description | Crate | Files | Dependencies | Est. Tests | Priority |
|------|------------|-------|-------|--------------|------------|----------|
| TASK-091 | Create `index/` module in cypherlite-storage: `PropertyIndex` trait, `IndexManager` struct | storage | `index/mod.rs` | None | ~4 | High |
| TASK-092 | Implement B+Tree-backed `PropertyIndex`: `(label_id, prop_key_id, PropertyValue) -> Vec<NodeId>` | storage | `index/btree_index.rs` | TASK-091 | ~15 | High |
| TASK-093 | Implement `IndexManager::create_index(label, property)` and `drop_index(name)` | storage | `index/mod.rs` | TASK-092 | ~8 | High |
| TASK-094 | Store index definitions in Catalog; load/save with catalog persistence | storage | `catalog/mod.rs`, `index/mod.rs` | TASK-093 | ~8 | High |
| TASK-095 | Auto-update indexes on CREATE node, SET property, DELETE node operations | storage | `lib.rs`, `index/mod.rs` | TASK-094 | ~12 | High |
| TASK-096 | Implement `StorageEngine::scan_nodes_by_property(label_id, prop_key, value) -> Vec<NodeId>` using index | storage | `lib.rs` | TASK-095 | ~8 | High |
| TASK-097 | Implement `StorageEngine::scan_nodes_by_range(label_id, prop_key, min, max) -> Vec<NodeId>` using index | storage | `lib.rs` | TASK-095 | ~8 | Medium |
| TASK-098 | Parse `CREATE INDEX [name] ON :Label(property)` and `DROP INDEX name` DDL in query engine | query | `parser/clause.rs`, `parser/ast.rs`, `lexer/mod.rs` | TASK-093 | ~10 | High |
| TASK-099 | Wire DDL execution through `CypherLite::execute()` to `IndexManager` | query | `api/mod.rs`, `executor/mod.rs` | TASK-098 | ~6 | High |
| TASK-100 | Unit + integration tests: index creation, index-assisted lookup, index auto-update on mutation, DROP INDEX | storage, query | `tests/` | TASK-099 | ~25 | High |

**Phase 3b Total: 22 tasks (TASK-079 ~ TASK-100), ~194 estimated tests**

---

### Phase 3c: Advanced Patterns + Optimization

#### Group R: Variable-Length Paths

| Task | Description | Crate | Files | Dependencies | Est. Tests | Priority |
|------|------------|-------|-------|--------------|------------|----------|
| TASK-101 | Add `min_hops: Option<u32>` and `max_hops: Option<u32>` to `RelationshipPattern` AST node | query | `parser/ast.rs` | None | ~2 | High |
| TASK-102 | Extend `parse_relationship_pattern()` to handle `[*]`, `[*N]`, `[*N..M]`, `[:TYPE*N..M]` syntax | query | `parser/pattern.rs` | TASK-101 | ~12 | High |
| TASK-103 | Remove `UnsupportedSyntax` for variable-length paths; add semantic validation (max_hops <= configurable limit) | query | `semantic/mod.rs` | TASK-102 | ~6 | High |
| TASK-104 | Add `LogicalPlan::VarLengthExpand { source, rel_type, direction, min_hops, max_hops }` variant | query | `planner/mod.rs` | TASK-103 | ~4 | High |
| TASK-105 | Apply configurable default max_hops (default: 10) in planner when unbounded `[*]` is used | query | `planner/mod.rs` | TASK-104 | ~4 | High |
| TASK-106 | Implement `VarLengthExpandOp`: BFS traversal with depth tracking | query | `executor/operators/var_length_expand.rs`, `executor/mod.rs` | TASK-105 | ~18 | High |
| TASK-107 | Implement cycle detection in `VarLengthExpandOp`: track visited edges to prevent infinite loops in cyclic graphs | query | `executor/operators/var_length_expand.rs` | TASK-106 | ~10 | High |
| TASK-108 | Support exact-hop matching `[*2]` (min_hops == max_hops) and zero-length paths `[*0..1]` | query | `executor/operators/var_length_expand.rs` | TASK-106 | ~8 | Medium |
| TASK-109 | Integration tests: bounded paths `[*1..3]`, unbounded `[*]` with max cap, cycle detection, exact hop, typed paths `[:KNOWS*2]` | query | `tests/` | TASK-107 | ~20 | High |
| TASK-110 | Property-based tests (proptest): random graph generation + variable-length path depth limit invariants | query | `tests/` | TASK-107 | ~5 | Medium |

#### Group S: Query Optimization Rules

| Task | Description | Crate | Files | Dependencies | Est. Tests | Priority |
|------|------------|-------|-------|--------------|------------|----------|
| TASK-111 | Add `LogicalPlan::IndexScan { label_id, prop_key, value }` variant | query | `planner/mod.rs` | TASK-096 | ~3 | High |
| TASK-112 | Implement index scan selection rule: when property index exists, replace NodeScan+Filter with IndexScan | query | `planner/optimize.rs` | TASK-111 | ~10 | High |
| TASK-113 | Implement MERGE short-circuit: use index lookup for MERGE match-check before full label scan | query | `executor/operators/merge.rs`, `planner/optimize.rs` | TASK-112 | ~6 | Medium |
| TASK-114 | Implement LIMIT pushdown: push LIMIT into NodeScan/Expand to stop early | query | `planner/optimize.rs` | None | ~8 | Medium |
| TASK-115 | Implement constant folding: evaluate constant expressions at plan time (e.g., `1 + 2` -> `3`) | query | `planner/optimize.rs` | None | ~8 | Medium |
| TASK-116 | Implement projection pruning: remove unused columns early in the pipeline | query | `planner/optimize.rs` | None | ~8 | Medium |
| TASK-117 | Integration tests: index vs full scan comparison, LIMIT pushdown verification, constant folding correctness | query | `tests/` | TASK-112, TASK-114, TASK-115 | ~15 | High |

#### Group T: Quality and Performance Validation

| Task | Description | Crate | Files | Dependencies | Est. Tests | Priority |
|------|------------|-------|-------|--------------|------------|----------|
| TASK-118 | Property-based tests: OPTIONAL MATCH correctness on random graphs, UNWIND length invariants | query | `tests/` | TASK-076, TASK-071 | ~8 | Medium |
| TASK-119 | Criterion benchmarks: index scan vs full scan, variable-length path scaling, MERGE vs MATCH+CREATE | query | `benches/` | TASK-112, TASK-107 | ~6 | Medium |
| TASK-120 | Version bump to 0.3.0; update public API documentation for new features | all | `Cargo.toml`, `api/mod.rs` | All above | ~2 | High |

**Phase 3c Total: 20 tasks (TASK-101 ~ TASK-120), ~148 estimated tests**

---

## 4. Critical Path (Dependency Order)

```
Phase 3a (Core Clauses):
  L (WITH) ─────────────────────┐
  M (UNWIND: Lexer→AST→Parser→  │
     Semantic→Planner→Executor) ─┼──> Phase 3b
  N (OPTIONAL MATCH) ───────────┘

Phase 3b (MERGE + Indexing):
  O (Storage APIs) ──> P (MERGE Clause) ──┐
  Q (Property Index Infrastructure) ──────┼──> Phase 3c

Phase 3c (Advanced + Optimization):
  R (Variable-Length Paths) ──────────────┐
  S (Query Optimization Rules) ───────────┼──> T (Quality Gate)
```

Within Phase 3a, Groups L, M, and N are largely independent and can be developed in parallel.
Within Phase 3b, Group O must precede Group P; Group Q is independent of P.
Phase 3c depends on Phase 3b completion (index infrastructure required for optimization rules).

---

## 5. Milestone Ordering

### Primary Goal: Phase 3a -- Core Clauses (WITH, UNWIND, OPTIONAL MATCH)

- Group L: WITH clause implementation (TASK-060 ~ TASK-065)
- Group M: UNWIND clause full stack (TASK-066 ~ TASK-073)
- Group N: OPTIONAL MATCH left join (TASK-074 ~ TASK-078)

**Success Criteria**: Multi-clause pipeline queries work end-to-end. `MATCH...WITH...RETURN`, `UNWIND [...] AS x`, and `OPTIONAL MATCH` with NULL propagation all produce correct results.

### Secondary Goal: Phase 3b -- MERGE + Indexing

- Group O: Storage API extensions (TASK-079 ~ TASK-082)
- Group P: MERGE clause full implementation (TASK-083 ~ TASK-090)
- Group Q: Property index infrastructure (TASK-091 ~ TASK-100)

**Success Criteria**: MERGE is idempotent, ON MATCH/ON CREATE actions execute correctly. `CREATE INDEX` DDL works and index-assisted queries return correct results. All indexes auto-update on mutations.

### Final Goal: Phase 3c -- Advanced Patterns + Optimization

- Group R: Variable-length path traversal (TASK-101 ~ TASK-110)
- Group S: Query optimization rules (TASK-111 ~ TASK-117)
- Group T: Quality and performance validation (TASK-118 ~ TASK-120)

**Success Criteria**: Variable-length paths with cycle detection work correctly. Index-based query plans demonstrate measurable performance improvement over full scans. All quality gates (85%+ coverage, zero clippy warnings) pass.

### Optional Goal: Extended Optimizations

- Predicate decomposition: split AND predicates and push independently
- Join order optimization: reorder MATCH patterns for smallest intermediate result
- Composite indexes: `(label, prop1, prop2)` multi-property indexes

These are deferred to Phase 3+ or Phase 4 based on profiling results.

---

## 6. Risk Analysis

### R-010: MERGE Atomicity under Concurrent Access (HIGH)

- **Risk**: The match-then-create sequence in MERGE must be atomic. If the match and create are separate operations, a concurrent writer could create the same node between the match and create steps.
- **Mitigation**: StorageEngine uses a single-writer model (write transactions are serialized). MERGE's match-then-create executes within a single write transaction, which provides the necessary atomicity. Document this guarantee explicitly. Add test to verify idempotency under sequential write transactions.

### R-011: Variable-Length Path Memory Explosion (HIGH)

- **Risk**: Unbounded variable-length paths (`[*]`) on dense graphs can expand exponentially, exhausting memory.
- **Mitigation**: Enforce configurable `max_hops` limit (default: 10) in the planner. Track visited edges for cycle detection. Add `max_path_results` configuration (default: 10,000) to cap result set size. Return `ExecutionError` when limits are exceeded.

### R-012: WITH Scope Semantics Correctness (MEDIUM)

- **Risk**: Variable scoping after WITH is strict -- only projected variables survive. Incorrect scope reset could silently drop variables or leak variables from prior clauses.
- **Mitigation**: Comprehensive test suite for scope boundaries. Semantic analyzer creates a new scope after WITH that contains only the projected variables. Test cases specifically targeting variable leakage and missing variable errors.

### R-013: OPTIONAL MATCH Left Join in Volcano Model (MEDIUM)

- **Risk**: Generating NULL-padded records when no match is found requires careful handling in the pull-based Volcano model where operators request records one at a time.
- **Mitigation**: `OptionalExpandOp` buffers inner results per source record. If inner produces zero records for a given source record, emit one record with NULL bindings for all inner-bound variables. Test with source records that have varying match counts (0, 1, many).

### R-014: Index Consistency on Crash (MEDIUM)

- **Risk**: If a crash occurs after a node mutation but before the index is updated, the index becomes inconsistent with the data.
- **Mitigation**: Index updates are performed within the same write transaction as data mutations. Leverage WAL for crash consistency -- all mutations (data + index) are written to WAL atomically before commit. On recovery, WAL replay restores both data and index pages.

### R-015: UNWIND on Non-List Values (LOW)

- **Risk**: Users may attempt UNWIND on non-list property values (e.g., integer, string), causing unexpected behavior.
- **Mitigation**: Semantic analyzer and executor validate that UNWIND expression evaluates to a list. Return clear `ExecutionError("UNWIND requires a list expression, got: <type>")` for non-list values. UNWIND on NULL produces zero rows (matches Neo4j behavior).

### R-016: Index Storage Overhead (LOW)

- **Risk**: Property indexes consume additional storage and memory, which may be significant for large graphs.
- **Mitigation**: Indexes are optional and user-created via DDL. Document storage overhead expectations. No auto-indexing -- user controls which properties are indexed.

---

## 7. MX Tag Strategy

### @MX:ANCHOR Candidates (fan_in >= 3)

| Location | Rationale |
|----------|-----------|
| `StorageEngine::find_node()` | Called by MergeOp, potential index-assisted queries, and integration tests |
| `IndexManager::update_index()` | Called from CreateOp, SetPropsOp, DeleteOp on every mutation |
| `VarLengthExpandOp::next()` | Critical path for all variable-length traversals; complex cycle detection logic |

### @MX:WARN Candidates (complex lifecycle/safety)

| Location | Rationale |
|----------|-----------|
| `MergeOp` match-then-create sequence | Atomicity-critical; must execute within single write transaction |
| `VarLengthExpandOp` cycle detection | Memory management and visited-edge tracking in cyclic graphs |
| `WithOp` scope barrier | Scope reset semantics are subtle; variable leakage bugs are hard to detect |

### @MX:NOTE Candidates (design rationale documentation)

| Location | Rationale |
|----------|-----------|
| `PropertyIndex` B+Tree reuse | Why we reuse Phase 1 B+Tree instead of hash index or external library |
| `OptionalExpandOp` NULL padding | Left join semantics and why buffering is necessary in Volcano model |
| `UNWIND` NULL/empty list behavior | Documents Neo4j-compatible behavior: NULL produces zero rows |
| Index auto-update strategy | Why index updates are co-located with data mutations in same transaction |

---

## 8. Architecture Design Direction

### 8.1 WITH as Pipeline Barrier

WITH acts as a "mini-RETURN" that feeds into the next clause instead of producing output:

- Semantics: Projects specified columns and resets variable scope
- Implementation: Reuse `ProjectOp` logic with explicit scope filtering
- WITH WHERE: Apply filter after projection (different from MATCH WHERE)
- WITH + aggregation: Combine with `AggregateOp` before scope reset
- WITH DISTINCT: Apply deduplication before passing to next clause

### 8.2 MERGE Match-or-Create Pattern

MERGE follows a two-phase execution within a single write transaction:

1. Match phase: Use `find_node(labels, properties)` or `find_edge(start, end, type_id)` to check existence
2. Create phase: If no match found, execute CREATE path
3. Action phase: Apply ON MATCH SET or ON CREATE SET based on which phase produced the result
4. Binding: Bind the matched-or-created entity to the MERGE variable

### 8.3 OPTIONAL MATCH Left Join

OPTIONAL MATCH preserves all source records, padding with NULL when no match exists:

- Source records from prior MATCH are preserved unconditionally
- Inner expansion (edge traversal) is attempted for each source record
- If inner produces zero results: emit source record with NULL for all new variables
- If inner produces N results: emit N records (same as regular MATCH)

### 8.4 Property Index Architecture

Single-property B+Tree indexes stored separately from data:

- Key: `(label_id, prop_key_id, PropertyValue)` composite
- Value: `Vec<NodeId>` (list of matching nodes)
- One B+Tree per (label, property) pair
- Index definitions stored in Catalog for persistence
- Auto-updated on CREATE, SET, DELETE within the same write transaction
- Planner selects index scan when matching index exists and predicate is equality or range

### 8.5 Variable-Length Path Traversal

BFS-based traversal with safety guarantees:

- BFS is preferred over DFS for shortest-path-first ordering
- Visited edge tracking (HashSet<EdgeId>) prevents cycles
- Configurable max_hops (default: 10) prevents unbounded expansion
- Each depth level produces results within [min_hops, max_hops] range
- Memory bound: O(visited_edges + frontier_size)

---

## 9. Phase 2 Crate Modifications (Phase 3 Scope)

### cypherlite-storage Changes

1. `catalog/mod.rs`: Add `get_label_id()`, `get_type_id()` read-only lookups; add index definition storage
2. `lib.rs`: Add `find_node()`, `find_edge()`, `scan_nodes_by_property()`, `scan_nodes_by_range()` methods
3. `index/` module (NEW): PropertyIndex trait, IndexManager, B+Tree-backed index implementation
4. Auto-update hooks: Index maintenance on create/set/delete operations

### cypherlite-query Changes

1. `lexer/mod.rs`: Add UNWIND keyword token; add INDEX, ON, CREATE INDEX tokens
2. `parser/ast.rs`: Add UnwindClause, extend MergeClause (ON MATCH/ON CREATE), add RelationshipPattern hops
3. `parser/clause.rs`: Add parse_unwind_clause(), extend parse_merge_clause(), add DDL parsing
4. `parser/pattern.rs`: Add variable-length path syntax parsing
5. `semantic/mod.rs`: WITH scope reset, UNWIND variable binding, OPTIONAL MATCH handling, MERGE validation
6. `planner/mod.rs`: New LogicalPlan variants (With, Unwind, MergeOp, OptionalExpand, VarLengthExpand, IndexScan)
7. `planner/optimize.rs`: Index scan selection, LIMIT pushdown, constant folding, projection pruning
8. `executor/mod.rs`: Dispatch for 6 new operator types
9. `executor/operators/`: 6 new operator files (with.rs, unwind.rs, optional_expand.rs, merge.rs, var_length_expand.rs, index_scan.rs)

### cypherlite-core Changes

1. `error.rs`: Possible new error variants for index operations

---

## 10. Summary

| Metric | Value |
|--------|-------|
| Total Tasks | 61 (TASK-060 ~ TASK-120) |
| Total Groups | 9 (L through T) |
| Estimated Tests | ~519 |
| New Files | ~8 (6 operators + 2 index module files) |
| Modified Files | ~15 |
| Sub-phases | 3 (3a: Core Clauses, 3b: MERGE + Indexing, 3c: Advanced + Optimization) |
