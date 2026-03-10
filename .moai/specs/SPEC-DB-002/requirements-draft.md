# SPEC-DB-002 Requirements Analysis

> CypherLite Phase 2 — openCypher Subset Query Engine
> Analyst: team-reader (analyst role)
> Date: 2026-03-10

---

## 1. openCypher Subset Definition (v1.0)

### Rationale for Subset Selection

CypherLite targets LLM agent memory systems and embedded knowledge graph applications. These use cases demand:
- Pattern traversal (finding related nodes/entities)
- Data ingestion (creating and updating knowledge)
- Basic filtering and aggregation (querying memory by relevance)

The following subset covers approximately 80% of real-world use cases for the target user base, based on analysis of openCypher usage patterns in agent frameworks and graph application development.

### Priority-Ordered Clause Set (v1.0)

| Priority | Clause | Justification |
|----------|--------|---------------|
| P0 | `MATCH` + `RETURN` | Core read path; every query uses this |
| P0 | `CREATE` | Core write path for node/edge insertion |
| P1 | `WHERE` | Essential filtering for all query types |
| P1 | `SET` / `REMOVE` | Property mutation; required for update workflows |
| P1 | `DELETE` / `DETACH DELETE` | Node/edge removal |
| P2 | `MERGE` | Idempotent upsert; critical for LLM agent "remember" operations |
| P2 | `WITH` | Subquery chaining for multi-hop patterns |
| P2 | `ORDER BY` / `LIMIT` / `SKIP` | Result pagination; important for streaming |
| P3 | `OPTIONAL MATCH` | Left-join semantics for incomplete graphs |
| P3 | `UNWIND` | List expansion; useful for bulk operations |

### Deferred to v2.0

- `CALL` subqueries (requires subquery execution engine)
- `FOREACH` (mutation inside expressions)
- `UNION` / `UNION ALL`
- `LOAD CSV`
- Stored procedures
- Full-text index queries (`CALL db.index.fulltext.*`)
- Graph algorithms via APOC-style functions

---

## 2. Pattern Syntax Scope

### v1.0 Supported Patterns

#### Node Patterns
```
(n)                          -- anonymous node, any label
(n:Label)                    -- node with single label
(n:Label {prop: value})      -- node with label and property predicate
(:Label)                     -- anonymous node, label only (for MATCH)
(n:Label:Label2)             -- multi-label node (AND semantics)
```

#### Relationship Patterns
```
-[r]-                        -- undirected, any type (traversal only)
-[r:TYPE]->                  -- directed, specific type
<-[r:TYPE]-                  -- directed incoming
-[r:TYPE {prop: value}]-     -- typed relationship with property predicate
-[:TYPE]->                   -- anonymous relationship with type
```

#### Path Patterns
```
(a)-[r]->(b)                 -- single-hop directed
(a)-[r]-(b)                  -- single-hop undirected
(a)-[r:TYPE*]->(b)           -- variable-length (1..n, unbounded)
(a)-[r:TYPE*1..3]->(b)       -- variable-length bounded (min..max)
(a)-[r:TYPE*..5]->(b)        -- up to 5 hops
(a)-[r:TYPE*2..]->(b)        -- at least 2 hops
```

#### Named Paths
```
p = (a)-[*1..3]->(b)         -- bind full path to variable p
```

### v2.0 Deferred Patterns

- Quantified path patterns (QPP): `(a)((n)-[r]->(m))+(b)` (openCypher 2.0)
- Shortest path: `shortestPath()`, `allShortestPaths()`
- Node pattern predicates beyond property equality (complex WHERE inside pattern)

---

## 3. Expression Scope

### v1.0 Supported Expressions

#### Comparison Operators
| Operator | Semantics |
|----------|-----------|
| `=` | Equality (type-aware) |
| `<>` | Inequality |
| `<`, `>`, `<=`, `>=` | Ordered comparison (numeric and string) |
| `IN` | List membership test |

#### Boolean Operators
| Operator | Semantics |
|----------|-----------|
| `AND` | Short-circuit logical AND |
| `OR` | Short-circuit logical OR |
| `NOT` | Logical negation |
| `XOR` | Exclusive OR |

#### String Operators
| Operator | Semantics |
|----------|-----------|
| `STARTS WITH` | Prefix match |
| `ENDS WITH` | Suffix match |
| `CONTAINS` | Substring match |
| `=~` | Regular expression match (RE2 syntax) |

#### Mathematical Operators
| Operator | Semantics |
|----------|-----------|
| `+` | Addition (numeric) or concatenation (string) |
| `-` | Subtraction |
| `*` | Multiplication |
| `/` | Division (float for integer/integer) |
| `%` | Modulo |
| `^` | Exponentiation |

#### NULL Handling
| Expression | Semantics |
|------------|-----------|
| `IS NULL` | True if value is null |
| `IS NOT NULL` | True if value is non-null |
| Comparison with NULL | Always returns NULL (three-valued logic) |

#### Type and Identity Functions (v1.0)
| Function | Return Type | Notes |
|----------|-------------|-------|
| `type(r)` | String | Relationship type name |
| `id(n)` | Integer | Internal node/edge ID (u64) |
| `labels(n)` | List<String> | Node label names |
| `keys(n)` | List<String> | Property key names |
| `properties(n)` | Map | All properties as map |
| `size(list)` | Integer | List length or string char count |
| `length(path)` | Integer | Hop count in path |
| `coalesce(a, b, ...)` | Any | First non-null value |
| `toString(x)` | String | Type coercion to string |
| `toInteger(x)` | Integer | Type coercion to integer |
| `toFloat(x)` | Float | Type coercion to float |
| `toBoolean(x)` | Boolean | Type coercion to boolean |

#### Aggregation Functions (with GROUP BY semantics via RETURN keys)
| Function | Return Type |
|----------|-------------|
| `count(*)` | Integer |
| `count(expr)` | Integer (excludes NULL) |
| `sum(expr)` | Numeric |
| `avg(expr)` | Float |
| `min(expr)` | Comparable |
| `max(expr)` | Comparable |
| `collect(expr)` | List |
| `count(DISTINCT expr)` | Integer |
| `collect(DISTINCT expr)` | List (deduplicated) |

#### List Expressions
| Expression | Semantics |
|------------|-----------|
| `[1, 2, 3]` | List literal |
| `list[0]` | Index access |
| `list[1..3]` | Range slice |
| `[x IN list WHERE pred]` | List comprehension |
| `[x IN list | expr]` | List transformation |

#### Map Expressions
| Expression | Semantics |
|------------|-----------|
| `{key: value}` | Map literal |
| `map.key` | Property access |
| `map[key]` | Dynamic property access |

### v2.0 Deferred Expressions

- Pattern comprehensions: `[(n:Person)-[r]->(m) | m.name]`
- `CASE WHEN` expressions (deferred to reduce parser complexity)
- `exists()` predicate function
- `any()`, `all()`, `none()`, `single()` list predicates
- Date/time functions: `datetime()`, `date()`, `duration()`
- Mathematical functions: `abs()`, `ceil()`, `floor()`, `round()`, `sqrt()`

---

## 4. EARS Requirements

### Module 1: Lexer/Tokenizer

**REQ-LEX-001** (Ubiquitous): The lexer shall tokenize a valid openCypher v1.0 query string into a sequence of typed tokens without data loss.

**REQ-LEX-002** (Ubiquitous): The lexer shall be case-insensitive for all keywords (MATCH, WHERE, RETURN, etc.) while preserving case for identifiers and string literals.

**REQ-LEX-003** (Event-driven): When the lexer encounters an unrecognized character sequence, it shall emit a `LexError` with the byte offset and offending character.

**REQ-LEX-004** (Ubiquitous): The lexer shall support the following token categories: Keywords, Identifiers, Integer literals, Float literals, String literals (single and double quoted), Boolean literals (true/false), NULL literal, Operators, Punctuation, Comments (single-line `//` only).

**REQ-LEX-005** (Ubiquitous): The lexer shall support unicode identifiers (UTF-8 encoded) for node and property names.

**REQ-LEX-006** (State-driven): While scanning a string literal, the lexer shall handle escape sequences: `\n`, `\t`, `\r`, `\\`, `\'`, `\"`, `\uXXXX`.

**REQ-LEX-007** (Ubiquitous): The lexer shall preserve source position (line, column, byte offset) for every token to enable descriptive error messages.

**REQ-LEX-008** (Conditional): Where the input is empty or contains only whitespace and comments, the lexer shall produce an empty token stream without error.

### Module 2: Parser/AST

**REQ-PARSE-001** (Ubiquitous): The parser shall accept a token stream from the lexer and produce a typed Abstract Syntax Tree (AST) representing the query structure.

**REQ-PARSE-002** (Ubiquitous): The AST shall represent all v1.0 clauses: MATCH, OPTIONAL MATCH, CREATE, MERGE, SET, REMOVE, DELETE, DETACH DELETE, WITH, RETURN, ORDER BY, LIMIT, SKIP, UNWIND, WHERE.

**REQ-PARSE-003** (Event-driven): When the parser encounters a syntax error, it shall return a `ParseError` containing: the offending token, its source position (line, column), and a human-readable message describing the expected token or construct.

**REQ-PARSE-004** (Ubiquitous): The parser shall validate that all variable names used in a clause are either introduced in the same clause or bound in a preceding clause (semantic scoping check).

**REQ-PARSE-005** (Ubiquitous): The AST shall be fully owned (no borrows from source string) to support multi-phase query planning.

**REQ-PARSE-006** (Ubiquitous): The parser shall support all v1.0 pattern syntax including variable-length relationships with optional bounds (`*min..max`).

**REQ-PARSE-007** (Conditional): Where a query contains multiple clauses, the parser shall model the clause sequence as an ordered list that preserves reading order for the query planner.

**REQ-PARSE-008** (Ubiquitous): The parser shall be implemented as a recursive-descent parser with no external parser-generator dependencies, for compile-time predictability and WASM compatibility.

### Module 3: Query Planner

**REQ-PLAN-001** (Ubiquitous): The query planner shall transform a parsed AST into a physical execution plan (operator tree).

**REQ-PLAN-002** (Ubiquitous): The planner shall apply cost-based optimization for MATCH clauses, selecting the lowest-cost node scan or index scan as the anchor node.

**REQ-PLAN-003** (Ubiquitous): The planner shall push WHERE predicates as close to the data source as possible (predicate pushdown).

**REQ-PLAN-004** (Event-driven): When a MATCH pattern contains a node with both a label and an equality predicate on a indexed property, the planner shall prefer an index scan over a full label scan.

**REQ-PLAN-005** (Ubiquitous): The planner shall estimate cardinality for each operator using stored statistics (node count per label, edge count per type).

**REQ-PLAN-006** (Ubiquitous): The planner shall produce a logical plan first (relational-algebra-style), then lower it to a physical plan (iterator model).

**REQ-PLAN-007** (Conditional): Where a query contains an ORDER BY without LIMIT, the planner shall emit a warning and choose a sort algorithm appropriate for the estimated result set size.

**REQ-PLAN-008** (Ubiquitous): The planner shall detect Cartesian products (unconnected MATCH patterns) and emit a diagnostic warning, as these are almost always unintentional.

**REQ-PLAN-009** (State-driven): While planning a variable-length path pattern, the planner shall enforce a configurable maximum hop depth (default: 10) to prevent unbounded traversal.

### Module 4: Executor

**REQ-EXEC-001** (Ubiquitous): The executor shall evaluate a physical plan against the storage engine using the Volcano/Iterator model (open/next/close interface).

**REQ-EXEC-002** (Ubiquitous): The executor shall integrate with the MVCC transaction layer from SPEC-DB-001, using snapshot isolation for all read operations.

**REQ-EXEC-003** (Event-driven): When a CREATE clause is executed, the executor shall call the storage engine's node/edge insertion API and bind the new entity's ID to the query variable.

**REQ-EXEC-004** (Event-driven): When a MERGE clause is executed, the executor shall first attempt a MATCH, and only execute the CREATE path if no match is found, within the same transaction.

**REQ-EXEC-005** (Event-driven): When a DELETE clause is executed on a node that still has relationships, the executor shall return a `ConstraintError` unless DETACH DELETE is used.

**REQ-EXEC-006** (Ubiquitous): The executor shall perform type coercion for arithmetic expressions according to the openCypher type promotion rules (integer + float = float).

**REQ-EXEC-007** (Ubiquitous): The executor shall implement three-valued logic (TRUE, FALSE, NULL) for all boolean and comparison expressions.

**REQ-EXEC-008** (State-driven): While executing a variable-length path traversal, the executor shall track visited edges to prevent infinite loops in cyclic graphs.

**REQ-EXEC-009** (Conditional): Where an aggregation function is present in RETURN without a GROUP BY clause, the executor shall treat all non-aggregated RETURN expressions as implicit grouping keys.

**REQ-EXEC-010** (Ubiquitous): The executor shall expose a `QueryContext` struct containing: the active transaction handle, a symbol table for variable bindings, and a configurable resource limit (max rows scanned, max memory).

### Module 5: Result Streaming

**REQ-STREAM-001** (Ubiquitous): The result streaming layer shall expose query results as a Rust `Iterator<Item = Result<Row>>` interface, enabling lazy row-by-row consumption.

**REQ-STREAM-002** (Ubiquitous): A `Row` shall be a map from column names to `PropertyValue` variants, preserving the type information from the executor.

**REQ-STREAM-003** (Event-driven): When an executor error occurs mid-stream, the iterator shall yield `Err(QueryError)` on the next `next()` call and halt further iteration.

**REQ-STREAM-004** (Conditional): Where a LIMIT clause is present, the streaming layer shall close the executor and release all resources as soon as the limit is reached.

**REQ-STREAM-005** (Ubiquitous): The streaming layer shall support collecting results into a `Vec<Row>` via a convenience method `collect_all()` for non-streaming consumers.

**REQ-STREAM-006** (Ubiquitous): The result iterator shall be `Send` to allow query results to be consumed across thread boundaries (required for async runtime compatibility).

**REQ-STREAM-007** (Optional): Where the WASM feature flag is enabled, the streaming layer shall expose a synchronous, non-threaded iteration interface compatible with single-threaded WASM environments.

---

## 5. Acceptance Criteria

### AC-001: Basic MATCH Query Returns Nodes
```
Given: A database containing 3 Person nodes with name properties "Alice", "Bob", "Carol"
When: MATCH (n:Person) RETURN n.name is executed
Then: The result contains exactly 3 rows
  And: Each row has a "n.name" column with values "Alice", "Bob", "Carol" (any order)
  And: The query completes in under 10ms (p99)
```

### AC-002: CREATE Node and Relationship
```
Given: An empty database
When: CREATE (a:Person {name: "Alice"})-[:KNOWS]->(b:Person {name: "Bob"}) is executed
Then: The transaction commits successfully
  And: A subsequent MATCH (n:Person) RETURN count(n) returns 2
  And: A subsequent MATCH (a)-[:KNOWS]->(b) RETURN b.name returns "Bob"
```

### AC-003: MATCH with WHERE Filter
```
Given: A database with 5 Person nodes with age properties 20, 25, 30, 35, 40
When: MATCH (n:Person) WHERE n.age > 28 RETURN n.age ORDER BY n.age is executed
Then: The result contains exactly 3 rows with values 30, 35, 40 in that order
  And: Rows with age <= 28 are excluded
```

### AC-004: Two-Hop Pattern Matching
```
Given: A graph (Alice)-[:KNOWS]->(Bob)-[:KNOWS]->(Carol)
When: MATCH (a:Person {name: "Alice"})-[:KNOWS*2]->(c:Person) RETURN c.name is executed
Then: The result contains exactly 1 row with value "Carol"
  And: "Bob" is not in the result (he is 1 hop, not 2)
  And: The query completes in under 50ms (p99)
```

### AC-005: Syntax Error Handling
```
Given: A malformed query string "MATCH n RETURN n" (missing parentheses)
When: The query is parsed
Then: A ParseError is returned (not a panic)
  And: The error message contains the line number and column of the syntax error
  And: The error message identifies the offending token and the expected construct
```

### AC-006: Type Mismatch Error
```
Given: A database with a Person node with age = 30 (integer)
When: MATCH (n:Person) WHERE n.age + "foo" > 0 RETURN n is executed
Then: A TypeError is returned before any rows are produced
  And: The error message identifies the incompatible types
```

### AC-007: Transaction Isolation
```
Given: Transaction T1 has started a read on Person nodes
  And: Transaction T2 creates a new Person node and commits
When: T1 reads Person nodes again within the same transaction
Then: T1 does NOT see the node created by T2 (snapshot isolation)
  And: A new Transaction T3 started after T2's commit DOES see the new node
```

### AC-008: Performance — Simple MATCH (p99 < 10ms)
```
Given: A database with 10,000 Person nodes
When: MATCH (n:Person {name: "Alice"}) RETURN n is executed 1,000 times
Then: The p99 latency is under 10ms
  And: Memory allocated per query execution does not exceed 1MB
```

### AC-009: Performance — Two-Hop Pattern (p99 < 50ms)
```
Given: A database with 1,000 Person nodes and 5,000 KNOWS relationships
When: MATCH (a:Person)-[:KNOWS*2]->(c:Person) RETURN count(c) is executed 100 times
Then: The p99 latency is under 50ms
```

### AC-010: NULL Handling
```
Given: A database with Person nodes where some have a "email" property and some do not
When: MATCH (n:Person) WHERE n.email IS NOT NULL RETURN n.name is executed
Then: Only nodes with a non-null email property are returned
When: MATCH (n:Person) RETURN n.email is executed
Then: Nodes without the email property return NULL in the "n.email" column
  And: NULL does not cause an error
```

### AC-011: MERGE Idempotency
```
Given: A database with a Person node {name: "Alice"}
When: MERGE (n:Person {name: "Alice"}) is executed twice
Then: The database still contains exactly 1 Person node named "Alice" after both executions
When: MERGE (n:Person {name: "Dave"}) is executed
Then: A new Person node {name: "Dave"} is created
```

### AC-012: Result Streaming with LIMIT
```
Given: A database with 10,000 nodes
When: MATCH (n) RETURN n LIMIT 10 is executed
Then: Exactly 10 rows are returned
  And: Storage scan is terminated after finding 10 matching rows (no full scan)
  And: Memory usage is bounded to the first 10 result rows
```

---

## 6. Non-Functional Requirements

### Performance

| Metric | Target | Measurement Method |
|--------|--------|--------------------|
| Simple MATCH (p99) | < 10ms | Criterion benchmark, 10K nodes, 1K iterations |
| 2-hop pattern (p99) | < 50ms | Criterion benchmark, 1K nodes, 5K edges, 100 iterations |
| Parse latency (p99) | < 1ms | Lexer + parser for typical query (50–200 chars) |
| Plan latency (p99) | < 2ms | Planner for 3-clause query |
| Memory per query | < 10MB | Heap profiler, worst-case aggregation |

### Memory Budget

- Query execution memory limit: configurable via `QueryConfig::max_memory_bytes` (default: 64MB)
- Intermediate result buffers for ORDER BY: spill to storage not required in v1.0; return `ResourceLimitError` if exceeded
- Symbol table allocation: O(number of variables in query)
- No global mutable state in query core (required for re-entrancy and WASM)

### Error Reporting

All errors returned by the query engine shall:
- Implement `std::error::Error` + `Display` + `Debug`
- Include an error kind enum for programmatic handling
- For parse errors: include source position (line, column, byte offset)
- For type errors: include the expression that failed and the actual vs. expected types
- For runtime errors: include the clause being executed and the entity involved
- Never expose internal implementation details (no raw Rust panic messages)

Error kind taxonomy:
```
QueryError::Parse(ParseError)        -- lexer/parser errors
QueryError::Type(TypeError)          -- type mismatch errors
QueryError::Constraint(ConstraintError)  -- e.g., delete node with relationships
QueryError::ResourceLimit(...)       -- memory or scan limit exceeded
QueryError::Storage(StorageError)    -- propagated from SPEC-DB-001 layer
QueryError::Internal(...)            -- unexpected errors (bug reports)
```

### Rust API Design

Public API surface for v1.0:

```rust
// Top-level query execution API
impl CypherLite {
    pub fn query(&self, cypher: &str) -> Result<QueryResult, QueryError>;
    pub fn query_with_params(
        &self,
        cypher: &str,
        params: HashMap<String, PropertyValue>
    ) -> Result<QueryResult, QueryError>;
}

// Streaming interface
pub struct QueryResult {
    pub columns: Vec<String>,
    pub rows: impl Iterator<Item = Result<Row, QueryError>>,
}

pub type Row = HashMap<String, PropertyValue>;
```

### WASM Compatibility

- Query core (lexer, parser, planner, executor) shall not use `std::thread` directly
- No thread-local storage that cannot be reset between calls
- No platform-specific system calls in the query path
- Async executor integration is opt-in via a feature flag
- WASM build target: `wasm32-unknown-unknown` (no WASI required for query core)

### Observability

- Each query execution shall emit a `QueryPlan` struct inspectable via `EXPLAIN`-equivalent API
- Query execution statistics (rows scanned, rows returned, execution time) shall be captured per-query
- No external logging dependencies; emit events via a pluggable observer/callback interface

---

## 7. Out of Scope (v1.0)

The following features are explicitly deferred to future versions:

### Deferred Query Language Features

| Feature | Reason for Deferral |
|---------|---------------------|
| `CALL` subqueries | Requires subquery execution context and correlated variable binding |
| `UNION` / `UNION ALL` | Requires schema compatibility validation between branches |
| `FOREACH` | Mutation inside expressions; complex interaction with MVCC |
| `LOAD CSV` | I/O integration outside query engine scope |
| `CASE WHEN` expressions | Incremental scope; can be emulated with WHERE + OPTIONAL MATCH |
| `exists()` predicate | Complex interaction with variable-length patterns |
| `any()`, `all()`, `none()` | List predicate functions; low priority for target use cases |
| Date/time functions | Temporal features planned for Phase 3 (SPEC-DB-003) |
| Full-text search | Requires inverted index (plugin scope) |
| Stored procedures | Runtime extensibility for Phase 4+ |
| APOC-style functions | Plugin scope; not in core engine |
| Graph algorithms | Shortest path, PageRank, community detection — plugin scope |
| `shortestPath()` | Path algorithm; deferred to v1.1 or plugin |
| Named graph patterns (GQL-style) | openCypher 2.0 feature, not in current spec |
| Quantified path patterns (QPP) | openCypher 2.0 syntax `(a)((n)-->(m))+(b)` |

### Deferred Infrastructure Features

| Feature | Reason for Deferral |
|---------|---------------------|
| Query result caching | Requires invalidation logic tied to write transactions |
| Prepared statement parameter binding beyond basic scalars | Complex type-mapping for nested structures |
| Concurrent query execution (parallel plans) | Single-writer model in Phase 1 limits parallelism benefit |
| Query timeout enforcement | Requires interruptible executor (cooperative cancellation) |
| Index creation via Cypher (`CREATE INDEX ON :Label(prop)`) | Schema management DDL; Phase 3 |
| Schema constraints (`CREATE CONSTRAINT`) | DDL; Phase 3 |
| Property type enforcement | Schema-free design in v1.0 |
| Python/Node.js bindings with async query streaming | FFI bindings planned for Phase 4 |

### Explicitly Not Supported in v1.0

- Multi-database queries (`USE database`)
- Role-based access control
- Network/remote query protocol
- Query plan hints (`USING INDEX`, `USING SCAN`)
- Transaction bookmarks (causal consistency across sessions)
- Point spatial data types and spatial functions

---

## Appendix: Integration with SPEC-DB-001 Storage API

The query engine integrates with the Phase 1 storage engine through the following data types and traits:

**Used from `cypherlite-core`:**
- `NodeId(u64)` — node identity
- `EdgeId(u64)` — edge identity
- `PropertyValue` — all 7 variants (Null, Bool, Int64, Float64, String, Bytes, Array)
- `NodeRecord` — full node record with label IDs and property map
- `RelationshipRecord` — full edge record with start/end node, type ID, adjacency chain
- `Direction` — Outgoing, Incoming, Both
- `TransactionView` — snapshot frame for MVCC read consistency

**Label and type name resolution:**
The storage engine uses integer IDs for label names (`labels: Vec<u32>`) and relationship type IDs (`rel_type_id: u32`). The query engine must maintain a string-to-ID catalog (stored as nodes in a reserved B-tree or in the database header) to resolve `Person` → `u32(1)`, `KNOWS` → `u32(5)`, etc.

**Property key resolution:**
Properties are stored as `Vec<(u32, PropertyValue)>`. The query engine maintains a property-key-name catalog (similar to label catalog) mapping `"name"` → `u32(1)`.

These catalogs are a required deliverable for SPEC-DB-002 that is not present in SPEC-DB-001.
