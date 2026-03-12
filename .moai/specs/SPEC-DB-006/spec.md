---
id: SPEC-DB-006
version: "0.6.0"
status: completed
created: "2026-03-11"
updated: "2026-03-12"
author: epsilondelta
priority: P1
tags: [subgraph, snapshot, temporal-subgraph, named-graph, metagraph]
lifecycle: spec-anchored
depends_on: [SPEC-DB-005]
---

# SPEC-DB-006: CypherLite Phase 6 - Subgraph Entities & Temporal Snapshots (v0.6)

> Introduce first-class Subgraph entities with dedicated storage, temporal anchor support, and SNAPSHOT syntax for materializing time-sliced graph views. Enable relationships between subgraphs to model temporal state transitions and community evolution.

---

## HISTORY

| Version | Date | Changes |
|---------|------|---------|
| 0.6.0 | 2026-03-11 | Initial SPEC based on temporal hypergraph analysis |

---

## 1. Environment

### 1.1 Feature Flag

```toml
subgraph = ["temporal-edge"]  # Requires v0.5 temporal edge support
```

### 1.2 Crate Structure

- **Extended crate**: `cypherlite-core` -- SubgraphId, SubgraphRecord types
- **New module**: `cypherlite-storage/src/subgraph/` -- SubgraphStore, MembershipIndex
- **Extended crate**: `cypherlite-query` -- SNAPSHOT/SUBGRAPH syntax, SubgraphScan operators

---

## 2. Requirements (EARS Format)

### Group GG: Subgraph Storage

**GG-001**: When the `subgraph` feature is enabled, the system SHALL provide a SubgraphStore backed by BTree<u64, SubgraphRecord>.

**GG-002**: The SubgraphRecord SHALL contain: subgraph_id (SubgraphId), name (optional String via property), temporal_anchor (Option<i64>), properties (Vec<(u32, PropertyValue)>).

**GG-003**: The system SHALL maintain a MembershipIndex: BTreeMap<SubgraphId, Vec<NodeId>> for forward lookup and BTreeMap<NodeId, Vec<SubgraphId>> for reverse lookup.

**GG-004**: The DatabaseHeader SHALL include subgraph_root_page and next_subgraph_id fields.

**GG-005**: The system SHALL support CRUD operations on subgraphs: create, get, delete, list_members.

### Group HH: SNAPSHOT Syntax

**HH-001**: The system SHALL support the syntax:
```
CREATE SNAPSHOT (var:Label {props})
AT TIME <datetime_expr>
FROM MATCH <pattern> WHERE <filter> RETURN <nodes>
```

**HH-002**: When a SNAPSHOT is created, the system SHALL:
1. Execute the FROM MATCH query at the specified time
2. Create a SubgraphRecord with the temporal anchor
3. Create membership entries for all matched nodes
4. Set _created_at on the subgraph

**HH-003**: When a SNAPSHOT is created without AT TIME, the system SHALL use the current timestamp as the temporal anchor.

**HH-004**: The system SHALL support querying subgraph members:
```
MATCH (sg:Subgraph {name: 'name'})-[:CONTAINS]->(n)
RETURN n
```

**HH-005**: The `:CONTAINS` relationship type SHALL be a virtual relationship derived from the MembershipIndex, not stored as physical edges.

### Group II: Subgraph Relationships

**II-001**: The system SHALL allow standard edges between SubgraphRecord entities, using a GraphEntity enum:
```rust
pub enum GraphEntity {
    Node(NodeId),
    Subgraph(SubgraphId),
}
```

**II-002**: RelationshipRecord start/end fields SHALL be extended to accept GraphEntity (backward-compatible: existing NodeId maps to GraphEntity::Node).

**II-003**: The system SHALL support creating edges between subgraphs:
```
MATCH (sg1:Subgraph {name: 'A'}), (sg2:Subgraph {name: 'B'})
CREATE (sg1)-[:MIGRATION {count: 30}]->(sg2)
```

**II-004**: The system SHALL support creating edges between nodes and subgraphs:
```
CREATE (person)-[:BELONGS_TO]->(sg)
```

### Group JJ: Subgraph Queries

**JJ-001**: The system SHALL support MATCH patterns that include subgraph nodes:
```
MATCH (sg:Subgraph)-[r:MIGRATION]->(sg2:Subgraph)
RETURN sg.name, sg2.name, r.count
```

**JJ-002**: The system SHALL support aggregate functions over subgraph members:
```
MATCH (sg:Subgraph {name: 'A'})-[:CONTAINS]->(n)
RETURN count(n) AS member_count
```

**JJ-003**: The system SHALL support filtering subgraphs by temporal anchor:
```
MATCH (sg:Subgraph)
WHERE sg._temporal_anchor >= datetime('2026-01-01')
RETURN sg.name
```

### Group KK: Quality

**KK-001**: All SPEC-DB-005 tests SHALL continue to pass.
**KK-002**: Proptest invariants for subgraph membership consistency.
**KK-003**: Integration tests for SNAPSHOT creation and subgraph relationship queries.
**KK-004**: Criterion benchmarks for subgraph creation and membership lookup.
**KK-005**: Version bump to 0.6.0.

---

## 3. Non-Goals for v0.6

- Hyperedge support (SPEC-DB-007)
- Automatic subgraph membership updates (snapshots are immutable once created)
- Subgraph-level ACID transactions
- Recursive subgraph nesting (subgraph containing subgraphs)

---

## 4. Architecture Design

### 4.1 SubgraphRecord

```rust
#[cfg(feature = "subgraph")]
pub struct SubgraphId(pub u64);

#[cfg(feature = "subgraph")]
pub struct SubgraphRecord {
    pub subgraph_id: SubgraphId,
    pub temporal_anchor: Option<i64>,
    pub properties: Vec<(u32, PropertyValue)>,
}
```

### 4.2 SubgraphStore

```rust
#[cfg(feature = "subgraph")]
pub struct SubgraphStore {
    store: BTree<u64, SubgraphRecord>,
    membership: MembershipIndex,
}

pub struct MembershipIndex {
    forward: BTreeMap<u64, Vec<u64>>,   // subgraph_id -> [node_ids]
    reverse: BTreeMap<u64, Vec<u64>>,   // node_id -> [subgraph_ids]
}
```

### 4.3 GraphEntity Extension

```rust
#[derive(Debug, Clone, PartialEq)]
pub enum GraphEntity {
    Node(NodeId),
    #[cfg(feature = "subgraph")]
    Subgraph(SubgraphId),
}
```

Serialization: tag byte 0 = Node, tag byte 1 = Subgraph.

### 4.4 DatabaseHeader v4

```
Bytes 56-63: subgraph_root_page (v0.6)
Bytes 64-67: next_subgraph_id (v0.6)
```

---

## 5. Example Scenario: 철수's Migration

```cypher
-- Step 1: Create base data
CREATE (철수:Person {name: '철수'})
CREATE (부산:City {name: '부산'}), (서울:City {name: '서울'})
CREATE (철수)-[:LIVED_IN {_valid_from: datetime('2026-03-04'), _valid_to: datetime('2026-03-10')}]->(부산)
CREATE (철수)-[:LIVED_IN {_valid_from: datetime('2026-03-10')}]->(서울)

-- Step 2: Create temporal snapshots
CREATE SNAPSHOT (sg1:LocationSnapshot {name: '철수_부산기간'})
AT TIME datetime('2026-03-05')
FROM MATCH (p:Person {name: '철수'})-[:LIVED_IN]->(c:City) RETURN p, c

CREATE SNAPSHOT (sg2:LocationSnapshot {name: '철수_서울기간'})
AT TIME datetime('2026-03-11')
FROM MATCH (p:Person {name: '철수'})-[:LIVED_IN]->(c:City) RETURN p, c

-- Step 3: Create temporal subgraph relationship
MATCH (sg1:LocationSnapshot {name: '철수_부산기간'})
MATCH (sg2:LocationSnapshot {name: '철수_서울기간'})
CREATE (sg1)-[:이사 {reason: '직장 이동', at: datetime('2026-03-10')}]->(sg2)

-- Step 4: Query temporal transitions
MATCH (from:LocationSnapshot)-[m:이사]->(to:LocationSnapshot)
RETURN from.name, to.name, m.reason, m.at
```

---

## 6. Risk Assessment

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| GraphEntity enum breaks existing serialization | Medium | High | Use tagged serialization; v3->v4 migration |
| MembershipIndex memory overhead | Medium | Medium | Lazy loading; index only when subgraph feature used |
| SNAPSHOT query complexity | Medium | Medium | Reuse existing MATCH executor; wrap results |
| Virtual :CONTAINS relationship confusion | Low | Low | Document clearly; distinguish from physical edges |
