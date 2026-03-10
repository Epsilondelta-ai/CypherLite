---
id: SPEC-DB-003
type: acceptance
version: "1.0.0"
status: draft
created: "2026-03-10"
updated: "2026-03-10"
author: epsilondelta
priority: P1
tags: [advanced-query, with, merge, optional-match, unwind, variable-length-paths, indexing, optimization]
---

# SPEC-DB-003 Acceptance Criteria: CypherLite Phase 3 - Advanced Query Features

---

## HISTORY

| Version | Date | Changes |
|---------|------|---------|
| 1.0.0 | 2026-03-10 | Initial acceptance criteria based on research analysis |

---

## 1. Functional Acceptance Criteria

### Phase 3a: Core Clauses

#### AC-020: WITH Clause -- Scope Reset and Projection

```gherkin
Given: A database with Person nodes {name: "Alice", age: 30}, {name: "Bob", age: 25}, {name: "Carol", age: 35}
When: MATCH (n:Person) WITH n.name AS name, n.age AS age RETURN name, age is executed
Then: The result contains exactly 3 rows with projected columns "name" and "age"
  And: The original variable "n" is no longer accessible (scope reset)
  And: Attempting to reference "n" after WITH produces a SemanticError
```

#### AC-021: WITH WHERE -- Filtering After Projection

```gherkin
Given: A database with Person nodes having age values 20, 25, 30, 35, 40
When: MATCH (n:Person) WITH n, n.age AS age WHERE age > 28 RETURN n.name is executed
Then: The result contains exactly 3 rows (persons with age 30, 35, 40)
  And: Persons with age <= 28 are excluded
```

#### AC-022: WITH Aggregation -- Grouping with Scope Barrier

```gherkin
Given: A database with Person nodes having city values "Seoul" (2 nodes) and "Tokyo" (1 node)
When: MATCH (n:Person) WITH n.city AS city, count(*) AS cnt WHERE cnt > 1 RETURN city, cnt is executed
Then: The result contains exactly 1 row: {city: "Seoul", cnt: 2}
  And: "Tokyo" is excluded because cnt = 1 which does not satisfy cnt > 1
```

#### AC-023: WITH DISTINCT -- Deduplication

```gherkin
Given: A database with Person nodes having city values "Seoul", "Seoul", "Tokyo", "Tokyo", "Tokyo"
When: MATCH (n:Person) WITH DISTINCT n.city AS city RETURN city is executed
Then: The result contains exactly 2 rows: "Seoul" and "Tokyo"
  And: Duplicate cities are removed
```

#### AC-024: UNWIND -- List Expansion (Literal)

```gherkin
Given: An empty database (UNWIND does not require existing data)
When: UNWIND [1, 2, 3] AS x RETURN x is executed
Then: The result contains exactly 3 rows with values 1, 2, 3
  And: Each row has a single column "x"
```

#### AC-025: UNWIND -- List Property Expansion

```gherkin
Given: A database with a Person node {name: "Alice", hobbies: ["reading", "coding", "gaming"]}
When: MATCH (n:Person {name: "Alice"}) UNWIND n.hobbies AS hobby RETURN hobby is executed
Then: The result contains exactly 3 rows: "reading", "coding", "gaming"
```

#### AC-026: UNWIND -- Empty List Produces Zero Rows

```gherkin
Given: A database with a Person node {name: "Alice", tags: []}
When: MATCH (n:Person) UNWIND n.tags AS tag RETURN tag is executed
Then: The result contains 0 rows
  And: No error is returned (empty list is valid)
```

#### AC-027: UNWIND -- NULL List Produces Zero Rows

```gherkin
Given: A database with a Person node {name: "Alice"} that has no "tags" property
When: MATCH (n:Person) UNWIND n.tags AS tag RETURN tag is executed
Then: The result contains 0 rows
  And: No error is returned (NULL is treated as empty list)
```

#### AC-028: UNWIND -- Non-List Value Produces Error

```gherkin
Given: A database with a Person node {name: "Alice", age: 30}
When: MATCH (n:Person) UNWIND n.age AS x RETURN x is executed
Then: An ExecutionError is returned indicating that UNWIND requires a list expression
  And: The error message identifies the actual type received
```

#### AC-029: OPTIONAL MATCH -- NULL Propagation

```gherkin
Given: A database with (Alice:Person)-[:KNOWS]->(Bob:Person) and (Carol:Person) with no outgoing KNOWS edges
When: MATCH (a:Person) OPTIONAL MATCH (a)-[:KNOWS]->(b) RETURN a.name, b.name is executed
Then: The result contains 3 rows:
  | a.name  | b.name |
  | "Alice" | "Bob"  |
  | "Bob"   | NULL   |
  | "Carol" | NULL   |
  And: Rows where no match exists have NULL for the optional variable "b"
```

#### AC-030: OPTIONAL MATCH -- Left Join with Multiple Matches

```gherkin
Given: A database with (Alice)-[:KNOWS]->(Bob) and (Alice)-[:KNOWS]->(Carol)
When: MATCH (a:Person {name: "Alice"}) OPTIONAL MATCH (a)-[:KNOWS]->(b) RETURN a.name, b.name is executed
Then: The result contains exactly 2 rows:
  | a.name  | b.name  |
  | "Alice" | "Bob"   |
  | "Alice" | "Carol" |
```

#### AC-031: OPTIONAL MATCH -- Chained with Regular MATCH

```gherkin
Given: A database with (Alice)-[:KNOWS]->(Bob) and no WORKS_AT relationships
When: MATCH (a:Person {name: "Alice"}) OPTIONAL MATCH (a)-[:WORKS_AT]->(c) RETURN a.name, c is executed
Then: The result contains 1 row: {a.name: "Alice", c: NULL}
  And: The query does not fail even though no WORKS_AT edges exist
```

#### AC-032: OPTIONAL MATCH -- NULL in WHERE Filter

```gherkin
Given: A database with (Alice)-[:KNOWS]->(Bob {age: 25}) and (Carol) with no outgoing KNOWS edges
When: MATCH (a:Person) OPTIONAL MATCH (a)-[:KNOWS]->(b) WHERE b.age > 20 RETURN a.name, b.name is executed
Then: Carol's row has b.name as NULL (OPTIONAL MATCH filter on NULL yields no match, but source row preserved)
  And: Alice's row has b.name as "Bob" (filter b.age > 20 passes)
```

---

### Phase 3b: MERGE + Indexing

#### AC-033: MERGE -- Create If Not Exists (Node)

```gherkin
Given: An empty database
When: MERGE (n:Person {name: "Alice"}) is executed
Then: A new Person node with name "Alice" is created
  And: Subsequent MATCH (n:Person) RETURN count(n) returns 1
```

#### AC-034: MERGE -- Idempotency (Node)

```gherkin
Given: A database with one Person node {name: "Alice"}
When: MERGE (n:Person {name: "Alice"}) is executed
Then: No new node is created
  And: The database still contains exactly 1 Person node with name "Alice"
When: MERGE (n:Person {name: "Alice"}) is executed a second time
Then: Still exactly 1 Person node with name "Alice" exists
```

#### AC-035: MERGE -- Create New When No Match

```gherkin
Given: A database with one Person node {name: "Alice"}
When: MERGE (n:Person {name: "Bob"}) is executed
Then: A new Person node with name "Bob" is created
  And: The database now contains exactly 2 Person nodes
```

#### AC-036: MERGE -- ON CREATE SET

```gherkin
Given: An empty database
When: MERGE (n:Person {name: "Alice"}) ON CREATE SET n.created = true is executed
Then: A new Person node is created with {name: "Alice", created: true}
When: MERGE (n:Person {name: "Alice"}) ON CREATE SET n.created = true is executed again
Then: The existing node is matched; ON CREATE SET is NOT applied again
  And: The node still has {name: "Alice", created: true} (unchanged)
```

#### AC-037: MERGE -- ON MATCH SET

```gherkin
Given: A database with Person node {name: "Alice", visits: 1}
When: MERGE (n:Person {name: "Alice"}) ON MATCH SET n.visits = n.visits + 1 is executed
Then: The existing node is matched
  And: The visits property is updated to 2
  And: No new node is created
```

#### AC-038: MERGE -- ON MATCH + ON CREATE Combined

```gherkin
Given: A database with Person node {name: "Alice"}
When: MERGE (n:Person {name: "Alice"}) ON MATCH SET n.lastSeen = "today" ON CREATE SET n.firstSeen = "today" is executed
Then: The existing node is matched; ON MATCH SET applies: {name: "Alice", lastSeen: "today"}
  And: ON CREATE SET is NOT applied
When: MERGE (n:Person {name: "Bob"}) ON MATCH SET n.lastSeen = "today" ON CREATE SET n.firstSeen = "today" is executed
Then: A new node is created; ON CREATE SET applies: {name: "Bob", firstSeen: "today"}
  And: ON MATCH SET is NOT applied
```

#### AC-039: MERGE -- Relationship

```gherkin
Given: A database with Person nodes "Alice" and "Bob" but no KNOWS relationship between them
When: MATCH (a:Person {name: "Alice"}), (b:Person {name: "Bob"}) MERGE (a)-[r:KNOWS]->(b) is executed
Then: A KNOWS relationship is created from Alice to Bob
When: The same MERGE is executed again
Then: No duplicate relationship is created; exactly 1 KNOWS edge exists between Alice and Bob
```

#### AC-040: CREATE INDEX -- Basic Index Creation

```gherkin
Given: A database with 1,000 Person nodes with varying name properties
When: CREATE INDEX ON :Person(name) is executed
Then: An index is created on Person.name
  And: The index definition is persisted in the Catalog
  And: Subsequent MATCH (n:Person {name: "Alice"}) RETURN n uses the index (verify via plan inspection if available)
```

#### AC-041: Index-Assisted Query -- Equality Lookup

```gherkin
Given: A database with 10,000 Person nodes and an index on :Person(name)
When: MATCH (n:Person {name: "Alice"}) RETURN n is executed
Then: The result is correct (returns Alice node)
  And: The query planner selects IndexScan instead of NodeScan+Filter
  And: Query execution time is measurably faster than without index (benchmark)
```

#### AC-042: Index Auto-Update on Mutations

```gherkin
Given: A database with an index on :Person(name)
When: CREATE (n:Person {name: "Dave"}) is executed
Then: The index is automatically updated to include "Dave"
  And: Subsequent MATCH (n:Person {name: "Dave"}) RETURN n finds the node via index
When: MATCH (n:Person {name: "Dave"}) SET n.name = "David" is executed
Then: The index is updated: "Dave" entry removed, "David" entry added
  And: MATCH (n:Person {name: "Dave"}) returns 0 rows
  And: MATCH (n:Person {name: "David"}) returns 1 row
When: MATCH (n:Person {name: "David"}) DELETE n is executed
Then: The index entry for "David" is removed
```

#### AC-043: DROP INDEX

```gherkin
Given: A database with an existing index on :Person(name)
When: DROP INDEX person_name_idx is executed
Then: The index is removed
  And: The Catalog no longer contains the index definition
  And: Subsequent queries fall back to full label scan
```

---

### Phase 3c: Advanced Patterns + Optimization

#### AC-044: Variable-Length Path -- Bounded Range

```gherkin
Given: A graph (A)-[:KNOWS]->(B)-[:KNOWS]->(C)-[:KNOWS]->(D)-[:KNOWS]->(E)
When: MATCH (a {name: "A"})-[:KNOWS*1..3]->(x) RETURN x.name is executed
Then: The result contains exactly 3 rows: "B" (1 hop), "C" (2 hops), "D" (3 hops)
  And: "E" is NOT included (4 hops exceeds max 3)
  And: "A" is NOT included (0 hops is below min 1)
```

#### AC-045: Variable-Length Path -- Exact Hop Count

```gherkin
Given: A graph (A)-[:KNOWS]->(B)-[:KNOWS]->(C)-[:KNOWS]->(D)
When: MATCH (a {name: "A"})-[:KNOWS*2]->(x) RETURN x.name is executed
Then: The result contains exactly 1 row: "C" (exactly 2 hops)
  And: "B" (1 hop) and "D" (3 hops) are NOT included
```

#### AC-046: Variable-Length Path -- Unbounded with Max Cap

```gherkin
Given: A linear chain of 15 nodes connected by KNOWS edges: N1->N2->...->N15
When: MATCH (a {name: "N1"})-[:KNOWS*]->(x) RETURN x.name is executed with default max_hops = 10
Then: The result contains exactly 10 rows (N2 through N11)
  And: Nodes N12 through N15 are NOT included (beyond default max_hops)
  And: No error is returned; the query completes normally with truncated results
```

#### AC-047: Variable-Length Path -- Cycle Detection

```gherkin
Given: A cyclic graph (A)-[:KNOWS]->(B)-[:KNOWS]->(C)-[:KNOWS]->(A)
When: MATCH (a {name: "A"})-[:KNOWS*1..10]->(x) RETURN x.name is executed
Then: The result contains "B" and "C"
  And: The query terminates (does not loop infinitely)
  And: Each edge is traversed at most once in any single path
```

#### AC-048: Variable-Length Path -- Typed Relationships

```gherkin
Given: A graph with (A)-[:KNOWS]->(B)-[:WORKS_WITH]->(C)-[:KNOWS]->(D)
When: MATCH (a {name: "A"})-[:KNOWS*1..3]->(x) RETURN x.name is executed
Then: The result contains only "B" (1 hop via KNOWS)
  And: "C" is NOT included (reached via WORKS_WITH, not KNOWS)
  And: "D" is NOT included (no KNOWS path from A)
```

#### AC-049: Index Scan Selection -- Performance Improvement

```gherkin
Given: A database with 100,000 Person nodes and an index on :Person(name)
When: MATCH (n:Person {name: "Alice"}) RETURN n is executed with index
  And: The same query is executed without index (DROP INDEX, re-query)
Then: The indexed query is at least 10x faster than the non-indexed query
  And: Both queries return identical results
```

#### AC-050: LIMIT Pushdown -- Early Termination

```gherkin
Given: A database with 10,000 Person nodes
When: MATCH (n:Person) RETURN n LIMIT 5 is executed
Then: Exactly 5 rows are returned
  And: The executor stops scanning after finding 5 matching nodes (does not materialize all 10,000)
```

#### AC-051: Constant Folding -- Plan-Time Evaluation

```gherkin
Given: A database with Person nodes
When: MATCH (n:Person) WHERE n.age > 10 + 20 RETURN n is executed
Then: The query planner folds "10 + 20" to "30" at plan time
  And: The physical plan contains a filter with the constant value 30
  And: Query results are correct (same as WHERE n.age > 30)
```

---

## 2. Performance Gates

| Metric | Target | Measurement Method |
|--------|--------|--------------------|
| Index equality lookup (p99) | < 1ms | Criterion benchmark, 100K nodes, 1K iterations |
| Index range scan (p99) | < 5ms | Criterion benchmark, 100K nodes, range returning ~1% of data |
| Variable-length path 3-hop (p99) | < 20ms | Criterion benchmark, 1K nodes, 5K edges, 100 iterations |
| MERGE (p99) | < 5ms | Criterion benchmark, existing node match scenario |
| WITH pipeline (p99) | < 15ms | Criterion benchmark, MATCH + WITH + RETURN on 10K nodes |
| OPTIONAL MATCH (p99) | < 20ms | Criterion benchmark, 1K nodes, partial matches |

---

## 3. Quality Gates

### 3.1 Build and Test

| Gate | Criteria | Command |
|------|----------|---------|
| All tests pass | 100% pass | `cargo test --workspace` |
| Clippy zero warnings | zero warnings | `cargo clippy -- -D warnings` |
| Code coverage | 85% or above | `cargo tarpaulin` (Linux) or equivalent |
| Format check | pass | `cargo fmt --check` |

### 3.2 Binary Size

| Metric | Criteria |
|--------|----------|
| Full binary size (release) | < 50MB |

### 3.3 TRUST 5 Verification

| Pillar | Verification Items |
|--------|-------------------|
| **Tested** | 85%+ coverage; all ACs have corresponding tests; proptest for var-length paths and OPTIONAL MATCH |
| **Readable** | Clear naming; English code comments; MX tags on new operators and index module |
| **Unified** | `cargo fmt` passes; `cargo clippy` zero warnings |
| **Secured** | Input validation on UNWIND/variable-length limits; index operations within transaction boundaries |
| **Trackable** | Conventional commits; SPEC-DB-003 reference; changelog entries |

---

## 4. Definition of Done

SPEC-DB-003 is considered complete when all of the following conditions are met:

### Mandatory Conditions

- [ ] AC-020 ~ AC-032 (Phase 3a: WITH, UNWIND, OPTIONAL MATCH) all passing
- [ ] AC-033 ~ AC-043 (Phase 3b: MERGE, Indexing) all passing
- [ ] AC-044 ~ AC-051 (Phase 3c: Variable-length paths, Optimization) all passing
- [ ] `cargo test --workspace` 100% pass
- [ ] `cargo clippy -- -D warnings` zero warnings
- [ ] Code coverage 85% or above
- [ ] WITH clause: scope reset, projection, WHERE filtering, aggregation, DISTINCT all functional
- [ ] MERGE: create-if-not-exists, idempotency, ON MATCH/ON CREATE, relationship MERGE all functional
- [ ] OPTIONAL MATCH: NULL propagation, left join semantics, chained usage all functional
- [ ] UNWIND: literal lists, property lists, empty lists, NULL handling all functional
- [ ] Variable-length paths: bounded, unbounded with max cap, cycle detection, typed relationships all functional
- [ ] Property indexes: CREATE INDEX, DROP INDEX, index-assisted queries, auto-update on mutations all functional
- [ ] `StorageEngine::find_node()` and `find_edge()` APIs operational
- [ ] Performance gates met: index lookup p99 < 1ms, var-length 3-hop p99 < 20ms

### Optional Conditions

- [ ] Criterion benchmark suite for Phase 3 features
- [ ] proptest: random graph + OPTIONAL MATCH correctness
- [ ] proptest: variable-length path depth limit invariants
- [ ] Index scan selection demonstrates measurable speedup (AC-049)
- [ ] Constant folding and projection pruning operational

---

## 5. Test Scenarios (Detailed)

### 5.1 WITH Clause Tests

| Test ID | Scenario | Expected Result |
|---------|----------|-----------------|
| WITH-T001 | `MATCH (n) WITH n.name AS name RETURN name` | Projected column only |
| WITH-T002 | Access original variable after WITH | SemanticError |
| WITH-T003 | `WITH n, n.age AS age WHERE age > 25` | Filtered rows |
| WITH-T004 | `WITH n.city AS city, count(*) AS cnt` | Grouped aggregation |
| WITH-T005 | `WITH DISTINCT n.city AS city` | Deduplicated values |
| WITH-T006 | Multiple WITH clauses chained | Correct progressive scope narrowing |
| WITH-T007 | `WITH n ORDER BY n.age LIMIT 5` | ORDER BY + LIMIT within WITH scope |

### 5.2 UNWIND Tests

| Test ID | Scenario | Expected Result |
|---------|----------|-----------------|
| UNWIND-T001 | `UNWIND [1, 2, 3] AS x RETURN x` | 3 rows |
| UNWIND-T002 | `UNWIND [] AS x RETURN x` | 0 rows |
| UNWIND-T003 | `UNWIND null AS x RETURN x` | 0 rows (NULL -> empty) |
| UNWIND-T004 | `UNWIND n.list_prop AS x` | Rows per list element |
| UNWIND-T005 | `UNWIND n.age AS x` (non-list) | ExecutionError |
| UNWIND-T006 | `UNWIND [[1,2],[3,4]] AS x RETURN x` | 2 rows (nested lists) |
| UNWIND-T007 | UNWIND + WITH pipeline | Correct chaining |

### 5.3 OPTIONAL MATCH Tests

| Test ID | Scenario | Expected Result |
|---------|----------|-----------------|
| OPT-T001 | Node with no outgoing edges | NULL for optional variables |
| OPT-T002 | Node with multiple matches | One row per match |
| OPT-T003 | Chained OPTIONAL MATCH | Both levels can be NULL |
| OPT-T004 | OPTIONAL MATCH with WHERE | Filter applies to optional part only |
| OPT-T005 | NULL in aggregation (count) | count(b) excludes NULL rows |
| OPT-T006 | OPTIONAL MATCH same label, no edge | Source preserved, optional NULL |
| OPT-T007 | OPTIONAL MATCH + UNWIND combined | Correct pipeline interaction |

### 5.4 MERGE Tests

| Test ID | Scenario | Expected Result |
|---------|----------|-----------------|
| MERGE-T001 | MERGE on empty DB | Node created |
| MERGE-T002 | MERGE on existing node | Node matched, not duplicated |
| MERGE-T003 | MERGE twice (idempotency) | Still 1 node |
| MERGE-T004 | MERGE with ON CREATE SET | Properties set on creation only |
| MERGE-T005 | MERGE with ON MATCH SET | Properties set on match only |
| MERGE-T006 | MERGE with both ON MATCH + ON CREATE | Correct branch selection |
| MERGE-T007 | MERGE relationship | Edge created if not exists |
| MERGE-T008 | MERGE relationship idempotency | No duplicate edge |
| MERGE-T009 | MERGE with multiple properties | All properties used for matching |
| MERGE-T010 | MERGE with non-existent label | New label created via Catalog |

### 5.5 Variable-Length Path Tests

| Test ID | Scenario | Expected Result |
|---------|----------|-----------------|
| VLP-T001 | `[*1..3]` on linear chain | Correct depth-bounded results |
| VLP-T002 | `[*2]` exact hop | Only nodes at exact distance |
| VLP-T003 | `[*]` unbounded | Capped at default max_hops (10) |
| VLP-T004 | Cyclic graph | Terminates, no infinite loop |
| VLP-T005 | `[:TYPE*1..3]` typed | Only follows specified relationship type |
| VLP-T006 | `[*0..1]` zero-length | Includes start node |
| VLP-T007 | No matching paths | Empty result (no error) |
| VLP-T008 | Dense graph (high branching) | Completes within memory limits |
| VLP-T009 | `[*1..0]` invalid range | ParseError or SemanticError |
| VLP-T010 | `[*1..100]` exceeds max_hops config | Capped to config max, warning |

### 5.6 Index Tests

| Test ID | Scenario | Expected Result |
|---------|----------|-----------------|
| IDX-T001 | CREATE INDEX ON :Person(name) | Index created in Catalog |
| IDX-T002 | Query with index (equality) | IndexScan selected by planner |
| IDX-T003 | Query with index (range) | Range scan returns correct results |
| IDX-T004 | CREATE node -> index auto-update | New node findable via index |
| IDX-T005 | SET property -> index auto-update | Old value removed, new value indexed |
| IDX-T006 | DELETE node -> index auto-update | Deleted node removed from index |
| IDX-T007 | DROP INDEX | Index removed, falls back to full scan |
| IDX-T008 | Query on non-indexed property | Full scan (no error) |
| IDX-T009 | Multiple indexes on same label | Each index works independently |
| IDX-T010 | Index persistence across restart | Index definitions survive checkpoint/recovery |

### 5.7 Optimization Tests

| Test ID | Scenario | Expected Result |
|---------|----------|-----------------|
| OPT-T001 | LIMIT pushdown | Early termination in scan |
| OPT-T002 | Constant folding `1 + 2` | Plan contains literal 3 |
| OPT-T003 | Projection pruning | Unused columns removed early |
| OPT-T004 | Index scan vs full scan | Correct plan selection |
| OPT-T005 | MERGE with index | Uses index for match-check |

### 5.8 Integration Tests

| Test ID | Scenario | Expected Result |
|---------|----------|-----------------|
| INT-T010 | `MATCH...WITH...RETURN` end-to-end | Correct pipeline execution |
| INT-T011 | `MERGE + MATCH` verify idempotency | Data integrity maintained |
| INT-T012 | `OPTIONAL MATCH + aggregation` | Correct NULL handling in count/sum |
| INT-T013 | `UNWIND + CREATE` pattern | Nodes created from list |
| INT-T014 | Variable-length path + WHERE filter | Filter applied to path results |
| INT-T015 | `CREATE INDEX` + `MERGE` combo | MERGE uses index for matching |
| INT-T016 | Complex pipeline: `MATCH...WITH...UNWIND...OPTIONAL MATCH...RETURN` | All clauses compose correctly |

---

## 6. Verification Methods

| Verification Type | Tool | Target |
|-------------------|------|--------|
| Unit tests | `cargo test` | Each new operator, parser extension, semantic rule, planner rule |
| Integration tests | `cargo test --test '*'` | End-to-end query execution for all new clauses |
| Property-based tests | `proptest` | OPTIONAL MATCH on random graphs, var-length path invariants |
| Benchmarks | `criterion` | Index vs full scan, var-length scaling, MERGE performance |
| Static analysis | `cargo clippy` | Code quality, potential bugs |
| Format check | `cargo fmt --check` | Consistent code style |
| Coverage | `cargo tarpaulin` | 85% or above line coverage |

---

## 7. Traceability

| Requirement Area | Implementation Target | Test Target |
|-----------------|----------------------|-------------|
| WITH clause | `executor/operators/with.rs`, `semantic/mod.rs`, `planner/mod.rs` | WITH-T001 ~ T007, AC-020 ~ AC-023 |
| UNWIND clause | `executor/operators/unwind.rs`, `parser/clause.rs`, `parser/ast.rs`, `lexer/mod.rs` | UNWIND-T001 ~ T007, AC-024 ~ AC-028 |
| OPTIONAL MATCH | `executor/operators/optional_expand.rs`, `semantic/mod.rs`, `planner/mod.rs` | OPT-T001 ~ T007, AC-029 ~ AC-032 |
| MERGE clause | `executor/operators/merge.rs`, `parser/clause.rs`, `parser/ast.rs` | MERGE-T001 ~ T010, AC-033 ~ AC-039 |
| Variable-length paths | `executor/operators/var_length_expand.rs`, `parser/pattern.rs`, `parser/ast.rs` | VLP-T001 ~ T010, AC-044 ~ AC-048 |
| Property indexes | `index/mod.rs`, `index/btree_index.rs`, `catalog/mod.rs`, `lib.rs` | IDX-T001 ~ T010, AC-040 ~ AC-043 |
| Query optimization | `planner/optimize.rs`, `executor/operators/index_scan.rs` | OPT-T001 ~ T005, AC-049 ~ AC-051 |
| Storage APIs | `lib.rs` (find_node, find_edge) | MERGE-T*, IDX-T* |
