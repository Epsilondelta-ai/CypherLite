# CypherLite Temporal Hypergraph Roadmap

**Document Date**: March 11, 2026
**Version**: 1.0
**Status**: Design Phase
**Audience**: Architects, Lead Engineers

---

## Executive Summary

This document describes the extended temporal roadmap for CypherLite, evolving from basic temporal queries (v0.4) to a full temporal hypergraph model (v0.7). CypherLite aims to become the first embedded graph database with native temporal hyperedge support.

---

## Architecture: Native + Feature Flags (SQLite Model)

All temporal features use Cargo compile-time feature flags. Zero runtime overhead. Users opt in at build time.

```toml
[features]
default = ["temporal-core"]
temporal-core = []                     # v0.4: DateTime, VersionStore, AT TIME nodes
temporal-edge = ["temporal-core"]      # v0.5: Edge validity, AT TIME edges, edge indexes
subgraph = ["temporal-edge"]           # v0.6: Subgraph entities, SNAPSHOT syntax
hypergraph = ["subgraph"]             # v0.7: Native hyperedges, HYPEREDGE syntax
full-temporal = ["hypergraph"]         # All temporal features
```

### Feature Flag Hierarchy

```
full-temporal
  └── hypergraph
        └── subgraph
              └── temporal-edge
                    └── temporal-core (default)
```

### DatabaseHeader Feature Bits

```
Bit 0: temporal-core   (v0.4, FORMAT_VERSION >= 2)
Bit 1: temporal-edge   (v0.5, FORMAT_VERSION >= 3)
Bit 2: subgraph        (v0.6, FORMAT_VERSION >= 4)
Bit 3: hypergraph      (v0.7, FORMAT_VERSION >= 5)
```

Compatibility: A binary without a required feature returns a clear error on database open.

---

## Phase Roadmap

### Phase 4: Temporal Foundation (v0.4) -- COMPLETED

**SPEC**: SPEC-DB-004 | **Status**: Completed | **Tests**: 978

| Feature | Description |
|---------|------------|
| PropertyValue::DateTime(i64) | Milliseconds since epoch, manual ISO 8601 parser |
| datetime() / now() | Built-in query functions |
| _created_at / _updated_at | Automatic node timestamp injection |
| VersionStore | Pre-update node/edge snapshots, version chain API |
| AT TIME syntax | Point-in-time node queries |
| BETWEEN TIME syntax | Temporal range node queries |

### Phase 5: Temporal Edge Validity (v0.5) -- PLANNED

**SPEC**: SPEC-DB-005 | **Priority**: P0 | **Depends on**: SPEC-DB-004

| Feature | Description |
|---------|------------|
| Feature flags foundation | Cargo features for modular temporal compilation |
| _valid_from / _valid_to on edges | Temporal validity periods for relationships |
| Edge property indexes | BTreeMap indexes on edge properties |
| AT TIME for edges | Filter edges by temporal validity during traversal |
| BETWEEN TIME for edges | Range queries on edge validity periods |
| Temporal path continuity | Variable-length paths respect time constraints |

**Key Design Decision**: AT TIME applies to BOTH nodes AND edges. An edge is valid at time T if `_valid_from <= T AND (_valid_to IS NULL OR _valid_to > T)`.

### Phase 6: Subgraph Entities (v0.6) -- PLANNED

**SPEC**: SPEC-DB-006 | **Priority**: P1 | **Depends on**: SPEC-DB-005

| Feature | Description |
|---------|------------|
| SubgraphRecord | First-class subgraph entity with temporal anchor |
| SubgraphStore | BTree storage + MembershipIndex (forward/reverse) |
| SNAPSHOT syntax | Materialize time-sliced graph views |
| GraphEntity enum | Edges can connect nodes OR subgraphs |
| Subgraph relationships | Standard edges between subgraph entities |
| :CONTAINS virtual edges | Query subgraph membership |

**Key Design Decision**: Subgraphs are immutable snapshots. Once created, membership doesn't change. Create new snapshots for new time points.

### Phase 7: Native Hyperedges (v0.7) -- PLANNED

**SPEC**: SPEC-DB-007 | **Priority**: P2 | **Depends on**: SPEC-DB-006

| Feature | Description |
|---------|------------|
| HyperEdgeRecord | Single edge connecting arbitrary node/subgraph sets |
| HyperEdgeStore | BTree storage + reverse index |
| TemporalRef | Reference a node at a specific timestamp |
| HYPEREDGE syntax | CREATE/MATCH hyperedges |
| Lazy TemporalRef resolution | Resolve node version only when properties accessed |

**Key Design Decision**: Hyperedges use FROM/TO sets (directed hyperedge), not undirected membership. TemporalRef enables connecting "node X at time T1" to "node X at time T2".

---

## Entity Model Evolution

```
v0.1-0.3: Standard Property Graph
  Node ──edge──> Node

v0.4: Temporal Property Graph
  Node ──edge──> Node
  │                │
  └──versions──>  └──versions──>

v0.5: Temporal Edge Validity
  Node ──edge[T1,T2]──> Node
  "Edge is valid from T1 to T2"

v0.6: Subgraph Entities
  Node ──edge──> Node
  Subgraph ──edge──> Subgraph
  Node ──edge──> Subgraph
  Subgraph {contains: [Node, Node, ...]}

v0.7: Temporal Hypergraph
  {Node, Node, Subgraph} ──hyperedge──> {Node@T, Subgraph}
  "N-to-M relationship with temporal references"
```

---

## Storage Architecture Evolution

```
v0.1: NodeStore + EdgeStore
v0.4: + VersionStore
v0.5: + EdgeIndexManager + feature_flags in header
v0.6: + SubgraphStore + MembershipIndex + GraphEntity
v0.7: + HyperEdgeStore + reverse index
```

DatabaseHeader grows from 44 bytes (v0.4) to ~80 bytes (v0.7), well within the 4096-byte page.

---

## Query Capability Matrix

| Query Pattern | v0.4 | v0.5 | v0.6 | v0.7 |
|--------------|:----:|:----:|:----:|:----:|
| Node AT TIME | Y | Y | Y | Y |
| Edge AT TIME | - | Y | Y | Y |
| Temporal paths | - | Y | Y | Y |
| Subgraph creation | - | - | Y | Y |
| Subgraph relationships | - | - | Y | Y |
| Hyperedge creation | - | - | - | Y |
| Temporal references (Node@T) | - | - | - | Y |
| Group events | - | - | - | Y |

---

## Competitive Positioning (v0.7)

CypherLite v0.7 would be the ONLY graph database (embedded or server) supporting all five capabilities: temporal edges, temporal nodes, subgraph entities, native hyperedges, and temporal node references.

| System | Temporal Nodes | Temporal Edges | Subgraphs | Hyperedges | Temporal Refs |
|--------|:-:|:-:|:-:|:-:|:-:|
| Neo4j | N | N | N | N | N |
| Amazon Neptune | N | N | Named Graphs | N | N |
| TigerGraph | N | N | N | N | N |
| AeonG (academic) | Y | Y | N | N | N |
| **CypherLite v0.7** | **Y** | **Y** | **Y** | **Y** | **Y** |

---

## Future Extensions (Post v0.7)

These are explicitly NOT in scope for v0.5-v0.7:

- **Bitemporal queries**: Valid time + transaction time (v0.8+)
- **Temporal graph algorithms**: PageRank over time, temporal shortest path (v0.9+)
- **Delta compression**: Anchor+delta versioning for storage efficiency (v0.8+)
- **Interval tree index**: Efficient overlapping interval queries (v0.8+)
- **Plugin API (trait-based)**: Extract temporal features to separate crate when API stabilizes (v1.0+)
- **FFI bindings**: Python, Node.js, C interfaces (v0.8+)

---

## Internal Trait Abstraction (Plugin Preparation)

While v0.5-v0.7 use feature flags (not plugins), internal code uses trait abstractions to prepare for future plugin extraction:

```rust
// Internal trait (pub(crate), NOT public API)
pub(crate) trait RecordStore {
    type Id;
    type Record;
    fn get(&self, id: Self::Id) -> Option<&Self::Record>;
    fn insert(&mut self, record: Self::Record) -> Result<Self::Id>;
    fn delete(&mut self, id: Self::Id) -> Result<Option<Self::Record>>;
}

// All stores implement this trait
impl RecordStore for NodeStore { ... }
impl RecordStore for EdgeStore { ... }
impl RecordStore for SubgraphStore { ... }
impl RecordStore for HyperEdgeStore { ... }
```

When the API stabilizes (v1.0+), `pub(crate)` becomes `pub` and stores can be extracted to separate crates.
