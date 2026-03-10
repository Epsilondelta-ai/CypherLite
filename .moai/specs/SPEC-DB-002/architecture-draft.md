# SPEC-DB-002 Architecture Design
# CypherLite Phase 2: Query Engine

**Author**: architect (team-reader)
**Date**: 2026-03-10
**Status**: Draft v2 (revised with researcher + analyst findings)
**Phase 1 Reference**: SPEC-DB-001 (storage engine complete, 207 tests, 96.82% coverage)

---

## Revision Notes (v2)

The following critical constraints from the researcher and analyst are incorporated in this revision:

- **String Catalog Gap**: Phase 1 has no `String <-> u32` mapping for labels, property keys, or relationship types. A `SymbolTable` / `Catalog` is a required new deliverable.
- **Missing Scan API**: `StorageEngine` has no iterator over nodes or edges. New scan methods must be added to `cypherlite-storage`.
- **`StorageEngine` is not `Send`**: The executor must be strictly single-threaded; no cross-thread movement of `StorageEngine`.
- **No `PartialOrd` on `PropertyValue`**: The query engine must implement its own comparison logic for ordered operators.
- **`get_edges_for_node()` is O(E)**: Document as known limitation; index optimization deferred to Phase 3.
- **WASM compatibility**: No `std::thread` in the query core.
- **Clause priority**: P0 = MATCH+RETURN, CREATE. P1 = WHERE, SET/REMOVE, DELETE. P2 = MERGE, WITH, ORDER BY/LIMIT/SKIP.
- **Performance targets**: parse p99 < 1ms, plan p99 < 2ms, simple MATCH p99 < 10ms, 2-hop p99 < 50ms.

---

## 1. Crate Structure Decision

### Decision: Single new crate `cypherlite-query` + Catalog extension to `cypherlite-storage`

**Recommendation**: Add one new crate `cypherlite-query`. The `SymbolTable`/`Catalog` is implemented as a new module inside `cypherlite-storage` (not as a separate crate, and not in `cypherlite-core`).

### Crate Responsibilities

| Crate | Phase 2 additions |
|-------|-------------------|
| `cypherlite-core` | Add `LabelRegistry` trait; add query error variants to `CypherLiteError` |
| `cypherlite-storage` | Add `Catalog` module; add `scan_nodes()`, `scan_nodes_by_label()`, `scan_edges_by_type()` to `StorageEngine` |
| `cypherlite-query` | New crate: lexer, parser, AST, semantic analysis, logical planner, executor, public API |

### SymbolTable / Catalog Placement Decision

**Decision**: `Catalog` lives in `cypherlite-storage`.

**Rationale**:

1. **Persistence requirement**: The catalog maps `String <-> u32` for labels, property keys, and relationship types and must survive process restarts. It must be persisted to disk (reserved B-tree pages in the `.cyl` file). Only `cypherlite-storage` has disk access.

2. **Tight coupling with `StorageEngine`**: The catalog must be opened when the database is opened, and catalog writes must participate in the WAL transaction that also writes node/edge records. This coupling is necessary for consistency and cannot be achieved if the catalog lives in `cypherlite-query`.

3. **Why not `cypherlite-core`**: `cypherlite-core` has no external dependencies and no disk access. Adding persistence logic or a dashmap would violate its role as a pure type/trait layer.

4. **Why not a new crate**: Splitting to `cypherlite-catalog` adds workspace overhead without benefit. The catalog is a well-bounded module within storage.

5. **`LabelRegistry` trait in `cypherlite-core`**: The semantic analyzer in `cypherlite-query` needs to resolve string labels to u32 IDs. To avoid making `cypherlite-query` depend on storage internals for this one operation, `cypherlite-core` defines a `LabelRegistry` trait. `StorageEngine` (via `Catalog`) implements it. This keeps semantic analysis unit-testable with a mock.

### Workspace Update

```toml
# Cargo.toml (workspace root)
[workspace]
members = [
    "crates/cypherlite-core",
    "crates/cypherlite-storage",
    "crates/cypherlite-query",   # new
]
```

### Dependency Graph

```
cypherlite-query
  ├── cypherlite-core   (LabelRegistry trait, PropertyValue, error types)
  └── cypherlite-storage (StorageEngine, Catalog, scan APIs)

cypherlite-storage
  └── cypherlite-core   (NodeId, EdgeId, PropertyValue, CypherLiteError)

cypherlite-core
  (no dependencies)
```

### What External Users Import

Users interact with `CypherLite` (from `cypherlite-query`), which wraps `StorageEngine` and exposes `execute(cypher) -> Result<QueryResult>`. Consumers that need only raw Rust storage without Cypher can depend solely on `cypherlite-storage`.

---

## 2. Catalog Design (String <-> u32 Mapping)

This is a new required deliverable identified by the researcher. It resolves the critical gap where Phase 1 labels and property keys are bare `u32` values with no string backing.

### Catalog Structure

The `Catalog` manages three namespaces:

| Namespace | Maps | Used by |
|-----------|------|---------|
| Labels | `String <-> u32` | Node labels (e.g. `"Person"` → `1`) |
| Property keys | `String <-> u32` | Node and edge property keys (e.g. `"name"` → `1`) |
| Relationship types | `String <-> u32` | Edge types (e.g. `"KNOWS"` → `1`) |

IDs within each namespace are independent (a label and a property key can both have ID `1`).

### Catalog API (pseudocode)

```rust
pub struct Catalog {
    labels:     BiMap<String, u32>,  // two-way lookup
    prop_keys:  BiMap<String, u32>,
    rel_types:  BiMap<String, u32>,
    // next available ID per namespace
    next_label_id:    u32,
    next_prop_key_id: u32,
    next_rel_type_id: u32,
}

impl Catalog {
    // Get or create a label ID (idempotent)
    pub fn label_id(&mut self, name: &str) -> u32;
    pub fn label_name(&self, id: u32) -> Option<&str>;

    pub fn prop_key_id(&mut self, name: &str) -> u32;
    pub fn prop_key_name(&self, id: u32) -> Option<&str>;

    pub fn rel_type_id(&mut self, name: &str) -> u32;
    pub fn rel_type_name(&self, id: u32) -> Option<&str>;

    // Serialize to / deserialize from a catalog page in the .cyl file
    pub fn save(&self, page_manager: &mut PageManager) -> Result<()>;
    pub fn load(page_manager: &PageManager) -> Result<Self>;
}
```

### Persistence

- One dedicated catalog page (page 2 in the `.cyl` file, after header page 0 and free-space map page 1).
- Serialized with `bincode` (consistent with Phase 1).
- On `StorageEngine::open()`, catalog is loaded from page 2 (or initialized empty if new database).
- Catalog mutations are written via WAL (same as any other page write) to maintain ACID.
- `StorageEngine` holds `catalog: Catalog` as a field.

### `LabelRegistry` Trait in `cypherlite-core`

```rust
// cypherlite-core/src/traits.rs (addition)
pub trait LabelRegistry {
    fn label_id(&mut self, name: &str) -> u32;
    fn label_name(&self, id: u32) -> Option<&str>;
    fn prop_key_id(&mut self, name: &str) -> u32;
    fn prop_key_name(&self, id: u32) -> Option<&str>;
    fn rel_type_id(&mut self, name: &str) -> u32;
    fn rel_type_name(&self, id: u32) -> Option<&str>;
}
```

`StorageEngine` (via `Catalog`) implements `LabelRegistry`. The semantic analyzer holds `&mut dyn LabelRegistry`, allowing tests to inject a `MockCatalog`.

---

## 3. Storage Engine Extensions Required

These additions to `cypherlite-storage` are prerequisites for the query executor. They are part of SPEC-DB-002 scope.

### Scan APIs

```rust
// Methods to add to StorageEngine
impl StorageEngine {
    // Full node scan (no filter)
    pub fn scan_nodes(&self) -> impl Iterator<Item = &NodeRecord>;

    // Scan nodes with a specific label (linear scan filtered by label — no index in Phase 2)
    pub fn scan_nodes_by_label(&self, label_id: u32) -> Vec<NodeRecord>;

    // Scan edges by relationship type (linear scan — no index in Phase 2)
    pub fn scan_edges_by_type(&self, type_id: u32) -> Vec<RelationshipRecord>;
}
```

**Note on `get_edges_for_node()` O(E) complexity**: The existing `get_edges_for_node()` performs a full linear scan of all edges to find those belonging to a node. This is a known Phase 1 limitation. The query planner must document that `Expand` operations in Phase 2 are O(E) per anchor node. Index-free adjacency traversal via the linked `next_edge_id` chain on `NodeRecord` is the correct O(degree) path; the Phase 2 executor will use `next_edge_id` chain traversal rather than `get_edges_for_node()` to avoid the O(E) scan.

### `StorageEngine` is Not `Send`

The researcher confirmed `StorageEngine` is not `Send` (contains `parking_lot::MutexGuard` with `'static` lifetime extension via `unsafe transmute` — noted in `@MX:WARN` at `transaction/mvcc.rs:48`). Consequences for the query engine:

- The executor holds `&mut StorageEngine` for the duration of a query.
- No executor thread pool or work-stealing scheduler that moves the engine across threads.
- `CypherLite` struct is also not `Send`. Document this in the public API.
- WASM compatibility is satisfied because the single-threaded constraint is already enforced.
- If multi-threaded query execution is needed in Phase 3+, the `unsafe transmute` in mvcc.rs must be resolved first.

### `CypherLiteError` Extension

`cypherlite-core/src/error.rs` must add:

```rust
pub enum CypherLiteError {
    // ... existing variants ...

    // New for Phase 2
    #[error("Parse error at {span:?}: {message}")]
    ParseError { message: String, span: (usize, usize) },

    #[error("Semantic error: {0}")]
    SemanticError(String),

    #[error("Query execution error: {0}")]
    ExecutionError(String),

    #[error("Unsupported syntax: {0}")]
    UnsupportedSyntax(String),
}
```

Using concrete variants (rather than `Box<dyn Error>`) keeps the error enum exhaustively matchable, which is important for library users.

---

## 4. Parsing Strategy Decision

### Decision: `logos` 0.14 for lexing + hand-written recursive descent parser

**Recommendation**: Use `logos` 0.14 for the lexer and a hand-written recursive descent parser for AST construction. This matches `tech.md` exactly.

logos generates a DFA-based lexer from a derive macro at compile time. Zero runtime overhead. MSRV 1.65 — compatible with Rust 1.84.

**Alternatives rejected**:
- **nom 7.x**: Composable but produces poor error messages for keyword-heavy grammars. Error recovery requires significant wrapper code. Rejected.
- **pest 2.x**: PEG backtracking is O(n²) worst case. Error recovery is separate from grammar logic. Rejected.
- **lalrpop 0.22**: Introduces build-time compilation step. Shift-reduce conflicts are non-obvious to debug. Error recovery is harder to customize than recursive descent. Rejected.
- **Fully hand-written lexer**: Writing a correct, performant DFA lexer for Cypher's keyword density saves significant effort with logos at zero runtime cost. Rejected in favor of logos.

**Parse p99 < 1ms target**: A logos DFA lexer + recursive descent parser for typical Cypher queries (<200 tokens) will be well under 1ms on modern hardware. No performance risk.

---

## 5. AST Type Design

### Design Principles

- `Box<T>` for all recursive types.
- `String` (owned) for identifiers. `Arc<str>` optimization deferred to Phase 3 if query plan interning becomes a bottleneck.
- `Span { start: usize, end: usize }` on every AST node (byte offsets into original query string).
- `#[derive(Debug, Clone, PartialEq)]` on all AST nodes.

### Phase 2 Scope

Only P0 and P1 clauses need full AST support. P2 clauses (MERGE, WITH, ORDER BY) may be parsed into stub `UnsupportedClause` nodes that immediately emit `CypherLiteError::UnsupportedSyntax`.

### Core Type Hierarchy (type sketches)

```rust
pub struct Span { pub start: usize, pub end: usize }

pub enum Query {
    Single(SingleQuery),
    Union { left: Box<Query>, right: Box<Query>, all: bool },  // parsed, executor deferred P2
}

pub struct SingleQuery {
    pub match_clause:    Option<MatchClause>,
    pub updating_clause: Option<UpdatingClause>,  // CREATE | SET | DELETE
    pub return_clause:   Option<ReturnClause>,
    pub span: Span,
}

// P0: MATCH
pub struct MatchClause {
    pub optional: bool,
    pub pattern:  Pattern,
    pub where_clause: Option<Box<Expression>>,  // P1
    pub span: Span,
}

// P0/P1 updating clauses
pub enum UpdatingClause {
    Create(CreateClause),   // P0
    Set(SetClause),         // P1
    Delete(DeleteClause),   // P1
    Remove(RemoveClause),   // P1
}

pub struct CreateClause { pub pattern: Pattern, pub span: Span }

pub struct SetClause { pub items: Vec<SetItem>, pub span: Span }
pub enum SetItem {
    PropertySet { target: Box<Expression>, value: Box<Expression> },
    LabelAdd    { variable: String, labels: Vec<String> },
}

pub struct RemoveClause { pub items: Vec<RemoveItem>, pub span: Span }
pub enum RemoveItem {
    PropertyRemove { target: Box<Expression> },
    LabelRemove    { variable: String, labels: Vec<String> },
}

pub struct DeleteClause { pub detach: bool, pub exprs: Vec<Expression>, pub span: Span }

// Pattern
pub struct Pattern { pub parts: Vec<PatternPart>, pub span: Span }
pub struct PatternPart {
    pub variable: Option<String>,
    pub chain: PatternChain,
}
pub struct PatternChain {
    pub start: NodePattern,
    pub segments: Vec<(RelationshipPattern, NodePattern)>,
}

pub struct NodePattern {
    pub variable: Option<String>,
    pub labels:   Vec<String>,
    pub properties: Vec<(String, Box<Expression>)>,
    pub span: Span,
}

pub struct RelationshipPattern {
    pub variable:  Option<String>,
    pub types:     Vec<String>,
    pub direction: RelDirection,
    pub properties: Vec<(String, Box<Expression>)>,
    pub span: Span,
    // variable-length: deferred to Phase 3; parser emits UnsupportedSyntax
}

pub enum RelDirection { Outgoing, Incoming, Undirected }

// Expressions
pub enum Expression {
    Literal(Literal, Span),
    Variable(String, Span),
    Property { base: Box<Expression>, key: String, span: Span },
    BinaryOp { op: BinaryOp, left: Box<Expression>, right: Box<Expression>, span: Span },
    UnaryOp  { op: UnaryOp, operand: Box<Expression>, span: Span },
    FunctionCall { name: String, args: Vec<Expression>, distinct: bool, span: Span },
    List(Vec<Expression>, Span),
    IsNull    { expr: Box<Expression>, span: Span },
    IsNotNull { expr: Box<Expression>, span: Span },
    In        { expr: Box<Expression>, list: Box<Expression>, span: Span },
    Parameter(String, Span),  // $param_name
}

pub enum Literal { Integer(i64), Float(f64), Str(String), Bool(bool), Null }

pub enum BinaryOp {
    Add, Sub, Mul, Div, Mod, Pow,
    Eq, Ne, Lt, Lte, Gt, Gte,
    And, Or, Xor,
    Contains, StartsWith, EndsWith,
}
pub enum UnaryOp { Not, Neg }

// P0: RETURN
pub struct ReturnClause {
    pub distinct: bool,
    pub items: ReturnItems,
    pub order_by: Option<OrderBy>,   // P2 executor, P1 parse
    pub skip:  Option<Box<Expression>>,  // P2
    pub limit: Option<Box<Expression>>,  // P2
    pub span: Span,
}
pub enum ReturnItems { Wildcard, Items(Vec<ReturnItem>) }
pub struct ReturnItem { pub expression: Expression, pub alias: Option<String>, pub span: Span }
pub struct OrderBy   { pub items: Vec<SortItem> }
pub struct SortItem  { pub expression: Expression, pub descending: bool }
```

---

## 6. Query Planner Design

### Logical Plan Operators

```
LogicalPlan:
  NodeScan    { label_filter: Option<u32>, alias: String }
  Expand      { input: Box<LogicalPlan>, from_alias: String, to_alias: String,
                rel_alias: Option<String>, rel_types: Vec<u32>, direction: RelDirection }
  Filter      { input: Box<LogicalPlan>, predicate: Expression }
  Project     { input: Box<LogicalPlan>, items: Vec<(Expression, String)> }
  Limit       { input: Box<LogicalPlan>, count: u64 }
  Skip        { input: Box<LogicalPlan>, count: u64 }
  Sort        { input: Box<LogicalPlan>, keys: Vec<SortKey> }
  Aggregate   { input: Box<LogicalPlan>, group_keys: Vec<Expression>,
                agg_exprs: Vec<(AggFunction, String)> }
  Create      { input: Option<Box<LogicalPlan>>, pattern: Pattern }
  Delete      { input: Box<LogicalPlan>, exprs: Vec<Expression>, detach: bool }
  SetProps    { input: Box<LogicalPlan>, items: Vec<SetItem> }
```

Note: All label/type references in `LogicalPlan` use resolved `u32` IDs (post-semantic-analysis), not strings.

### Planner Phases

**Phase 1 — Semantic Analysis**
- Walk the AST; track variable bindings in a `SymbolTable` (query-local scope, not the storage `Catalog`).
- Resolve string labels/types to u32 IDs via `&mut dyn LabelRegistry` (backed by `Catalog`).
- Type-check expressions where statically determinable.
- Output: symbol-annotated AST, or `SemanticError`.

**Phase 2 — Logical Plan Construction**
- `MATCH (n:Label)` → `NodeScan { label_filter: Some(label_id), alias: "n" }`
- `MATCH (n)-[r:TYPE]->(m)` → `NodeScan` → `Expand { rel_types: [type_id], direction: Outgoing }`
- `WHERE expr` → `Filter` wrapping the scan/expand chain
- `RETURN items` → `Project`
- `CREATE pattern` → `Create` at leaf
- `SET / DELETE` → `SetProps` / `Delete` wrapping an input plan

**Phase 3 — Rule-Based Optimization (no cost model in Phase 2)**
- Label filter pushdown: merge `WHERE n:Label` predicate into the `NodeScan.label_filter`.
- Predicate pushdown: move `Filter` nodes toward leaves.
- Limit pushdown: annotate early-termination on `NodeScan` when `LIMIT k` is present.

A cost-based optimizer is deferred to Phase 3 when indexes are added. Plan p99 < 2ms is achievable for typical query sizes without cost estimation.

### Known O(E) Limitation

`get_edges_for_node()` in Phase 1 is O(E) (full scan). The `Expand` operator will instead traverse the `next_edge_id` linked list directly on `NodeRecord`, which is O(degree). This must be documented in the `ExpandOp` source and in the SPEC.

---

## 7. Executor Design

### Decision: Volcano Iterator Model

Single-threaded pull-based (Volcano) model. Satisfies:
- No `std::thread` requirement (WASM compatibility).
- `StorageEngine` is not `Send` — no movement across threads.
- Memory efficient for streaming.
- Simple composition of operators.

### Typed Value Comparison

`PropertyValue` has no `PartialOrd`. The expression evaluator must implement typed comparison:

```rust
// eval_cmp(op, left, right) -> Result<bool>
// Rules:
//   Integer op Integer    -> numeric comparison
//   Float op Float        -> floating-point comparison
//   Integer op Float      -> promote Integer to Float, then compare
//   String op String      -> lexicographic comparison (only Eq/Ne/Lt/Lte/Gt/Gte)
//   mismatched types      -> CypherLiteError::ExecutionError("type mismatch in comparison")
//   Null op anything      -> always false (Cypher null semantics)
```

This logic lives in `executor/eval.rs` and is the single authoritative comparison implementation.

### Core Trait (pseudocode)

```rust
pub struct Record {
    pub columns: Vec<String>,
    pub values:  Vec<Value>,
}

pub enum Value {
    Null,
    Bool(bool),
    Integer(i64),
    Float(f64),
    Str(String),
    Node(NodeId),
    Edge(EdgeId),
    List(Vec<Value>),
}

pub trait PhysicalOperator {
    fn open(&mut self, engine: &mut StorageEngine) -> Result<()>;
    fn next(&mut self) -> Result<Option<Record>>;
    fn close(&mut self);
}
```

### Operator Implementations

| Operator | Notes |
|----------|-------|
| `NodeScanOp` | Calls `scan_nodes()` or `scan_nodes_by_label(label_id)` |
| `ExpandOp` | Traverses `NodeRecord.next_edge_id` linked list — O(degree), not O(E) |
| `FilterOp` | Wraps inner op; calls `eval(predicate, record)` |
| `ProjectOp` | Evaluates RETURN expressions, renames columns |
| `LimitOp` | Stops after N rows |
| `SkipOp` | Discards first N rows |
| `SortOp` | Materializes full input (in-memory), then sorts by key |
| `AggregateOp` | Materializes full input, groups, computes COUNT/SUM/AVG/MIN/MAX |
| `CreateOp` | Calls `StorageEngine::create_node()` / `create_edge()` |
| `DeleteOp` | Calls `StorageEngine::delete_node()` / `delete_edge()` |
| `SetPropsOp` | Calls `StorageEngine::update_node()` |

`SortOp` and `AggregateOp` materialize full input into `Vec<Record>`. Document per-query memory limit in `DatabaseConfig` (`max_sort_rows: usize`, default 100_000). Exceeding the limit returns `ExecutionError("sort/aggregate result set too large")`.

### Expression Evaluator

A standalone `eval(expr: &Expression, record: &Record, params: &Params) -> Result<Value>` function in `executor/eval.rs`. Recursive on `Expression` variants. Called by `FilterOp` and `ProjectOp`. Handles:
- Literal → Value conversion
- Variable lookup in `record`
- Property access: look up `NodeRecord.properties` by resolved property key ID
- `BinaryOp`: arithmetic + comparison (typed, using `eval_cmp` above) + logical
- `FunctionCall`: `count()`, `sum()`, `avg()`, `min()`, `max()`, `id()`, `labels()`, `type()`
- `Parameter`: lookup in `params` map

---

## 8. Public API Design

```rust
// Entry point
pub struct CypherLite {
    engine: StorageEngine,
    // NOTE: CypherLite is NOT Send (StorageEngine is not Send)
}

impl CypherLite {
    pub fn open(config: DatabaseConfig) -> Result<Self>;
    pub fn execute(&mut self, cypher: &str) -> Result<QueryResult>;
    pub fn execute_with_params(&mut self, cypher: &str, params: &Params) -> Result<QueryResult>;
    pub fn begin(&mut self) -> Result<Transaction<'_>>;
}

// Named parameter map — prevents Cypher injection
pub struct Params(pub HashMap<String, Value>);

// Materialized result set (Phase 2: full collection)
pub struct QueryResult {
    pub columns: Vec<String>,
    rows: Vec<Record>,
    cursor: usize,
}
impl QueryResult {
    pub fn column_names(&self) -> &[String];
    pub fn is_empty(&self) -> bool;
    pub fn len(&self) -> usize;
    pub fn rows(&self) -> &[Row];
}
impl Iterator for QueryResult { type Item = Row; fn next(&mut self) -> Option<Row>; }

pub struct Row(Record);
impl Row {
    pub fn get(&self, column: &str) -> Option<&Value>;
    pub fn get_typed<T: FromValue>(&self, column: &str) -> Result<T>;
}

// Explicit transaction
pub struct Transaction<'db> { engine: &'db mut StorageEngine, committed: bool }
impl Transaction<'_> {
    pub fn execute(&mut self, cypher: &str) -> Result<QueryResult>;
    pub fn commit(self) -> Result<()>;
    pub fn rollback(self) -> Result<()>;
}
```

### API Notes

- `CypherLite` is not `Send`. Document this in the public crate docs.
- `execute()` auto-commits (single-statement default). Explicit `Transaction` for multi-statement batches.
- `Params` substitution uses `$name` syntax in Cypher: `MATCH (n) WHERE n.name = $name RETURN n`.
- `QueryResult` materializes all rows. Streaming `QueryResult` deferred to Phase 3.

---

## 9. New Dependencies

### Required

| Crate | Version | MSRV | Purpose | Binary impact |
|-------|---------|------|---------|---------------|
| `logos` | 0.14 | 1.65 | DFA lexer generation | Zero runtime; compile-time code gen |

### Deferred

| Crate | Reason for deferral |
|-------|---------------------|
| `smallvec` | Premature optimization; profile first in Phase 3 |
| `indexmap` | HashMap sufficient for Phase 2 |

### `cypherlite-query/Cargo.toml`

```toml
[package]
name = "cypherlite-query"
version = "0.1.0"
edition = "2021"
rust-version = "1.84"

[dependencies]
cypherlite-core    = { path = "../cypherlite-core" }
cypherlite-storage = { path = "../cypherlite-storage" }
logos              = "0.14"

[dev-dependencies]
tempfile = "3"
proptest = "1"
```

Total new runtime crates: **1** (logos).

---

## 10. Implementation Task List

Tasks ordered by dependency. Each maps to one RED-GREEN-REFACTOR cycle.

### Group A: Catalog (cypherlite-storage addition — prerequisite for all query work)

| Task | Description | Crate |
|------|-------------|-------|
| TASK-001 | Add `LabelRegistry` trait to `cypherlite-core/src/traits.rs` | core |
| TASK-002 | Add `ParseError`, `SemanticError`, `ExecutionError`, `UnsupportedSyntax` variants to `CypherLiteError` | core |
| TASK-003 | Implement `Catalog` struct with BiMaps for labels, prop keys, rel types | storage |
| TASK-004 | Implement `Catalog::save()` / `load()` via a catalog page in PageManager | storage |
| TASK-005 | Integrate `Catalog` into `StorageEngine` (open, field, `impl LabelRegistry`) | storage |

### Group B: Scan APIs (cypherlite-storage addition — prerequisite for executor)

| Task | Description | Crate |
|------|-------------|-------|
| TASK-006 | Add `scan_nodes() -> impl Iterator<Item = &NodeRecord>` to `StorageEngine` | storage |
| TASK-007 | Add `scan_nodes_by_label(label_id: u32) -> Vec<NodeRecord>` to `StorageEngine` | storage |
| TASK-008 | Add `scan_edges_by_type(type_id: u32) -> Vec<RelationshipRecord>` to `StorageEngine` | storage |
| TASK-009 | Write tests for all three scan methods | storage |

### Group C: Workspace Scaffold (cypherlite-query new crate)

| Task | Description | Crate |
|------|-------------|-------|
| TASK-010 | Add `cypherlite-query` to workspace `Cargo.toml`; create `Cargo.toml` with logos dep | query |
| TASK-011 | Create `src/lib.rs` with module stubs: `lexer`, `parser`, `semantic`, `planner`, `executor`, `api` | query |

### Group D: Lexer

| Task | Description | Crate |
|------|-------------|-------|
| TASK-012 | Define `Token` enum with `#[derive(Logos)]`; all Cypher keywords for P0/P1/P2 | query |
| TASK-013 | Add identifier, integer, float, string literal token rules | query |
| TASK-014 | Add operator and punctuation tokens | query |
| TASK-015 | Define `Span` struct; integrate into lexer output | query |
| TASK-016 | Unit tests: keyword disambiguation, string escapes, error token handling | query |

### Group E: Parser — Expressions

| Task | Description | Crate |
|------|-------------|-------|
| TASK-017 | Define `Expression`, `Literal`, `BinaryOp`, `UnaryOp` AST types in `parser/ast.rs` | query |
| TASK-018 | Implement Pratt parser for expressions: precedence climbing for arithmetic and comparison | query |
| TASK-019 | Implement `parse_literal()`, `parse_parameter()` ($name substitution) | query |
| TASK-020 | Implement `parse_function_call()`: `name(args)` and `name(DISTINCT args)` | query |
| TASK-021 | Implement `parse_property_access()`: `n.name` chained access | query |
| TASK-022 | Unit tests: precedence, comparisons, NOT/AND/OR, function calls, $params | query |

### Group F: Parser — Patterns

| Task | Description | Crate |
|------|-------------|-------|
| TASK-023 | Define `NodePattern`, `RelationshipPattern`, `Pattern`, `PatternChain` AST types | query |
| TASK-024 | Implement `parse_node_pattern()`: `(n:Label {prop: val})` | query |
| TASK-025 | Implement `parse_relationship_pattern()`: all three direction variants | query |
| TASK-026 | Implement `parse_pattern()`: full chain; emit `UnsupportedSyntax` for `*` range | query |
| TASK-027 | Unit tests: single node, directed, undirected, unlabeled, multi-hop | query |

### Group G: Parser — Clauses

| Task | Description | Crate |
|------|-------------|-------|
| TASK-028 | Implement `parse_match_clause()`: MATCH / OPTIONAL MATCH + WHERE | query |
| TASK-029 | Implement `parse_return_clause()`: RETURN [DISTINCT] items | query |
| TASK-030 | Implement ORDER BY, SKIP, LIMIT parsing (P2 execution, but parse now) | query |
| TASK-031 | Implement `parse_create_clause()` | query |
| TASK-032 | Implement `parse_set_clause()` and `parse_remove_clause()` | query |
| TASK-033 | Implement `parse_delete_clause()` (with DETACH support) | query |
| TASK-034 | Implement top-level `parse_query()`: dispatch to clause parsers | query |
| TASK-035 | Integration tests: full query round-trips (parse → check AST shape) | query |

### Group H: Semantic Analysis

| Task | Description | Crate |
|------|-------------|-------|
| TASK-036 | Implement query-local `SymbolTable`: variable bindings, scope rules | query |
| TASK-037 | Implement `SemanticAnalyzer::analyze()`: variable scope validation | query |
| TASK-038 | Implement label/rel-type/prop-key resolution via `&mut dyn LabelRegistry` | query |
| TASK-039 | Unit tests with `MockCatalog`; test undeclared variable errors | query |

### Group I: Logical Planner

| Task | Description | Crate |
|------|-------------|-------|
| TASK-040 | Define `LogicalPlan` enum (all operators, u32 IDs for labels/types) | query |
| TASK-041 | Implement `LogicalPlanner::plan()`: MATCH → NodeScan + Expand chain | query |
| TASK-042 | Implement predicate pushdown and label-filter merge optimizations | query |
| TASK-043 | Unit tests: single-node MATCH, two-hop MATCH, MATCH+WHERE, MATCH+CREATE | query |

### Group J: Executor

| Task | Description | Crate |
|------|-------------|-------|
| TASK-044 | Define `Value`, `Record`, `PhysicalOperator` trait, `Params` | query |
| TASK-045 | Implement `eval()` expression evaluator with typed comparison (`eval_cmp`) | query |
| TASK-046 | Implement `NodeScanOp` (delegates to `scan_nodes` / `scan_nodes_by_label`) | query |
| TASK-047 | Implement `ExpandOp` (traverses `next_edge_id` linked list, O(degree)) | query |
| TASK-048 | Implement `FilterOp` (calls `eval()` on predicate) | query |
| TASK-049 | Implement `ProjectOp` (evaluates RETURN expressions, column rename) | query |
| TASK-050 | Implement `LimitOp` and `SkipOp` | query |
| TASK-051 | Implement `SortOp` and `AggregateOp` (materialized) | query |
| TASK-052 | Implement `CreateOp`, `DeleteOp`, `SetPropsOp` | query |
| TASK-053 | Unit tests for each operator; eval tests covering type mismatch, null semantics | query |

### Group K: Public API and Integration

| Task | Description | Crate |
|------|-------------|-------|
| TASK-054 | Implement `QueryResult`, `Row`, `FromValue` trait | query |
| TASK-055 | Implement `CypherLite::open()`, `execute()`, `execute_with_params()` | query |
| TASK-056 | Implement `CypherLite::begin()`, `Transaction::commit()`, `Transaction::rollback()` | query |
| TASK-057 | End-to-end integration tests: MATCH+RETURN, CREATE, WHERE, SET, DELETE against real StorageEngine | query |
| TASK-058 | proptest: random token sequences must not panic the parser | query |
| TASK-059 | Benchmark: parse + plan + execute for simple MATCH and 2-hop MATCH | query |

**Total**: 59 tasks across 11 groups.

**Critical path**: A (Catalog) → B (Scan APIs) → C (Scaffold) → D (Lexer) → E+F (Parser) → G (Clauses) → H (Semantic) → I (Planner) → J (Executor) → K (API).

---

## 11. Risk Assessment

### R-001: Grammar Scope Creep
**Risk**: Implementing too much of openCypher in Phase 2 delays completion.
**Mitigation**: Parser handles P0+P1+P2 syntax but emits `UnsupportedSyntax` for P2 clause execution (ORDER BY, SKIP, LIMIT, MERGE, UNION). Executor implements P0+P1 only. P2 execution in Phase 3.

### R-002: StorageEngine Not Send
**Risk**: Users may expect multi-threaded query execution or async integration.
**Mitigation**: Document `CypherLite: !Send` in the public crate root. Async wrappers (spawn_blocking) are the responsibility of the FFI/binding layer, not the core library. The `unsafe transmute` in `mvcc.rs:48` must be fixed before any threading is added.

### R-003: Catalog Persistence and WAL
**Risk**: Catalog mutations (new label IDs) must be durable. If a CREATE adds a new label, a crash before checkpoint must replay the catalog page from WAL.
**Mitigation**: Catalog writes go through `wal_write_page(CATALOG_PAGE_ID, ...)` before the transaction commit. On recovery, the catalog page is replayed from WAL exactly like any other page. This is automatically handled by the existing WAL recovery path if catalog page is a normal page.

### R-004: O(E) Edge Scan Acknowledged
**Risk**: `scan_edges_by_type()` is O(E). Large graphs will be slow for relationship-type-filtered scans.
**Mitigation**: Documented in the SPEC and executor source. `ExpandOp` uses `next_edge_id` chain (O(degree)), not `scan_edges_by_type`. The latter is available for administrative queries but not used in the MATCH executor path. Index on relationship type deferred to Phase 3.

### R-005: Null Semantics in Comparisons
**Risk**: Cypher null semantics differ from SQL in WHERE clause evaluation (`null = null` is null, not true). Incorrect null handling causes silent query correctness bugs.
**Mitigation**: `eval_cmp` explicitly handles `Value::Null` on either side: always returns false for comparison operators (matching openCypher spec). Unit tests must cover null in WHERE predicates.

### R-006: `PropertyValue` <-> `Value` Conversion
**Risk**: Two representations of property values exist: `PropertyValue` (storage layer) and `Value` (query executor). Conversion must handle all variants correctly and symmetrically.
**Mitigation**: Implement `From<PropertyValue> for Value` and `TryFrom<Value> for PropertyValue` in `cypherlite-query`. Property write path (SET) uses `TryFrom`. Tests cover all 7 type variants including array.

### R-007: SymbolTable vs. Catalog Naming Confusion
**Risk**: Two structures are named "symbol table" in the codebase: the query-local variable binding table (in semantic analysis) and the string-to-u32 catalog (in storage). This causes confusion.
**Mitigation**: Name them precisely: `SymbolTable` = query-local variable scope (lives in `cypherlite-query::semantic`). `Catalog` = persistent string-ID registry (lives in `cypherlite-storage::catalog`). `LabelRegistry` = the trait bridging them.

### R-008: logos MSRV Drift
**Risk**: logos bumps MSRV above 1.84 in a future release.
**Mitigation**: Pin `logos = "0.14"` in Cargo.toml. Verify before upgrading.

### R-009: SortOp Memory
**Risk**: `SortOp` materializes all rows; large scans OOM.
**Mitigation**: Add `max_sort_rows: usize` (default 100_000) to `DatabaseConfig`. `SortOp` returns `ExecutionError` when exceeded. External sort deferred to Phase 3.

---

## Appendix: Cypher Subset for Phase 2

### P0 (must-have for v0.2 release)
- `MATCH (n[:Label]) RETURN n`
- `MATCH (n)-[r[:TYPE]]->(m) RETURN n, r, m`
- `CREATE (n:Label {key: value})`
- `CREATE (n)-[:TYPE]->(m)` (where n, m already bound by MATCH)

### P1 (important for practical use)
- `WHERE expression` (comparisons, AND/OR/NOT, IS NULL, IN list)
- `SET n.prop = expr`, `SET n:Label`
- `REMOVE n.prop`, `REMOVE n:Label`
- `DELETE n`, `DETACH DELETE n`
- Property access: `n.name`
- Aggregates in RETURN: `count(n)`, `count(DISTINCT n)`, `sum(n.value)`, `avg(n.value)`, `min(n.value)`, `max(n.value)`
- `$param` substitution

### P2 (parser supported, executor deferred to Phase 3)
- `ORDER BY`, `SKIP`, `LIMIT`
- `MERGE`
- `WITH`
- `UNION` / `UNION ALL`
- OPTIONAL MATCH (parse only)
- Variable-length paths `(a)-[*1..3]->(b)` (parse: UnsupportedSyntax)

---

*Architecture design complete (v2). Ready for synthesis into SPEC-DB-002.*
