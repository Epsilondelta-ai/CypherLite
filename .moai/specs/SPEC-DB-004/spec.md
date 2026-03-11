---
id: SPEC-DB-004
version: "0.4.0"
status: draft
created: "2026-03-10"
updated: "2026-03-10"
author: epsilondelta
priority: P0
tags: [temporal, datetime, versioning, at-time, version-store, temporal-index]
lifecycle: spec-anchored
---

# SPEC-DB-004: CypherLite Phase 4 - Temporal Dimension (v0.4)

> CypherLite's temporal data layer. Build DateTime type support, automatic timestamp tracking, node/edge version storage, and temporal query syntax (AT TIME, BETWEEN TIME) on top of the Phase 3 query engine, enabling point-in-time and range-based historical graph queries.

---

## HISTORY

| Version | Date | Changes |
|---------|------|---------|
| 0.4.0 | 2026-03-10 | Initial SPEC creation based on research and master design document |

---

## 1. Environment

### 1.1 System Environment

- **Language**: Rust 1.84+ (Edition 2021)
- **MSRV**: 1.84 (consistent with Phase 1/2/3)
- **Target platforms**: Linux (x86_64), macOS (x86_64, aarch64), Windows (x86_64)
- **Execution model**: Synchronous, single-threaded, WASM-compatible (no `std::thread`)
- **WASM build target**: `wasm32-unknown-unknown`

### 1.2 Crate Structure

- **Extended crate**: `cypherlite-core` (`crates/cypherlite-core/`) -- PropertyValue::DateTime variant, temporal types
- **Extended crate**: `cypherlite-storage` (`crates/cypherlite-storage/`) -- VersionStore, timestamp tracking, temporal index
- **Extended crate**: `cypherlite-query` (`crates/cypherlite-query/`) -- AT TIME / BETWEEN TIME syntax, temporal operators
- **New external dependencies**: None (reuse existing workspace dependencies)

### 1.3 Dependency Graph (Phase 4 Additions)

```
cypherlite-query
  +-- cypherlite-core    (PropertyValue::DateTime, TemporalPredicate)
  +-- cypherlite-storage (VersionStore, temporal index APIs)
  +-- logos = "0.14"     (from Phase 2, retained)

cypherlite-storage
  +-- cypherlite-core    (PropertyValue::DateTime, NodeRecord, RelationshipRecord)
  +-- NEW: version/      (VersionStore module -- internal, no external deps)

cypherlite-core
  (no new external dependencies)
```

### 1.4 Phase 4 Crate Change Scope

```
crates/
  cypherlite-core/
    src/
      types.rs                  + PropertyValue::DateTime(i64) variant
                                + DateTime utility functions (now_millis, format)

  cypherlite-storage/
    src/
      version/                  [NEW] Version storage module
        mod.rs                  VersionStore struct, version chain operations
      btree/
        node_store.rs           + snapshot_before_update() hook
        edge_store.rs           + snapshot_before_update() hook
      page/
        mod.rs                  + DatabaseHeader temporal fields (version_store_root_page)
      index/
        mod.rs                  + temporal index on _created_at property
      lib.rs                    + get_node_at_time(), get_node_versions_between()
                                + automatic _created_at/_updated_at on mutations

  cypherlite-query/
    src/
      lexer/
        mod.rs                  + AT, TIME, BETWEEN, HISTORY tokens
      parser/
        ast.rs                  + TemporalPredicate enum (AsOf, Between)
                                + MatchClause.temporal_predicate field
      semantic/
        mod.rs                  + temporal predicate validation (DateTime type check)
      planner/
        mod.rs                  + AsOfScan, TemporalRangeScan logical plan operators
      executor/
        mod.rs                  + temporal scan execution (version chain traversal)
        value.rs                + Value::DateTime variant
```

### 1.5 Backward Compatibility

- Non-temporal databases (v0.3 format) shall continue to work without modification
- DatabaseHeader format_version bumps from 1 to 2
- Version store is allocated lazily on first temporal write
- Existing tests shall pass unchanged

---

## 2. Assumptions

### 2.1 Technical Assumptions

- A1: `i64` milliseconds since Unix epoch provides sufficient temporal resolution (range: +/- 292 million years)
- A2: Full-copy versioning (storing complete NodeRecord per version) is acceptable for v0.4; delta compression deferred to v0.5+
- A3: The existing `BTreeMap<PropertyValueKey, Vec<NodeId>>` index infrastructure supports DateTime keys without structural changes
- A4: Single-writer MVCC model means version chain mutations are serialized (no concurrent version writes)
- A5: System-managed timestamps (`_created_at`, `_updated_at`) use wall-clock time; no NTP synchronization guarantees

### 2.2 Scope Boundaries

- **In scope**: Transaction time (system-managed), point-in-time queries, range queries, version storage
- **Out of scope**: Valid time (user-defined bitemporal), HISTORY() function, anchor+delta compression, temporal relationship validity periods
- **Deferred to v0.5+**: Bitemporal queries, delta-based version compression, temporal path traversal, interval tree index

---

## 3. Requirements

### Group U: DateTime Foundation

#### U-001 PropertyValue::DateTime Variant [UBIQUITOUS]

The system shall support a `PropertyValue::DateTime(i64)` variant representing milliseconds since Unix epoch (1970-01-01T00:00:00Z).

**Acceptance Criteria**:
- DateTime values serialize/deserialize through bincode without breaking existing PropertyValue variants
- DateTime variant has discriminant tag 7 (following Bytes=5, Array=6)
- Negative values represent dates before epoch

#### U-002 datetime() Parser Function [EVENT-DRIVEN]

When a `datetime(string_literal)` function call is encountered in an expression, the system shall parse the string argument as an ISO 8601 datetime string and return a `PropertyValue::DateTime` value.

**Supported formats**:
- `datetime('2024-01-15')` -- date only, midnight UTC
- `datetime('2024-01-15T10:30:00')` -- date and time, UTC
- `datetime('2024-01-15T10:30:00Z')` -- explicit UTC
- `datetime('2024-01-15T10:30:00+09:00')` -- with timezone offset, converted to UTC millis

**Acceptance Criteria**:
- Invalid format produces `QueryError::InvalidDateTimeFormat` with the offending string
- Parsed result is i64 milliseconds since epoch

#### U-003 now() Function [EVENT-DRIVEN]

When a `now()` function call is encountered in an expression, the system shall return the current system time as `PropertyValue::DateTime`.

**Acceptance Criteria**:
- Returns current UTC milliseconds since epoch
- Consistent within a single query execution (captured at query start)

#### U-004 DateTime Comparison Operators [UBIQUITOUS]

The system shall support comparison operators (`<`, `<=`, `>`, `>=`, `=`, `<>`) between DateTime values in WHERE clauses and expressions.

**Acceptance Criteria**:
- DateTime comparisons follow numeric ordering of the underlying i64
- Comparing DateTime with non-DateTime types produces `TypeError`
- `PropertyValueKey` ordering includes DateTime for index range scans

#### U-005 DateTime Display Formatting [UBIQUITOUS]

The system shall format DateTime values as ISO 8601 strings (`YYYY-MM-DDTHH:MM:SS.sssZ`) in QueryResult output.

**Acceptance Criteria**:
- Display format is human-readable ISO 8601 UTC
- Debug format includes raw millisecond value

---

### Group V: Timestamp Tracking

#### V-001 Automatic _created_at on CREATE [EVENT-DRIVEN]

When a node or relationship is created via CREATE or MERGE (on create), the system shall automatically set a `_created_at` system property with the current DateTime value.

**Acceptance Criteria**:
- `_created_at` is stored as a regular property with name registered in the string catalog
- Value is captured at transaction commit time (consistent across all mutations in one transaction)
- Property is visible in RETURN clauses: `RETURN n._created_at`

#### V-002 Automatic _updated_at on SET [EVENT-DRIVEN]

When a node or relationship property is modified via SET, the system shall automatically set or update the `_updated_at` system property with the current DateTime value.

**Acceptance Criteria**:
- `_updated_at` is set on every SET operation
- On CREATE, `_updated_at` equals `_created_at`
- REMOVE of user properties still triggers `_updated_at` update

#### V-003 System Property Convention [UBIQUITOUS]

The system shall treat properties prefixed with `_` as system-managed properties.

**Acceptance Criteria**:
- User SET of `_created_at` or `_updated_at` produces `QueryError::SystemPropertyReadOnly`
- Other `_`-prefixed properties are reserved for future system use
- System properties are included in property serialization

#### V-004 Timestamp Opt-out [OPTIONAL]

Where temporal tracking is disabled via `DatabaseConfig::temporal_tracking_enabled = false`, the system shall skip automatic `_created_at` and `_updated_at` property injection.

**Acceptance Criteria**:
- Default configuration: temporal tracking enabled
- Disabling removes timestamp overhead from write path
- Existing timestamps remain readable but are not updated

---

### Group W: Version Storage

#### W-001 VersionStore Module [UBIQUITOUS]

The system shall provide a `VersionStore` that maintains a linked list of previous versions for each node and relationship.

**Acceptance Criteria**:
- VersionStore is a page-backed B-tree: `BTree<(EntityId, u64), VersionRecord>` where key is (entity_id, version_sequence_number)
- Each VersionRecord contains a full copy of NodeRecord or RelationshipRecord at the time of snapshot
- VersionStore root page is stored in DatabaseHeader (using available unused bytes)

#### W-002 Pre-Update Snapshot [EVENT-DRIVEN]

When a node or relationship is updated via SET or REMOVE, the system shall snapshot the current state into the VersionStore before applying changes.

**Acceptance Criteria**:
- Snapshot includes all properties, labels, and structural fields at the moment before mutation
- Snapshot includes the `_updated_at` timestamp of the previous version (serving as the version timestamp)
- Version sequence numbers are monotonically increasing per entity

#### W-003 Version Chain Structure [UBIQUITOUS]

The system shall maintain version chains as: current (live in NodeStore/EdgeStore) -> v(n) -> v(n-1) -> ... -> v(1) where v(1) is the earliest recorded version.

**Acceptance Criteria**:
- Current state is always in the primary store (no indirection for current reads)
- Version chain is traversable in reverse chronological order
- DELETE removes the current state but preserves version history

#### W-004 DatabaseHeader Extension [UBIQUITOUS]

The system shall extend DatabaseHeader with a `version_store_root_page: u64` field to locate the VersionStore B-tree.

**Acceptance Criteria**:
- Field occupies bytes 36-43 of the header page (immediately after existing fields)
- Value of 0 indicates no version store allocated (backward compatibility)
- `format_version` field bumps from 1 to 2

#### W-005 Version Storage Opt-out [OPTIONAL]

Where version storage is disabled via `DatabaseConfig::version_storage_enabled = false`, the system shall skip pre-update snapshots.

**Acceptance Criteria**:
- Default configuration: version storage enabled (when temporal tracking is enabled)
- Disabling reduces write amplification
- AT TIME queries return error when version storage is disabled

---

### Group X: AT TIME Query Syntax

#### X-001 Lexer Tokens [UBIQUITOUS]

The system shall recognize `AT`, `TIME`, `BETWEEN`, and `HISTORY` as reserved keyword tokens in the lexer.

**Acceptance Criteria**:
- Tokens are case-insensitive: `AT TIME`, `at time`, `At Time` all valid
- Tokens do not conflict with existing identifiers (AT, TIME are new reserved words)

#### X-002 AT TIME Parser Rule [EVENT-DRIVEN]

When `AT TIME <expression>` follows a MATCH pattern, the parser shall produce a `TemporalPredicate::AsOf(Expression)` node attached to the MatchClause.

**Grammar**:
```
match_clause ::= MATCH pattern_list [AT TIME expression] [WHERE expression]
```

**Acceptance Criteria**:
- `MATCH (n:Person) AT TIME datetime('2024-01-15') RETURN n` parses successfully
- `AT TIME` must appear after pattern and before WHERE
- Expression must be a single expression (not a list)

#### X-003 TemporalPredicate AST Node [UBIQUITOUS]

The system shall define a `TemporalPredicate` enum with variants:
- `AsOf(Expression)` -- point-in-time query
- `Between(Expression, Expression)` -- range query

**Acceptance Criteria**:
- TemporalPredicate is `Option<TemporalPredicate>` on MatchClause
- None means current (non-temporal) query

#### X-004 Semantic Validation [EVENT-DRIVEN]

When a TemporalPredicate is present, the semantic analyzer shall validate that the expression(s) evaluate to DateTime type.

**Acceptance Criteria**:
- Non-DateTime temporal expression produces `SemanticError::TemporalExpressionTypeMismatch`
- Literal integers are not auto-coerced to DateTime (explicit `datetime()` call required)

#### X-005 AsOfScan Logical Plan Operator [EVENT-DRIVEN]

When a MatchClause contains `TemporalPredicate::AsOf`, the planner shall produce an `AsOfScan { timestamp: Expression, child: LogicalPlan }` operator.

**Acceptance Criteria**:
- AsOfScan wraps the node/edge scan operator
- Timestamp expression is evaluated once at execution start
- Planner falls back to full version chain scan when no temporal index exists

#### X-006 AsOfScan Executor [EVENT-DRIVEN]

When the executor encounters an AsOfScan operator, the system shall:
1. Evaluate the timestamp expression to a DateTime value
2. For each entity from the child scan, check if `_created_at <= timestamp`
3. If the current version's `_created_at <= timestamp` and (`_updated_at` is NULL or `_updated_at > timestamp`), return current version
4. Otherwise, traverse the version chain to find the version valid at the given timestamp
5. If no version exists at the timestamp, exclude the entity from results

**Acceptance Criteria**:
- Point-in-time query returns exactly the state as it was at the specified time
- Entities created after the timestamp are excluded
- Entities deleted before the timestamp are excluded
- Version chain traversal stops at the first matching version (most recent <= timestamp)

---

### Group Y: Temporal Range Queries

#### Y-001 BETWEEN TIME Parser Rule [EVENT-DRIVEN]

When `BETWEEN TIME <expr1> AND <expr2>` follows a MATCH pattern, the parser shall produce a `TemporalPredicate::Between(Expression, Expression)` node.

**Grammar**:
```
match_clause ::= MATCH pattern_list [BETWEEN TIME expression AND expression] [WHERE expression]
```

**Acceptance Criteria**:
- `MATCH (n) BETWEEN TIME datetime('2024-01-01') AND datetime('2024-12-31') RETURN n` parses successfully
- Start time must be <= end time (validated at execution time, not parse time)

#### Y-002 TemporalRangeScan Executor [EVENT-DRIVEN]

When the executor encounters a TemporalRangeScan operator, the system shall return all versions of matching entities that were valid within the specified time range.

**Acceptance Criteria**:
- A version is "valid within range" if its `_created_at <= range_end` AND (`_updated_at` is NULL or `_updated_at >= range_start`)
- Multiple versions of the same entity may appear in results (one per version within range)
- Each result row includes the version's `_created_at` and `_updated_at` timestamps
- Empty range (start > end) produces `QueryError::InvalidTemporalRange`

#### Y-003 Temporal Index [EVENT-DRIVEN]

When temporal tracking is enabled, the system shall maintain a property index on `_created_at` for efficient temporal range lookups.

**Acceptance Criteria**:
- Index is automatically created on database open (not via CREATE INDEX)
- Index uses existing PropertyIndex infrastructure with `BTreeMap<PropertyValueKey, Vec<NodeId>>`
- AsOfScan and TemporalRangeScan use this index for initial candidate filtering
- Index is updated on every CREATE and SET operation

---

### Group Z: Quality Finalization

#### Z-001 Clippy Clean [UBIQUITOUS]

The system shall produce zero clippy warnings across the entire workspace with `cargo clippy --workspace --all-targets -- -D warnings`.

#### Z-002 Test Coverage [UBIQUITOUS]

The system shall maintain minimum 85% test coverage across all crates with temporal functionality.

**Acceptance Criteria**:
- Unit tests for each requirement (U-001 through Y-003)
- Integration tests for end-to-end temporal query workflows
- Edge case tests: NULL datetime, epoch boundaries, empty version chains

#### Z-003 Property-Based Tests [EVENT-DRIVEN]

When proptest is available, the system shall include property-based tests for temporal invariants.

**Invariants**:
- Version chain timestamps are strictly monotonically ordered
- Version count equals number of SET operations on an entity
- AT TIME query result matches manual version chain traversal
- Serialization round-trip preserves DateTime values exactly

#### Z-004 Benchmark Suite [EVENT-DRIVEN]

When criterion is available, the system shall include benchmarks measuring temporal overhead.

**Benchmarks**:
- CREATE with vs without temporal tracking (overhead measurement)
- SET with version snapshot (write amplification measurement)
- AT TIME query latency vs current-state query latency
- Version chain traversal: 1, 10, 100, 1000 versions

#### Z-005 Version Bump [UBIQUITOUS]

The system shall update all crate versions to `0.4.0` in workspace Cargo.toml upon completion.

---

## 4. Specifications

### 4.1 Data Model

```
PropertyValue (extended)
  +-- Null        (tag 0)
  +-- Bool        (tag 1)
  +-- Int64       (tag 2)
  +-- Float64     (tag 3)
  +-- String      (tag 4)
  +-- Bytes       (tag 5)
  +-- Array       (tag 6)
  +-- DateTime    (tag 7)  [NEW] i64 millis since epoch
```

### 4.2 Version Chain Model

```
NodeStore (current state)
  node_id=42 -> NodeRecord { labels, properties: [..., _created_at, _updated_at] }

VersionStore (historical versions)
  (42, 3) -> NodeRecord snapshot at version 3
  (42, 2) -> NodeRecord snapshot at version 2
  (42, 1) -> NodeRecord snapshot at version 1 (original CREATE state)
```

### 4.3 Temporal Query Execution Model

```
MATCH (n:Person) AT TIME datetime('2024-06-15') RETURN n

1. Lexer:  [..., AT, TIME, FunctionCall("datetime", "2024-06-15"), ...]
2. Parser: MatchClause { pattern, temporal: Some(AsOf(datetime('2024-06-15'))), where: None }
3. Semantic: Validate datetime expression -> DateTime type
4. Planner: AsOfScan(timestamp=2024-06-15, NodeScan(label=Person))
5. Executor:
   a. Evaluate timestamp -> 1718409600000 (millis)
   b. Scan Person nodes
   c. For each node:
      - If _created_at <= timestamp AND (_updated_at IS NULL OR _updated_at > timestamp): return current
      - Else: traverse version chain, return version where _created_at <= timestamp < next_version._created_at
      - If no version found: skip entity
```

### 4.4 DatabaseHeader Layout (v2)

```
Offset  Size   Field
0       4      magic (0x43594C54 = "CYLT")
4       4      format_version (2)  [CHANGED from 1]
8       4      page_count
12      8      root_node_page
20      8      root_edge_page
28      4      next_node_id
32      4      next_edge_id
36      8      version_store_root_page  [NEW]
44      4052   reserved
```

---

## 5. Traceability

| Requirement | Group | EARS Pattern | Test Coverage |
|-------------|-------|-------------|---------------|
| U-001 | DateTime Foundation | Ubiquitous | Unit: serialization round-trip |
| U-002 | DateTime Foundation | Event-driven | Unit: format parsing, error cases |
| U-003 | DateTime Foundation | Event-driven | Unit: now() returns valid DateTime |
| U-004 | DateTime Foundation | Ubiquitous | Unit: comparison operators |
| U-005 | DateTime Foundation | Ubiquitous | Unit: display formatting |
| V-001 | Timestamp Tracking | Event-driven | Integration: CREATE sets _created_at |
| V-002 | Timestamp Tracking | Event-driven | Integration: SET updates _updated_at |
| V-003 | Timestamp Tracking | Ubiquitous | Unit: system property write rejection |
| V-004 | Timestamp Tracking | Optional | Integration: opt-out skips timestamps |
| W-001 | Version Storage | Ubiquitous | Unit: VersionStore CRUD |
| W-002 | Version Storage | Event-driven | Integration: SET triggers snapshot |
| W-003 | Version Storage | Ubiquitous | Unit: version chain traversal |
| W-004 | Version Storage | Ubiquitous | Unit: header serialization |
| W-005 | Version Storage | Optional | Integration: opt-out skips versions |
| X-001 | AT TIME Syntax | Ubiquitous | Unit: lexer token recognition |
| X-002 | AT TIME Syntax | Event-driven | Unit: parser produces correct AST |
| X-003 | AT TIME Syntax | Ubiquitous | Unit: TemporalPredicate variants |
| X-004 | AT TIME Syntax | Event-driven | Unit: semantic validation errors |
| X-005 | AT TIME Syntax | Event-driven | Unit: planner produces AsOfScan |
| X-006 | AT TIME Syntax | Event-driven | Integration: end-to-end AT TIME query |
| Y-001 | Range Queries | Event-driven | Unit: BETWEEN TIME parsing |
| Y-002 | Range Queries | Event-driven | Integration: range query results |
| Y-003 | Range Queries | Event-driven | Integration: temporal index usage |
| Z-001 | Quality | Ubiquitous | CI: clippy check |
| Z-002 | Quality | Ubiquitous | CI: coverage report |
| Z-003 | Quality | Event-driven | CI: proptest suite |
| Z-004 | Quality | Event-driven | CI: criterion benchmarks |
| Z-005 | Quality | Ubiquitous | CI: version check |
