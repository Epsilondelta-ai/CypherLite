# SPEC-DB-003 Deep Research: Phase 3 Advanced Query Features

## 1. P2 Clause Readiness Matrix

### Already Parsed (AST exists)
| Clause | AST Node | Parser | Semantic | Planner | Executor |
|--------|----------|--------|----------|---------|----------|
| WITH | `WithClause` (ast.rs:86-90) | `parse_with_clause` (clause.rs:127) | NOT handled | NOT planned | NOT implemented |
| MERGE | `MergeClause` (ast.rs:93-95) | `parse_merge_clause` (clause.rs:149) | NOT handled | NOT planned | NOT implemented |
| OPTIONAL MATCH | `MatchClause { optional: true }` (ast.rs:23-27) | `parse_match_clause(true)` (clause.rs:15) | NOT handled | NOT planned | NOT implemented |
| ORDER BY | `ReturnClause.order_by` (ast.rs:33) | Parsed in RETURN | Handled | `Sort` plan node | `execute_sort` |
| SKIP | `ReturnClause.skip` (ast.rs:34) | Parsed in RETURN | Handled | `Skip` plan node | `execute_skip` |
| LIMIT | `ReturnClause.limit` (ast.rs:35) | Parsed in RETURN | Handled | `Limit` plan node | `execute_limit` |

### NOT Parsed (Needs new AST + parser)
| Clause | Status | Notes |
|--------|--------|-------|
| UNWIND | No `Clause::Unwind` variant | Need `UnwindClause { expr, variable }` AST node |
| Variable-length paths | No `RelationshipPattern.length` field | Need `min_hops: Option<u32>, max_hops: Option<u32>` on `RelationshipPattern` |

### Key Observations
- ORDER BY, SKIP, LIMIT are **fully implemented** in Phase 2 within RETURN clause scope. No additional work needed.
- WITH, MERGE, OPTIONAL MATCH have parser support but zero downstream pipeline (semantic, planner, executor).
- UNWIND and variable-length paths require changes at every layer (lexer through executor).

## 2. Existing Operator Patterns (Reference Implementations)

### Pattern A: Streaming (Filter, Project)
- Input: `Vec<Record>` from source plan
- Process: Transform/filter each record independently
- Files: `executor/operators/filter.rs`, `executor/operators/project.rs`
- Reuse for: WITH (projection + scope reset), UNWIND (record expansion)

### Pattern B: Stateful Aggregation (Aggregate)
- Input: Collect all source records into groups
- Process: Apply aggregate functions per group
- File: `executor/operators/aggregate.rs`
- Reuse for: WITH + aggregation (GROUP BY equivalent)

### Pattern C: Record Expansion (Expand)
- Input: Source records, expand each by following edges
- Process: Cross-product of source record x matching edges
- File: `executor/operators/expand.rs`
- Reuse for: Variable-length path traversal (recursive expand), UNWIND (list expansion)

### Pattern D: Mutation (Create, Delete, Set)
- Input: Source records, mutate storage engine
- Process: Create/delete/update entities, return updated records
- Files: `executor/operators/create.rs`, `delete.rs`, `set_props.rs`
- Reuse for: MERGE (conditional create-or-match)

## 3. Storage Engine API Gaps

### Current Public API (`StorageEngine`, lib.rs)
- `create_node`, `get_node`, `update_node`, `delete_node`
- `create_edge`, `get_edge`, `get_edges_for_node`, `delete_edge`
- `scan_nodes`, `scan_nodes_by_label`, `scan_edges_by_type`
- `begin_read`, `begin_write`, `wal_commit`, `wal_discard`, `checkpoint`
- `catalog()`, `catalog_mut()`, `node_count()`, `edge_count()`

### Missing APIs for Phase 3
1. **Property Index Scan**: `scan_nodes_by_property(label_id, prop_key, value) -> Vec<NodeId>` -- needed for MERGE match-by-property and index-based optimization
2. **Range Scan**: `scan_nodes_by_range(label_id, prop_key, min, max)` -- needed for index-assisted WHERE predicates
3. **Node lookup by label + properties**: `find_node(labels, properties) -> Option<NodeId>` -- critical for MERGE semantics
4. **Edge lookup by type + endpoints**: `find_edge(start, end, type_id) -> Option<EdgeId>` -- critical for MERGE edge patterns

### Catalog Extensions
- Current: `Catalog` with `get_or_create_label(name) -> u32` and `get_or_create_type(name) -> u32`
- Needed: `get_label_id(name) -> Option<u32>` (read-only lookup for MATCH without auto-creation)
- Needed: `get_type_id(name) -> Option<u32>` (read-only lookup)
- File: `cypherlite-storage/src/catalog/mod.rs`

## 4. Clause Implementation Analysis

### 4.1 WITH Clause
**Semantics**: Pipeline barrier -- projects columns and resets variable scope.
- Acts like RETURN but feeds into next clause instead of producing output
- WITH a, b WHERE a.age > 25 -- filter after scope reset
- WITH DISTINCT -- deduplication before next clause
- WITH + aggregation: `WITH a, count(*) AS cnt`

**Implementation Strategy**:
- Planner: New `LogicalPlan::With { source, items, distinct }` node OR reuse `Project` + scope barrier
- Executor: Reuse `execute_project` logic with scope variable filtering
- Semantic: Scope reset -- variables after WITH are ONLY those explicitly listed
- Key complexity: Variable scope boundary enforcement in semantic analyzer

### 4.2 MERGE Clause
**Semantics**: MATCH-or-CREATE atomic operation.
- MERGE (n:Person {name: 'Alice'}) -- find or create
- MERGE (a)-[r:KNOWS]->(b) -- find or create relationship
- ON MATCH SET n.updated = timestamp()
- ON CREATE SET n.created = timestamp()

**Implementation Strategy**:
- Phase 3a (basic): MERGE pattern without ON MATCH/ON CREATE
- Phase 3b (full): Add ON MATCH SET and ON CREATE SET actions
- Planner: New `LogicalPlan::MergeOp { source, pattern, on_match, on_create }`
- Executor: 1) Try to match pattern 2) If not found, create 3) Apply ON MATCH/ON CREATE
- Storage: Need `find_node(labels, props)` and `find_edge(start, end, type)` APIs
- AST change: Add `on_match: Vec<SetItem>` and `on_create: Vec<SetItem>` to `MergeClause`

### 4.3 OPTIONAL MATCH
**Semantics**: Left outer join -- if no match found, bind variables to NULL.
- MATCH (a:Person) OPTIONAL MATCH (a)-[:KNOWS]->(b) RETURN a, b
- If no b exists, b is NULL in the result

**Implementation Strategy**:
- Planner: New `LogicalPlan::OptionalExpand` or flag on existing Expand
- Executor: Execute inner plan; if empty result for a source record, emit record with NULL bindings
- Key complexity: Preserving source records when inner pattern yields no matches

### 4.4 UNWIND Clause
**Semantics**: Expand a list expression into individual rows.
- UNWIND [1, 2, 3] AS x RETURN x -- produces 3 rows
- UNWIND a.friends AS f -- expand list property

**Implementation Strategy**:
- Lexer: Add `Unwind` token (keyword already reserved? check lexer)
- AST: Add `Clause::Unwind(UnwindClause)` with `{ expr: Expression, variable: String }`
- Parser: `parse_unwind_clause` -- `UNWIND expr AS variable`
- Semantic: Register variable in scope
- Planner: New `LogicalPlan::Unwind { source, expr, variable }`
- Executor: For each source record, evaluate expr to a list, emit one row per element

### 4.5 Variable-Length Paths
**Semantics**: Match paths of variable length.
- (a)-[*1..3]->(b) -- paths of length 1 to 3
- (a)-[*]->(b) -- any length (unbounded, needs max cap)
- (a)-[:KNOWS*2]->(b) -- exactly 2 hops of type KNOWS

**Implementation Strategy**:
- Lexer: Already handles `*` token; need to parse `*min..max` in relationship pattern
- AST: Add to `RelationshipPattern`: `min_hops: Option<u32>`, `max_hops: Option<u32>`
- Parser: Extend `parse_relationship_pattern` to handle `[*]`, `[*N]`, `[*N..M]`
- Planner: New `LogicalPlan::VarLengthExpand { source, ..., min_hops, max_hops }`
- Executor: BFS/DFS traversal with depth tracking and cycle detection
- Default max: 10 hops (configurable) to prevent infinite traversal
- Key complexity: Cycle detection, memory management for large graphs

## 5. Indexing System Analysis

### Current State
- No secondary indexes exist -- all queries use full label scan + filter
- B+Tree exists in storage layer but only for node/edge ID lookup
- Label scan (`scan_nodes_by_label`) returns all nodes with label, then filter applied

### Proposed Index Architecture
1. **Property Index (B+Tree)**: `(label_id, property_key_id, property_value) -> Vec<NodeId>`
   - Speeds up: `MATCH (n:Person {name: 'Alice'})`, `WHERE n.age > 25`
   - Storage: Separate B+Tree per (label, property) pair
   - Create: `CREATE INDEX ON :Person(name)`

2. **Composite Index**: `(label_id, prop1, prop2) -> Vec<NodeId>`
   - Speeds up: Multi-property lookups
   - Phase 3+ scope

3. **Full-Text Index** (future): Text search on string properties
   - Out of scope for Phase 3

### Index Management
- DDL: `CREATE INDEX [name] ON :Label(property)`, `DROP INDEX name`
- Catalog: Store index definitions in Catalog
- Auto-update: Indexes must be updated on CREATE, SET, DELETE
- Query planner: Choose index scan vs full scan based on selectivity

## 6. Query Optimization Rules

### Already Implemented (Phase 2)
- Predicate pushdown: WHERE filters pushed below PROJECT
- Label-based scan: NodeScan uses label_id when available

### Proposed Phase 3 Optimizations
1. **Index Scan Selection**: When property index exists, replace NodeScan+Filter with IndexScan
2. **MERGE Short-Circuit**: Use index lookup for MERGE match-check before full scan
3. **LIMIT Pushdown**: Push LIMIT into NodeScan/Expand to avoid materializing all results
4. **Predicate Decomposition**: Split AND predicates and push each filter independently
5. **Constant Folding**: Evaluate constant expressions at plan time (e.g., `1 + 2` -> `3`)
6. **Projection Pruning**: Remove unused columns early in the pipeline
7. **Join Order Optimization**: Reorder MATCH patterns for smallest intermediate result

## 7. Risk Assessment

### High Risk
- **MERGE atomicity**: Must be atomic (match + create) under concurrent access. Current StorageEngine uses single-writer model, which helps, but need to ensure the match-then-create sequence is not interrupted.
- **Variable-length path memory**: Unbounded paths can explode memory. Must enforce max_hops limit and track visited nodes for cycle detection.

### Medium Risk
- **WITH scope semantics**: Variable scoping after WITH is strict -- only projected variables survive. This requires careful semantic analysis changes.
- **OPTIONAL MATCH left join**: Generating NULL-padded records when no match found needs careful handling in the Volcano model where operators pull records.
- **Index consistency**: Indexes must be updated atomically with data mutations. Leveraging WAL for crash consistency.

### Low Risk
- **UNWIND**: Straightforward list expansion; well-understood semantics.
- **ORDER BY/SKIP/LIMIT in WITH**: Already implemented for RETURN; reuse pattern.
- **Constant folding**: Pure transformation, no storage interaction.

## 8. Recommended Implementation Order

### Phase 3a: Core Clauses (Foundation)
1. WITH clause (scope barrier, enables multi-clause queries)
2. UNWIND clause (simple, adds list processing capability)
3. OPTIONAL MATCH (left join semantics)

### Phase 3b: MERGE + Indexing
4. Storage: `find_node`, `find_edge` APIs
5. MERGE clause (basic, without ON MATCH/ON CREATE)
6. Property Index infrastructure (B+Tree index, CREATE INDEX DDL)
7. MERGE with ON MATCH/ON CREATE

### Phase 3c: Advanced Patterns + Optimization
8. Variable-length path parsing and execution
9. Index-based query optimization rules
10. LIMIT pushdown, constant folding, projection pruning

## 9. File Impact Summary

### New Files
- `crates/cypherlite-query/src/executor/operators/with.rs`
- `crates/cypherlite-query/src/executor/operators/unwind.rs`
- `crates/cypherlite-query/src/executor/operators/merge.rs`
- `crates/cypherlite-query/src/executor/operators/optional_expand.rs`
- `crates/cypherlite-query/src/executor/operators/var_length_expand.rs`
- `crates/cypherlite-storage/src/index/` (new module for property indexes)

### Modified Files (High Impact)
- `crates/cypherlite-query/src/parser/ast.rs` -- UnwindClause, MergeClause extensions, RelationshipPattern.hops
- `crates/cypherlite-query/src/parser/clause.rs` -- parse_unwind, extend parse_merge
- `crates/cypherlite-query/src/parser/pattern.rs` -- variable-length path syntax
- `crates/cypherlite-query/src/lexer/mod.rs` -- UNWIND keyword token
- `crates/cypherlite-query/src/semantic/mod.rs` -- WITH scope reset, UNWIND variable binding, OPTIONAL MATCH
- `crates/cypherlite-query/src/planner/mod.rs` -- LogicalPlan variants: With, MergeOp, Unwind, VarLengthExpand, OptionalExpand
- `crates/cypherlite-query/src/executor/mod.rs` -- dispatch new plan nodes
- `crates/cypherlite-storage/src/lib.rs` -- find_node, find_edge, index APIs

### Modified Files (Low Impact)
- `crates/cypherlite-query/src/api/mod.rs` -- expose new capabilities
- `Cargo.toml` -- version bump to 0.3.0
- `crates/cypherlite-core/src/lib.rs` -- possible new error variants

## 10. Test Strategy

### Unit Tests (per module)
- Parser: Round-trip tests for UNWIND, extended MERGE (ON MATCH/CREATE), var-length path syntax
- Semantic: WITH scope isolation tests, UNWIND variable binding, OPTIONAL MATCH null propagation
- Planner: Plan tree structure tests for each new LogicalPlan variant
- Executor: Operator correctness tests with mock storage

### Integration Tests (cross-module)
- `MATCH ... WITH ... RETURN` pipeline queries
- `MERGE` create-if-not-exists scenarios
- `OPTIONAL MATCH` with NULL result propagation
- `UNWIND` with list properties and literal lists
- Variable-length path traversal with cycle detection

### Property-Based Tests (proptest)
- Random graph generation + OPTIONAL MATCH correctness
- UNWIND list length invariants
- Variable-length path depth limits

### Performance Tests (criterion)
- Index vs full scan comparison
- Variable-length path scaling with graph size
- MERGE vs MATCH+CREATE comparison
