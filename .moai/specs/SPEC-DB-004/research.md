# SPEC-DB-004 Research: Temporal Dimension

## 1. Storage Layer Deep Dive

### 1.1 Core Data Structures

**NodeRecord** (`cypherlite-core/src/types.rs:48-60`):
- Fields: `node_id`, `labels` (Vec<u32>), `properties` (Vec<(u32, PropertyValue)>), `next_edge_id`, `overflow_page`
- Serialization: bincode
- No timestamp fields currently

**RelationshipRecord** (`cypherlite-core/src/types.rs:64-81`):
- Fields: `edge_id`, `start_node`, `end_node`, `rel_type_id`, `direction`, `next_out_edge`, `next_in_edge`, `properties`
- No timestamp fields currently

**PropertyValue Enum** (`cypherlite-core/src/types.rs:19-34`):
- Variants: Null(0), Bool(1), Int64(2), Float64(3), String(4), Bytes(5), Array(6)
- **No DateTime variant** — primary extension point for temporal support

### 1.2 B-Tree Storage

**NodeStore** (`cypherlite-storage/src/btree/node_store.rs`):
- `BTree<u64, NodeRecord>` structure
- Sequential NodeId allocation from `next_id` counter
- CRUD operations: O(log n)

**EdgeStore** (`cypherlite-storage/src/btree/edge_store.rs`):
- Similar structure, maintains adjacency chains via `next_out_edge`/`next_in_edge` pointers

### 1.3 Database Header

**DatabaseHeader** (`cypherlite-storage/src/page/mod.rs:112-127`):
- Stored at page 0, 36 bytes used out of 4096
- Fields: magic, version, page_count, root_node_page, root_edge_page, next_node_id, next_edge_id
- **4060 unused bytes** available for temporal metadata

### 1.4 WAL Structure

**WalFrame** (`cypherlite-storage/src/wal/mod.rs:94-107`):
- `frame_number` (u64) as monotonic sequence — only temporal marker
- Frame size: 32 bytes header + 4096 bytes page data
- Checksum validation for integrity

### 1.5 MVCC Implementation

**TransactionManager** (`cypherlite-storage/src/transaction/mvcc.rs:10-92`):
- `snapshot_frame` (u64) captures WAL frame as version number
- Snapshot isolation via frame number
- Exclusive write lock
- **Key pattern**: Frame number acts as logical timestamp — extendable to hybrid timestamp

### 1.6 Index System

**PropertyIndex** (`cypherlite-storage/src/index/mod.rs:107-158`):
- BTreeMap<PropertyValueKey, Vec<NodeId>>
- Supports exact match AND range queries
- PropertyValueKey provides total ordering
- **Directly applicable** for temporal range queries

---

## 2. Query Layer Deep Dive

### 2.1 AST

**Clause Enum** (`cypherlite-query/src/parser/ast.rs:10-23`):
- Match, Return, Create, Set, Remove, Delete, With, Merge, Unwind, CreateIndex, DropIndex
- No temporal clause yet

**MatchClause** (`ast.rs:26-30`):
- `optional`, `pattern`, `where_clause`
- Integration point: add `temporal_predicate: Option<TemporalPredicate>`

**Expression Enum** (`ast.rs:179-199`):
- Supports FunctionCall — temporal functions (datetime(), now()) can use this

### 2.2 Lexer

**Token Enum** (`cypherlite-query/src/lexer/mod.rs:50-175`):
- Keywords with priority system
- New tokens needed: AT, TIME, BETWEEN, DATETIME, HISTORY

### 2.3 Logical Planner

**LogicalPlan** (`cypherlite-query/src/planner/mod.rs:9-141`):
- 20+ operator types including IndexScan
- Temporal operators needed: AsOfScan, TemporalRangeScan, AllVersionsScan

### 2.4 Executor (Volcano/Iterator Model)

**Execute dispatch** (`cypherlite-query/src/executor/mod.rs:85-89`):
- Pattern matching on LogicalPlan variants
- Records = HashMap<String, Value>
- **Value enum** (`executor/mod.rs:13-23`): No temporal value type yet

---

## 3. Design Document Analysis

### 3.1 Proposed Temporal Model (from docs/research/02_cypher_rdf_temporal.md)

**Bitemporal Model**:
- Valid Time (VT): When fact is true in reality (user-defined)
- Transaction Time (TT): When fact was recorded (system-managed)
- Enables both "as-was" and "as-recorded" queries

### 3.2 Version Storage Strategy

**Anchor+Delta** (selected in design docs):
- Full snapshot (anchor) stored periodically
- Deltas between versions for efficiency
- Bounded reconstruction time
- Reference: AeonG academic system

### 3.3 Proposed Cypher Syntax

```cypher
-- Point-in-time
MATCH (n:Person) AT TIME datetime('2024-01-15') RETURN n
-- Range
MATCH (n:Person) BETWEEN TIME '2024-01-01' AND '2024-12-31' RETURN n
-- History
MATCH (n:Person {id: 123}) RETURN HISTORY(n)
```

### 3.4 Temporal Index

- B+ Tree on (EntityID, Timestamp) as primary
- Interval Tree as future optimization
- Temporal pushdown optimization strategy

---

## 4. Integration Points

### 4.1 Storage Layer
1. Add `created_at`/`valid_from`/`valid_to` fields to NodeRecord/RelationshipRecord
2. Add DateTime variant to PropertyValue enum
3. Extend DatabaseHeader with temporal metadata (format_version bump)
4. Version store: linked list of node/edge versions
5. Temporal index: BTreeMap on timestamp property

### 4.2 Query Layer
1. Lexer: AT, TIME, BETWEEN, DATETIME tokens
2. Parser: TemporalPredicate in MatchClause
3. Semantic: Validate temporal expressions
4. Planner: AsOfScan, TemporalRangeScan operators
5. Executor: Temporal scan/filter operators
6. API: QueryResult includes temporal metadata

### 4.3 Optimizer
1. Temporal index scan selection
2. Temporal predicate pushdown
3. Constant folding for datetime expressions

---

## 5. Risks and Constraints

1. **No DateTime type**: PropertyValue needs new variant — serialization format change
2. **In-memory B-tree**: Versions only persist via page serialization + WAL
3. **Format version**: May need to bump from 1 to 2 for temporal header
4. **Storage overhead**: Every version creates additional records
5. **Query complexity**: Temporal path queries exponentially complex
6. **Backward compatibility**: Non-temporal databases must still work

---

## 6. Recommended Phased Approach

### Phase 4a: Foundation
- PropertyValue::DateTime variant
- datetime() function in expression evaluator
- Timestamp tracking on create/update (system-managed)

### Phase 4b: Version Storage
- Version chain for nodes/edges (linked list approach, simpler than anchor+delta for v0.4)
- Version store in storage engine
- AT TIME syntax parsing and execution

### Phase 4c: Temporal Queries
- Point-in-time queries (AT TIME)
- Range queries (BETWEEN TIME)
- Temporal index for efficient version lookup

### Phase 4d: Quality
- Proptest for temporal invariants
- Benchmarks for version overhead
- Clippy clean, documentation
