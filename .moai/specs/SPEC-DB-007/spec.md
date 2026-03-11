---
id: SPEC-DB-007
version: "0.7.0"
status: draft
created: "2026-03-11"
updated: "2026-03-11"
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

**LL-003**: GraphEntity SHALL be extended to include temporal references:
```rust
pub enum GraphEntity {
    Node(NodeId),
    Subgraph(SubgraphId),
    TemporalRef(NodeId, i64),  // node at specific timestamp
}
```

**LL-004**: The system SHALL maintain a reverse index: BTreeMap<NodeId, Vec<HyperEdgeId>> for efficient lookup of all hyperedges containing a given node.

**LL-005**: The DatabaseHeader SHALL include hyperedge_root_page and next_hyperedge_id fields.

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

**MM-004**: The system SHALL support finding hyperedges containing a specific node:
```cypher
MATCH HYPEREDGE (h)-[:INVOLVES]->(person:Person {name: 'Alice'})
RETURN h
```

### Group NN: Temporal References

**NN-001**: A TemporalRef(NodeId, timestamp) SHALL resolve to the version of that node at the specified timestamp (using VersionStore).

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

### 4.1 HyperEdgeRecord

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
```

### 4.2 HyperEdgeStore

```rust
#[cfg(feature = "hypergraph")]
pub struct HyperEdgeStore {
    store: BTree<u64, HyperEdgeRecord>,
    reverse_index: BTreeMap<u64, Vec<u64>>,  // node_id -> [hyperedge_ids]
    next_id: u64,
}
```

### 4.3 DatabaseHeader v5

```
Bytes 68-75: hyperedge_root_page (v0.7)
Bytes 76-79: next_hyperedge_id (v0.7)
```

### 4.4 Query Pipeline

```
Lexer:    HYPEREDGE keyword
Parser:   HyperEdgeClause { sources, targets, properties }
Planner:  HyperEdgeScan { filter, source_pattern, target_pattern }
Executor: Iterate HyperEdgeStore with filter, resolve members
```

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
| Variable-size HyperEdgeRecord serialization | Medium | Medium | Reuse Vec serialization pattern from properties |
| Reverse index memory overhead | Medium | Medium | Lazy loading; only build when queried |
| HYPEREDGE syntax Cypher deviation | High | Low | Document as CypherLite extension; prefix with keyword |
| TemporalRef resolution performance | Medium | Medium | Cache resolved versions; lazy resolution |
| GraphEntity enum expansion | Low | Medium | Use extensible tagged serialization |
