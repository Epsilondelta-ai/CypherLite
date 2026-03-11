---
id: SPEC-DB-004
type: acceptance
version: "0.4.0"
status: draft
created: "2026-03-10"
updated: "2026-03-10"
author: epsilondelta
priority: P0
tags: [temporal, datetime, versioning, at-time, version-store, temporal-index]
---

# SPEC-DB-004 Acceptance Criteria: CypherLite Phase 4 - Temporal Dimension

---

## HISTORY

| Version | Date | Changes |
|---------|------|---------|
| 0.4.0 | 2026-03-10 | Initial acceptance criteria based on research analysis |

---

## 1. Functional Acceptance Criteria

### Group U: DateTime Foundation

#### AC-U01: PropertyValue::DateTime Serialization Round-Trip

```gherkin
Given: A PropertyValue::DateTime(1705276800000) representing 2024-01-15T00:00:00Z
When: The value is serialized via bincode and deserialized
Then: The deserialized value equals PropertyValue::DateTime(1705276800000)
  And: The discriminant tag is 7
  And: Existing PropertyValue variants (Null, Bool, Int64, Float64, String, Bytes, Array) are unaffected
```

#### AC-U02: datetime() Function -- Valid Formats

```gherkin
Given: A query engine with datetime() function registered
When: The following expressions are evaluated:
  | Expression                                  | Expected millis     |
  | datetime('2024-01-15')                      | 1705276800000       |
  | datetime('2024-01-15T10:30:00')             | 1705314600000       |
  | datetime('2024-01-15T10:30:00Z')            | 1705314600000       |
  | datetime('2024-01-15T10:30:00+09:00')       | 1705282200000       |
Then: Each returns the corresponding PropertyValue::DateTime with expected milliseconds
```

#### AC-U03: datetime() Function -- Invalid Format

```gherkin
Given: A query engine with datetime() function registered
When: datetime('not-a-date') is evaluated
Then: A QueryError::InvalidDateTimeFormat error is returned
  And: The error message contains "not-a-date"
```

#### AC-U04: DateTime Comparison Operators

```gherkin
Given: Two DateTime values:
  a = datetime('2024-01-15')  (1705276800000)
  b = datetime('2024-06-15')  (1718409600000)
When: Comparison operations are performed
Then: a < b is true
  And: a > b is false
  And: a = a is true
  And: a <> b is true
  And: a <= b is true
  And: b >= a is true
```

#### AC-U05: DateTime Type Mismatch

```gherkin
Given: A DateTime value a = datetime('2024-01-15') and an Int64 value b = 42
When: a < b comparison is evaluated
Then: A TypeError is returned indicating incompatible types for comparison
```

#### AC-U06: DateTime Display Format

```gherkin
Given: A PropertyValue::DateTime(1705314600000)
When: The value is formatted for QueryResult display
Then: The output string is "2024-01-15T10:30:00.000Z"
```

#### AC-U07: now() Function

```gherkin
Given: A query engine with now() function registered
When: RETURN now() AS t is executed
Then: t is a DateTime value
  And: t represents a time within 1 second of actual system time
```

#### AC-U08: now() Consistency Within Query

```gherkin
Given: A database with 1000 Person nodes
When: MATCH (n:Person) RETURN now() AS t is executed
Then: All 1000 rows return the same DateTime value for t
```

---

### Group V: Timestamp Tracking

#### AC-V01: Automatic _created_at on Node CREATE

```gherkin
Given: An empty database with temporal tracking enabled
When: CREATE (n:Person {name: 'Alice'}) is executed
  And: MATCH (n:Person) RETURN n._created_at AS ts is executed
Then: ts is a DateTime value
  And: ts represents a time within 1 second of the CREATE execution time
```

#### AC-V02: Automatic _updated_at on SET

```gherkin
Given: A database with node Person {name: 'Alice'} created at time T1
When: 100ms elapses
  And: MATCH (n:Person {name: 'Alice'}) SET n.age = 30 is executed
  And: MATCH (n:Person {name: 'Alice'}) RETURN n._created_at AS c, n._updated_at AS u is executed
Then: c equals T1 (unchanged)
  And: u > c (updated after creation)
  And: u represents a time within 1 second of the SET execution time
```

#### AC-V03: _created_at Equals _updated_at on Initial CREATE

```gherkin
Given: An empty database with temporal tracking enabled
When: CREATE (n:Person {name: 'Alice'}) is executed
  And: MATCH (n:Person) RETURN n._created_at AS c, n._updated_at AS u is executed
Then: c = u (both set to the same transaction timestamp)
```

#### AC-V04: System Property Write Protection

```gherkin
Given: A database with node Person {name: 'Alice'}
When: MATCH (n:Person) SET n._created_at = datetime('2020-01-01') is executed
Then: A QueryError::SystemPropertyReadOnly error is returned
  And: The original _created_at value is unchanged
```

#### AC-V05: Timestamp Tracking Opt-out

```gherkin
Given: A database opened with DatabaseConfig { temporal_tracking_enabled: false }
When: CREATE (n:Person {name: 'Alice'}) is executed
  And: MATCH (n:Person) RETURN n._created_at AS ts is executed
Then: ts is NULL (no automatic timestamp injection)
```

#### AC-V06: Relationship Timestamp Tracking

```gherkin
Given: An empty database with temporal tracking enabled
When: CREATE (a:Person {name: 'Alice'})-[r:KNOWS]->(b:Person {name: 'Bob'}) is executed
  And: MATCH ()-[r:KNOWS]->() RETURN r._created_at AS ts is executed
Then: ts is a DateTime value (relationships also get automatic timestamps)
```

---

### Group W: Version Storage

#### AC-W01: Version Created on SET

```gherkin
Given: A database with version storage enabled
  And: A node Person {name: 'Alice'} created at time T1
When: MATCH (n:Person {name: 'Alice'}) SET n.name = 'Alice Smith' is executed at time T2
  And: MATCH (n:Person {name: 'Alice Smith'}) SET n.age = 30 is executed at time T3
Then: The node has 2 versions in the VersionStore
  And: Version 1 contains {name: 'Alice', _created_at: T1, _updated_at: T1}
  And: Version 2 contains {name: 'Alice Smith', _created_at: T1, _updated_at: T2}
  And: Current state contains {name: 'Alice Smith', age: 30, _created_at: T1, _updated_at: T3}
```

#### AC-W02: Version Chain Ordering

```gherkin
Given: A node that has been updated 5 times (creating 5 versions in VersionStore)
When: The version chain is traversed
Then: Version timestamps are in strictly increasing chronological order
  And: Version sequence numbers are monotonically increasing (1, 2, 3, 4, 5)
```

#### AC-W03: DatabaseHeader v2 Compatibility

```gherkin
Given: A database file created with format_version = 1 (v0.3)
When: The database is opened with v0.4 code
Then: The database opens successfully
  And: format_version is upgraded to 2
  And: version_store_root_page is initialized to 0 (no versions yet)
  And: All existing data is readable and writable
```

#### AC-W04: Version Storage After DELETE

```gherkin
Given: A database with node Person {name: 'Alice'} that has 3 versions
When: MATCH (n:Person {name: 'Alice'}) DELETE n is executed
Then: The node is removed from NodeStore (current state)
  And: The 3 historical versions remain in VersionStore
  And: AT TIME queries can still find the deleted node at historical timestamps
```

#### AC-W05: Empty Version Chain for New Node

```gherkin
Given: A database with version storage enabled
When: CREATE (n:Person {name: 'Alice'}) is executed (no subsequent SET)
Then: The VersionStore has 0 versions for this node
  And: The node is only in the primary NodeStore
```

#### AC-W06: Version Storage Opt-out

```gherkin
Given: A database opened with DatabaseConfig { version_storage_enabled: false }
When: CREATE (n:Person {name: 'Alice'}) is executed
  And: MATCH (n:Person) SET n.name = 'Bob' is executed
Then: No versions are stored in the VersionStore
  And: The current state reflects the latest SET (name: 'Bob')
```

---

### Group X: AT TIME Query Syntax

#### AC-X01: Lexer Token Recognition

```gherkin
Given: The input string "MATCH (n) AT TIME datetime('2024-01-15') RETURN n"
When: The lexer tokenizes the input
Then: The token stream includes Token::At followed by Token::Time
  And: Case variations "at time", "At Time", "AT TIME" all produce the same tokens
```

#### AC-X02: AT TIME Parser -- Valid Syntax

```gherkin
Given: The query "MATCH (n:Person) AT TIME datetime('2024-01-15') RETURN n"
When: The parser processes the query
Then: The AST contains a MatchClause with:
  - pattern: NodePattern(variable: "n", labels: ["Person"])
  - temporal_predicate: Some(AsOf(FunctionCall("datetime", ["2024-01-15"])))
  - where_clause: None
```

#### AC-X03: AT TIME Parser -- With WHERE

```gherkin
Given: The query "MATCH (n:Person) AT TIME datetime('2024-01-15') WHERE n.age > 25 RETURN n"
When: The parser processes the query
Then: The AST contains temporal_predicate AND where_clause both populated
  And: AT TIME appears before WHERE in the AST
```

#### AC-X04: Semantic Validation -- Non-DateTime Expression

```gherkin
Given: The query "MATCH (n:Person) AT TIME 42 RETURN n"
When: Semantic analysis is performed
Then: A SemanticError::TemporalExpressionTypeMismatch is returned
  And: The error indicates that integer 42 cannot be used as a temporal predicate
```

#### AC-X05: AT TIME End-to-End Query -- Point in Time

```gherkin
Given: A database with temporal tracking and version storage enabled
  And: At time T1: CREATE (n:Person {name: 'Alice', age: 25})
  And: At time T2 (T2 > T1): MATCH (n:Person) SET n.age = 30
  And: At time T3 (T3 > T2): MATCH (n:Person) SET n.age = 35
When: MATCH (n:Person) AT TIME <T1+1ms> RETURN n.name, n.age is executed
Then: Result contains 1 row: {name: 'Alice', age: 25}

When: MATCH (n:Person) AT TIME <T2+1ms> RETURN n.name, n.age is executed
Then: Result contains 1 row: {name: 'Alice', age: 30}

When: MATCH (n:Person) RETURN n.name, n.age is executed (no AT TIME, current state)
Then: Result contains 1 row: {name: 'Alice', age: 35}
```

#### AC-X06: AT TIME -- Entity Created After Timestamp

```gherkin
Given: At time T1: CREATE (a:Person {name: 'Alice'})
  And: At time T2 (T2 > T1): CREATE (b:Person {name: 'Bob'})
When: MATCH (n:Person) AT TIME <T1+1ms> RETURN n.name is executed
Then: Result contains only {name: 'Alice'}
  And: Bob is excluded (created after the queried timestamp)
```

#### AC-X07: AT TIME -- Before Any Entity Exists

```gherkin
Given: At time T1: CREATE (n:Person {name: 'Alice'})
When: MATCH (n:Person) AT TIME datetime('1970-01-01') RETURN n is executed
Then: Result is empty (no entities existed at epoch)
```

---

### Group Y: Temporal Range Queries

#### AC-Y01: BETWEEN TIME Parser -- Valid Syntax

```gherkin
Given: The query "MATCH (n:Person) BETWEEN TIME datetime('2024-01-01') AND datetime('2024-12-31') RETURN n"
When: The parser processes the query
Then: The AST contains temporal_predicate: Some(Between(
  FunctionCall("datetime", ["2024-01-01"]),
  FunctionCall("datetime", ["2024-12-31"])
))
```

#### AC-Y02: BETWEEN TIME -- Multiple Versions Returned

```gherkin
Given: A database with temporal tracking and version storage enabled
  And: At time T1: CREATE (n:Person {name: 'v1'})
  And: At time T2: SET n.name = 'v2'
  And: At time T3: SET n.name = 'v3'
  And: At time T4: SET n.name = 'v4'
When: MATCH (n:Person) BETWEEN TIME <T1> AND <T4> RETURN n.name is executed
Then: Result contains multiple rows representing versions valid within the range
  And: All versions from v1 through v4 are included
```

#### AC-Y03: BETWEEN TIME -- Partial Overlap

```gherkin
Given: A node with versions at times T1, T2, T3, T4, T5
When: MATCH (n) BETWEEN TIME <T2> AND <T4> RETURN n is executed
Then: Only versions valid during [T2, T4] are returned
  And: Version at T1 is excluded if its _updated_at < T2
  And: Version at T5 is excluded if its _created_at > T4
```

#### AC-Y04: BETWEEN TIME -- Invalid Range

```gherkin
Given: A temporal query with start time after end time
When: MATCH (n) BETWEEN TIME datetime('2024-12-31') AND datetime('2024-01-01') RETURN n is executed
Then: A QueryError::InvalidTemporalRange error is returned
```

#### AC-Y05: Temporal Index Usage

```gherkin
Given: A database with 10,000 Person nodes created at different timestamps
  And: Temporal index on _created_at is active
When: MATCH (n:Person) AT TIME datetime('2024-06-15') RETURN n is executed
Then: The query uses the temporal index for candidate filtering
  And: Query execution time is sub-linear relative to total node count
```

---

### Group Z: Quality Finalization

#### AC-Z01: Clippy Clean

```gherkin
Given: The complete CypherLite workspace after all Phase 4 changes
When: cargo clippy --workspace --all-targets -- -D warnings is executed
Then: The command exits with status 0
  And: No warnings or errors are produced
```

#### AC-Z02: Test Suite Passes

```gherkin
Given: The complete CypherLite workspace after all Phase 4 changes
When: cargo test --workspace is executed
Then: All tests pass (including all Phase 1/2/3 tests)
  And: No test regressions from previous phases
```

#### AC-Z03: Backward Compatibility

```gherkin
Given: A database file created with v0.3.0 containing nodes and relationships
When: The database is opened with v0.4.0 code
Then: All existing nodes and relationships are readable
  And: All existing queries (MATCH, CREATE, SET, DELETE, WITH, MERGE, etc.) work unchanged
  And: The database can be written to (new temporal features are additive)
```

#### AC-Z04: Proptest -- Version Chain Integrity

```gherkin
Given: An arbitrary sequence of CREATE and SET operations (generated by proptest)
When: The operations are applied to a database
Then: The version count for each entity equals the number of SET operations on that entity
  And: Version timestamps are strictly monotonically increasing per entity
  And: AT TIME query for any recorded timestamp returns the correct version
```

#### AC-Z05: Proptest -- DateTime Serialization

```gherkin
Given: An arbitrary i64 value (generated by proptest)
When: PropertyValue::DateTime(value) is serialized and deserialized
Then: The round-trip produces an identical value
  And: The discriminant tag is preserved as 7
```

---

## 2. Edge Case Scenarios

### EC-01: NULL DateTime Handling

```gherkin
Given: A query WHERE n._created_at > NULL
When: The comparison is evaluated
Then: The result is NULL (three-valued logic)
  And: The row is excluded from results (WHERE NULL = false)
```

### EC-02: Epoch Boundary

```gherkin
Given: PropertyValue::DateTime(0) representing 1970-01-01T00:00:00Z
When: The value is formatted for display
Then: The output is "1970-01-01T00:00:00.000Z"
  And: Comparisons with negative values (before epoch) work correctly
```

### EC-03: Negative DateTime (Before Epoch)

```gherkin
Given: datetime('1969-12-31T23:59:59Z') is evaluated
When: The millisecond value is computed
Then: The result is PropertyValue::DateTime(-1000)
  And: The value serializes and deserializes correctly
```

### EC-04: Empty Version Chain with AT TIME

```gherkin
Given: A node created at T1 that has never been updated (no versions in VersionStore)
When: MATCH (n) AT TIME <T1+1ms> RETURN n is executed
Then: The current state is returned (no version chain traversal needed)
```

### EC-05: AT TIME Exactly at Version Boundary

```gherkin
Given: A node created at T1, updated at T2
When: MATCH (n) AT TIME <T2> RETURN n is executed
Then: The version valid at T2 is returned (boundary is inclusive of the update)
```

### EC-06: Large Version Chain (1000 versions)

```gherkin
Given: A node that has been updated 1000 times
When: AT TIME query for the 500th version timestamp is executed
Then: The correct version is returned
  And: Query completes within reasonable time (< 100ms)
```

---

## 3. Performance Criteria

| Metric | Target | Measurement Method |
|--------|--------|--------------------|
| CREATE overhead with temporal tracking | < 20% vs without tracking | Criterion benchmark: create_with_temporal vs create_without |
| SET overhead with version snapshot | < 50% vs without versioning | Criterion benchmark: set_with_version vs set_without |
| AT TIME query (10 versions) | < 2x current-state query latency | Criterion benchmark: at_time_10v vs match_current |
| AT TIME query (100 versions) | < 5x current-state query latency | Criterion benchmark: at_time_100v vs match_current |
| AT TIME query (1000 versions) | < 10x current-state query latency | Criterion benchmark: at_time_1000v vs match_current |
| datetime() parsing | < 1 microsecond per parse | Criterion benchmark: datetime_parse |
| BETWEEN TIME (10K nodes, 100 in range) | < 10ms | Criterion benchmark: between_time_10k |
| Storage overhead per version | ~= sizeof(NodeRecord) | Manual calculation from benchmark data |

---

## 4. Quality Gate Criteria

| Gate | Requirement | Tool |
|------|-------------|------|
| Compilation | Zero errors, zero warnings | `cargo build --workspace` |
| Clippy | Zero warnings with `-D warnings` | `cargo clippy --workspace --all-targets` |
| Unit tests | All pass | `cargo test --workspace` |
| Test coverage | >= 85% for temporal modules | `cargo-tarpaulin` or `cargo-llvm-cov` |
| Backward compatibility | v0.3 database opens and works in v0.4 | Integration test |
| No regressions | All Phase 1/2/3 tests pass unchanged | `cargo test --workspace` |
| Proptest | Temporal invariants hold for 256+ cases | `proptest` in test suite |
| Benchmarks | Temporal overhead within targets | `cargo bench` with criterion |

---

## 5. Definition of Done

- [ ] All requirements U-001 through Z-005 implemented
- [ ] All acceptance criteria AC-U01 through AC-Z05 verified
- [ ] Edge cases EC-01 through EC-06 tested
- [ ] Performance benchmarks within target thresholds
- [ ] Quality gates all pass
- [ ] Workspace version bumped to 0.4.0
- [ ] Backward compatibility with v0.3 databases confirmed
- [ ] Zero clippy warnings
- [ ] No Phase 1/2/3 test regressions
