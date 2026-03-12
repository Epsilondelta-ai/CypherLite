Perfect! Now I have enough information. Let me compile the comprehensive research document. Since this is a READ-ONLY task, I'll provide the research as a detailed response.

---

# SPEC-DB-007: Native Hyperedges Research Document

## Executive Summary

This comprehensive research document supports the implementation of SPEC-DB-007 (Native Hyperedges v0.7) for CypherLite. The analysis covers all nine research areas across the codebase and provides reference implementations, dependency maps, and implementation recommendations.

**Key Findings:**
- **GraphEntity enum** is stable, cfg(feature)-gated, with solid serialization patterns
- **SubgraphStore pattern** demonstrates working B-tree store architecture reusable for HyperEdgeStore
- **Feature flag chain** is correctly structured: temporal-core → temporal-edge → subgraph → hypergraph
- **Query pipeline** extends cleanly: lexer keywords exist, parser/AST patterns established, planner/executor support structures ready
- **Header versioning** v4 exists; v5 reserved bytes available at bytes 64-71

---

## Research Area 1: GraphEntity Enum (From SPEC-DB-006)

### Location & Definition
**File**: `/Users/epsilondelta/epsilonDelta/projects/CypherLite/crates/cypherlite-core/src/types.rs` (lines 55-63, 127-185)

```rust
/// A graph entity that can be either a node or a subgraph.
#[cfg(feature = "subgraph")]
#[derive(Debug, Clone, PartialEq)]
pub enum GraphEntity {
    /// A regular node in the graph.
    Node(NodeId),
    /// A subgraph containing other entities.
    Subgraph(SubgraphId),
}

#[cfg(feature = "subgraph")]
impl From<NodeId> for GraphEntity {
    fn from(id: NodeId) -> Self {
        GraphEntity::Node(id)
    }
}
```

### Usages Across Codebase

1. **RelationshipRecord Extension** (lines 117-124, 127-185):
   - `start_is_subgraph: bool` and `end_is_subgraph: bool` fields added
   - Methods: `start_entity()`, `end_entity()`, `from_entities()`, `is_subgraph_edge()`
   - Pattern: boolean flag + u64 raw ID conversion

2. **Value::Subgraph Executor Extension** (`cypherlite-query/src/executor/mod.rs` lines 25-27, 62-68):
   - Runtime `Value` enum extended with `#[cfg(feature = "subgraph")] Subgraph(SubgraphId)`
   - Bidirectional conversion: PropertyValue ↔ Value
   - Graph entities (Node, Edge, Subgraph) cannot convert to PropertyValue

3. **Storage API** (`cypherlite-storage/src/lib.rs`):
   - Subgraph CRUD operations through StorageEngine
   - Membership management via MembershipIndex

### Serialization Pattern
- Uses **binary tag byte** approach: tag 0 = Node, tag 1 = Subgraph
- **Backward compatible**: RelationshipRecord serde uses `#[serde(default)]` for new boolean fields
- **Feature-gated**: All GraphEntity code wrapped in `#[cfg(feature = "subgraph")]`

### cfg(feature) Pattern Evidence
- Core: `#[cfg(feature = "subgraph")]` on types.rs lines 39, 44, 56, 65, 118, 127
- Storage: `#[cfg(feature = "subgraph")]` on lib.rs lines 19, 42-45, 64-67, 109-117, 575-656
- Query: `#[cfg(feature = "subgraph")]` on ast.rs line 23, mod.rs lines 161-174

### Key Insight for SPEC-DB-007
GraphEntity enum is **extensible**: Current pattern supports Node + Subgraph. For HyperEdges, can add:
```rust
pub enum GraphEntity {
    Node(NodeId),
    Subgraph(SubgraphId),
    #[cfg(feature = "hypergraph")]
    HyperEdge(HyperEdgeId),
}
```

---

## Research Area 2: SubgraphStore Pattern (Reference for HyperEdgeStore)

### Module Structure
**Path**: `/Users/epsilondelta/epsilonDelta/projects/CypherLite/crates/cypherlite-storage/src/subgraph/`

#### SubgraphStore Implementation
**File**: `mod.rs` (87 lines)

```rust
pub struct SubgraphStore {
    /// Storage: subgraph_id -> SubgraphRecord
    records: BTreeMap<u64, SubgraphRecord>,
    /// Next available subgraph ID.
    next_id: u64,
}
```

**CRUD API**:
- `pub fn new(start_id: u64)` - Initialize with starting ID counter
- `pub fn create(...) -> SubgraphId` - Auto-increment ID allocation
- `pub fn get(&self, id: SubgraphId) -> Option<&SubgraphRecord>` - Lookup by ID
- `pub fn delete(&mut self, id: SubgraphId) -> Option<SubgraphRecord>` - Remove and return
- `pub fn next_id(&self) -> u64` - Get next available ID
- `pub fn len(&self), is_empty(&self)` - Metrics
- `pub fn all(&self) -> impl Iterator<Item = &SubgraphRecord>` - Full scan

**Design Observations**:
1. **In-memory only** (currently using BTreeMap, not persisted to pages)
2. **Direct U64 keying**: SubgraphId(u64) maps to records BTreeMap key
3. **No separate versioning**: Records are immutable snapshots
4. **Auto-incrementing ID**: Thread-unsafe (mutable self required)

#### MembershipIndex Implementation
**File**: `membership.rs` (224 lines)

```rust
pub struct MembershipIndex {
    /// Forward index: subgraph_id -> list of node IDs.
    forward: BTreeMap<u64, Vec<u64>>,
    /// Reverse index: node_id -> list of subgraph IDs.
    reverse: BTreeMap<u64, Vec<u64>>,
}
```

**Key Methods**:
- `pub fn add(&mut self, subgraph_id: SubgraphId, node_id: NodeId)` - Idempotent add
- `pub fn remove(&mut self, ...) -> bool` - Returns success flag
- `pub fn remove_all(&mut self, subgraph_id) -> Vec<NodeId>` - Batch remove + cleanup
- `pub fn members(&self, subgraph_id) -> Vec<NodeId>` - Forward lookup
- `pub fn memberships(&self, node_id) -> Vec<SubgraphId>` - Reverse lookup

**Design Observations**:
1. **Dual-index**: Both forward and reverse maintained
2. **Idempotent operations**: Adding duplicate is no-op
3. **Automatic cleanup**: Empty lists removed to save memory
4. **No persistence**: Fully in-memory (for now)

### Integration with StorageEngine

**File**: `/Users/epsilondelta/epsilonDelta/projects/CypherLite/crates/cypherlite-storage/src/lib.rs` (lines 575-656)

```rust
pub struct StorageEngine {
    // ... other fields ...
    #[cfg(feature = "subgraph")]
    subgraph_store: SubgraphStore,
    #[cfg(feature = "subgraph")]
    membership_index: MembershipIndex,
}

// Initialization (lines 109-117)
#[cfg(feature = "subgraph")]
let next_subgraph_id = page_manager.header().next_subgraph_id;
#[cfg(feature = "subgraph")]
let subgraph_store = if next_subgraph_id > 0 {
    SubgraphStore::new(next_subgraph_id)
} else {
    SubgraphStore::new(1)
};
```

**Public API** (lines 579-656):
- `create_subgraph()` - Creates and updates header.next_subgraph_id
- `get_subgraph(id)` - Direct lookup
- `delete_subgraph()` - Removes and cleans memberships
- `add_member()`, `remove_member()` - Membership management
- `list_members()`, `get_subgraph_memberships()` - Bidirectional queries
- `scan_subgraphs()` - Iterator over all records

### Key Pattern for HyperEdgeStore

**Structure to Replicate:**
1. Main store: `BTreeMap<u64, HyperEdgeRecord>` with auto-increment ID
2. Index 1: `BTreeMap<u64, Vec<u64>>` for source → targets (forward)
3. Index 2: `BTreeMap<u64, Vec<u64>>` for target → sources (reverse)
4. StorageEngine field: `#[cfg(feature = "hypergraph")] hyperedge_store: HyperEdgeStore`
5. StorageEngine integration: Create/delete/scan operations

---

## Research Area 3: B-Tree Storage Pattern

### BTree Implementation
**File**: `/Users/epsilondelta/epsilonDelta/projects/CypherLite/crates/cypherlite-storage/src/btree/mod.rs` (156 lines)

```rust
pub struct BTree<K: Ord + Clone, V: Clone> {
    /// In-memory B-tree index.
    entries: BTreeMap<K, V>,
    /// Root page ID in the database file (0 means not yet assigned).
    root_page: u32,
}
```

**Core Operations**:
- `pub fn insert(&mut self, key: K, value: V) -> Option<V>` - Insert/update (O(log n))
- `pub fn search(&self, key: &K) -> Option<&V>` - Lookup (O(log n))
- `pub fn delete(&mut self, key: &K) -> Option<V>` - Remove (O(log n))
- `pub fn range_scan(&self, start: &K, end: &K) -> Vec<(&K, &V)>` - Range query
- `pub fn iter(&self) -> impl Iterator<Item = (&K, &V)>` - Full scan
- `pub fn root_page(&self)` / `set_root_page(&mut self, page: u32)` - Page allocation

**Current State**:
- **In-memory only**: Backed by Rust's BTreeMap (not page-based yet)
- **Root page tracking**: Prepared for future disk persistence
- **Ordered iteration**: Supports range scans and sorted traversal

### How Stores Use BTree

#### NodeStore (`src/btree/node_store.rs`)
```rust
pub struct NodeStore {
    tree: BTree<u64, NodeRecord>,
    next_id: u64,
}
```
- Key: NodeId.0 (u64)
- Value: NodeRecord (with labels, properties, adjacency chains)

#### EdgeStore (`src/btree/edge_store.rs`)
```rust
pub struct EdgeStore {
    tree: BTree<u64, RelationshipRecord>,
    next_id: u64,
}
```
- Key: EdgeId.0 (u64)
- Value: RelationshipRecord (start, end, type, direction, adjacency)

#### SubgraphStore Divergence
- **Does NOT use BTree**: Uses direct BTreeMap<u64, SubgraphRecord>
- **Reason**: Subgraph persistence not yet implemented; simpler in-memory store
- **Future**: Should migrate to B-tree for consistency and persistence

### Root Page Allocation Pattern

**HeaderPage Storage** (`src/page/mod.rs` lines 138-145, 207-212):
```rust
pub struct DatabaseHeader {
    pub root_node_page: u32,
    pub root_edge_page: u32,
    #[cfg(feature = "subgraph")]
    pub subgraph_root_page: u64,
}

// Serialization (lines 207-212)
#[cfg(feature = "subgraph")]
{
    page[48..56].copy_from_slice(&self.subgraph_root_page.to_le_bytes());
    page[56..64].copy_from_slice(&self.next_subgraph_id.to_le_bytes());
}
```

**For HyperEdgeStore:**
- Root page would be allocated at bytes 64-71 in v5 header (currently bytes 64-71 are unallocated)
- Next hyperedge ID at bytes 72-79

### Implication for HyperEdgeStore

**Two possible approaches**:
1. **Lightweight approach** (like SubgraphStore): In-memory BTreeMap, no persistence
2. **Full persistence approach**: Implement HyperEdgeBTree with page allocation

Given subgraph stores are still in-memory, recommend **lightweight approach for MVP**, with comment about future persistence.

---

## Research Area 4: Query Pipeline Extension Pattern

### Phase 1: Lexer Extensions

**File**: `/Users/epsilondelta/epsilonDelta/projects/CypherLite/crates/cypherlite-query/src/lexer/mod.rs` (lines 1-200)

**Current Keywords** (lines 128-142):
```rust
// -- P6 Keywords (Subgraph/Snapshot) ------------------------------------
#[regex("(?i)snapshot", priority = 10)]
Snapshot,
#[regex("(?i)from", priority = 10)]
From,

// -- P4 Keywords (Temporal) ---------------------------------------------
#[regex("(?i)at", priority = 10)]
At,
#[regex("(?i)time", priority = 10)]
Time,
```

**For SPEC-DB-007 (Hyperedges)**:

New keywords would be:
```rust
// -- P7 Keywords (Hyperedges) -------------------------------------------
#[regex("(?i)hyperedge", priority = 10)]
Hyperedge,
#[regex("(?i)sources", priority = 10)]
Sources,
#[regex("(?i)targets", priority = 10)]
Targets,
```

**Pattern**: All keywords use `#[regex(..., priority = 10)]` with case-insensitive matching.

### Phase 2: AST Extensions

**File**: `/Users/epsilondelta/epsilonDelta/projects/CypherLite/crates/cypherlite-query/src/parser/ast.rs` (lines 1-275)

**Top-level Enum** (lines 9-25):
```rust
pub enum Clause {
    Match(MatchClause),
    Return(ReturnClause),
    Create(CreateClause),
    #[cfg(feature = "subgraph")]
    CreateSnapshot(CreateSnapshotClause),
}
```

**CreateSnapshotClause Reference** (lines 256-274):
```rust
#[cfg(feature = "subgraph")]
#[derive(Debug, Clone, PartialEq)]
pub struct CreateSnapshotClause {
    pub variable: Option<String>,
    pub labels: Vec<String>,
    pub properties: Option<MapLiteral>,
    pub temporal_anchor: Option<Expression>,
    pub from_match: MatchClause,
    pub from_return: Vec<ReturnItem>,
}
```

**For SPEC-DB-007**:

Would add:
```rust
#[cfg(feature = "hypergraph")]
pub struct CreateHyperedgeClause {
    pub variable: Option<String>,
    pub properties: Option<MapLiteral>,
    pub sources: Vec<Expression>,  // list of source node/edge IDs
    pub targets: Vec<Expression>,  // list of target node/edge IDs
}
```

### Phase 3: Parser Implementation

**File**: `/Users/epsilondelta/epsilonDelta/projects/CypherLite/crates/cypherlite-query/src/parser/mod.rs` (lines 1-850)

**CREATE SNAPSHOT Parser** (grep shows line 68):
```rust
Clause::CreateSnapshot(parser.parse_create_snapshot_clause()?)
```

**Pattern for Adding Clauses** (based on CREATE SNAPSHOT):
1. Detect keyword in `parse_statement()` at top level
2. Delegate to specific parser function: `parse_create_snapshot_clause()`
3. Parse sub-components (variable, properties, sub-clauses)
4. Return AST node wrapped in Clause enum

**Testing** (`parser/mod.rs` lines 764-826):
- Full CREATE SNAPSHOT query tests
- WITH AT TIME temporal anchor tests
- WITH WHERE filter tests
- WITH property sets tests

### Phase 4: Planner Extensions

**File**: `/Users/epsilondelta/epsilonDelta/projects/CypherLite/crates/cypherlite-query/src/planner/mod.rs` (lines 1-700)

**LogicalPlan Enum** (lines 8-175):
```rust
pub enum LogicalPlan {
    NodeScan { ... },
    Expand { ... },
    Filter { ... },
    #[cfg(feature = "subgraph")]
    SubgraphScan { variable: String },
    #[cfg(feature = "subgraph")]
    CreateSnapshotOp {
        variable: Option<String>,
        labels: Vec<String>,
        properties: Option<MapLiteral>,
        temporal_anchor: Option<Expression>,
        sub_plan: Box<LogicalPlan>,
        return_vars: Vec<String>,
    },
}
```

**For SPEC-DB-007**:

Would add:
```rust
#[cfg(feature = "hypergraph")]
CreateHyperedgeOp {
    variable: Option<String>,
    properties: Option<MapLiteral>,
    sources: Vec<Expression>,
    targets: Vec<Expression>,
}
```

**Planning Logic** (typical pattern):
1. Pattern analysis: Detect hyperedge creation pattern
2. Source/target collection: Evaluate source/target expressions
3. Sub-plan generation: For each source/target, create scan/lookup plan
4. Merge: CreateHyperedgeOp at top

### Phase 5: Executor Implementation

**File**: `/Users/epsilondelta/epsilonDelta/projects/CypherLite/crates/cypherlite-query/src/executor/mod.rs` (lines 90-350)

**Main Executor Dispatch** (lines 96-150):
```rust
pub fn execute(
    plan: &LogicalPlan,
    engine: &mut StorageEngine,
    params: &Params,
) -> Result<Vec<Record>, ExecutionError> {
    match plan {
        LogicalPlan::NodeScan { ... } => {
            execute_node_scan(...)
        }
        LogicalPlan::CreateSnapshotOp { ... } => {
            execute_create_snapshot_op(...)
        }
        // ... many more arms ...
    }
}
```

**Value::Subgraph Pattern** (lines 25-27, 62-68):
```rust
#[cfg(feature = "subgraph")]
Subgraph(cypherlite_core::SubgraphId),
```

**For SPEC-DB-007**:

Would add:
```rust
#[cfg(feature = "hypergraph")]
Hyperedge(cypherlite_core::HyperEdgeId),
```

And operator:
```rust
#[cfg(feature = "hypergraph")]
LogicalPlan::CreateHyperedgeOp { ... } => {
    execute_create_hyperedge_op(...)
}
```

**Operator Module Structure** (`src/executor/operators/`):
- `create.rs` - CREATE node/edge patterns
- `subgraph_scan.rs` - MATCH (sg:Subgraph) scans
- New: `hyperedge_create.rs` or extend `create.rs`

### Full Pipeline Trace: CREATE SNAPSHOT Example

1. **Lexer**: Tokenizes `CREATE SNAPSHOT (s:Snap) FROM MATCH (n) RETURN n`
   - Tokens: CREATE, SNAPSHOT, LPAREN, s, COLON, Snap, RPAREN, FROM, MATCH, ...

2. **Parser** (`parser/mod.rs`):
   - Detects CREATE keyword (line 55-68)
   - Peeks next tokens to identify SNAPSHOT variant
   - Calls `parse_create_snapshot_clause()` → CreateSnapshotClause AST node

3. **AST Result**:
   ```rust
   Clause::CreateSnapshot(CreateSnapshotClause {
       variable: Some("s"),
       labels: vec!["Snap"],
       properties: None,
       temporal_anchor: None,
       from_match: MatchClause { ... },
       from_return: vec![ReturnItem { ... }],
   })
   ```

4. **Planner** (`planner/mod.rs`):
   - Converts CreateSnapshotClause to LogicalPlan
   - Creates sub-plan for `MATCH (n) RETURN n`
   - Wraps in CreateSnapshotOp

5. **Executor** (`executor/mod.rs`):
   - Matches on LogicalPlan::CreateSnapshotOp
   - Calls `execute_create_snapshot_op()`
   - Sub-plan executes first: returns nodes
   - Creates subgraph and membership entries
   - Returns empty result (or created snapshot ID)

---

## Research Area 5: Feature Flag Pattern

### Feature Chain Definition

**cypherlite-core/Cargo.toml** (lines 12-18):
```toml
[features]
default = ["temporal-core"]
temporal-core = []
temporal-edge = ["temporal-core"]
subgraph = ["temporal-edge"]
hypergraph = ["subgraph"]
full-temporal = ["hypergraph"]
```

**cypherlite-storage/Cargo.toml** (lines 7-13):
```toml
[features]
default = ["temporal-core"]
temporal-core = ["cypherlite-core/temporal-core"]
temporal-edge = ["temporal-core", "cypherlite-core/temporal-edge"]
subgraph = ["temporal-edge", "cypherlite-core/subgraph"]
hypergraph = ["subgraph", "cypherlite-core/hypergraph"]
full-temporal = ["hypergraph", "cypherlite-core/full-temporal"]
```

**cypherlite-query/Cargo.toml** (lines 7-13):
```toml
[features]
default = ["temporal-core"]
temporal-core = ["cypherlite-core/temporal-core", "cypherlite-storage/temporal-core"]
temporal-edge = ["temporal-core", "cypherlite-core/temporal-edge", "cypherlite-storage/temporal-edge"]
subgraph = ["temporal-edge", "cypherlite-core/subgraph", "cypherlite-storage/subgraph"]
hypergraph = ["subgraph", "cypherlite-core/hypergraph", "cypherlite-storage/hypergraph"]
full-temporal = ["hypergraph", "cypherlite-core/full-temporal", "cypherlite-storage/full-temporal"]
```

### Feature Dependency Chain

```
temporal-core (base, default)
    ↓ (required by)
temporal-edge (timestamps, interval queries)
    ↓ (required by)
subgraph (SNAPSHOT syntax, SubgraphStore, GraphEntity)
    ↓ (required by)
hypergraph (HYPEREDGE syntax, HyperEdgeStore, multiple sources/targets)
    ↓ (required by)
full-temporal (combination of all above)
```

### cfg(feature) Usage Pattern

**Type Definition** (types.rs):
```rust
#[cfg(feature = "subgraph")]
pub struct SubgraphId(pub u64);

#[cfg(feature = "subgraph")]
#[derive(Debug, Clone, PartialEq)]
pub enum GraphEntity {
    Node(NodeId),
    Subgraph(SubgraphId),
}
```

**Storage Module** (lib.rs):
```rust
#[cfg(feature = "subgraph")]
pub mod subgraph;

#[cfg(feature = "subgraph")]
use subgraph::SubgraphStore;

pub struct StorageEngine {
    #[cfg(feature = "subgraph")]
    subgraph_store: SubgraphStore,
}

#[cfg(feature = "subgraph")]
pub fn create_subgraph(...) { ... }
```

**Query AST** (parser/ast.rs):
```rust
#[cfg(feature = "subgraph")]
CreateSnapshot(CreateSnapshotClause),
```

### For SPEC-DB-007 (hypergraph feature)

**Already defined in Cargo.toml** (no changes needed):
- cypherlite-core: `hypergraph = ["subgraph"]`
- cypherlite-storage: `hypergraph = ["subgraph"]`
- cypherlite-query: `hypergraph = ["subgraph"]`

**Will add**:
```rust
// types.rs
#[cfg(feature = "hypergraph")]
pub struct HyperEdgeId(pub u64);

#[cfg(feature = "hypergraph")]
pub struct HyperEdgeRecord {
    pub hyperedge_id: HyperEdgeId,
    pub sources: Vec<GraphEntity>,  // Nodes or Subgraphs
    pub targets: Vec<GraphEntity>,
    pub properties: Vec<(u32, PropertyValue)>,
}

// Extend GraphEntity
pub enum GraphEntity {
    Node(NodeId),
    Subgraph(SubgraphId),
    #[cfg(feature = "hypergraph")]
    HyperEdge(HyperEdgeId),
}
```

---

## Research Area 6: DatabaseHeader Versioning

### Current Version Structure

**File**: `/Users/epsilondelta/epsilonDelta/projects/CypherLite/crates/cypherlite-storage/src/page/mod.rs` (lines 114-276)

**Version Constants** (lines 12-17):
```rust
/// Current database format version.
#[cfg(feature = "subgraph")]
pub const FORMAT_VERSION: u32 = 4;
/// Current database format version.
#[cfg(not(feature = "subgraph"))]
pub const FORMAT_VERSION: u32 = 3;
```

**DatabaseHeader Structure** (lines 115-146):
```rust
pub struct DatabaseHeader {
    pub magic: u32,                           // bytes 0-3
    pub version: u32,                         // bytes 4-7
    pub page_count: u32,                      // bytes 8-11
    pub root_node_page: u32,                  // bytes 12-15
    pub root_edge_page: u32,                  // bytes 16-19
    pub next_node_id: u64,                    // bytes 20-27
    pub next_edge_id: u64,                    // bytes 28-35
    pub version_store_root_page: u64,         // bytes 36-43 (v2+)
    pub feature_flags: u32,                   // bytes 44-47 (v3+)
    #[cfg(feature = "subgraph")]
    pub subgraph_root_page: u64,              // bytes 48-55 (v4+)
    #[cfg(feature = "subgraph")]
    pub next_subgraph_id: u64,                // bytes 56-63 (v4+)
}
```

**Byte Layout Summary**:
- **Bytes 0-35**: Core fields (magic, version, page_count, root pointers, ID counters)
- **Bytes 36-43**: version_store_root_page (v2+)
- **Bytes 44-47**: feature_flags (v3+)
- **Bytes 48-55**: subgraph_root_page (v4+)
- **Bytes 56-63**: next_subgraph_id (v4+)
- **Bytes 64-71**: AVAILABLE for v5 (hyperedge_root_page)
- **Bytes 72-79**: AVAILABLE for v5 (next_hyperedge_id)
- **Bytes 80-4095**: Padding (unused)

### Serialization Code

**to_page() Method** (lines 194-214):
```rust
pub fn to_page(&self) -> [u8; PAGE_SIZE] {
    let mut page = [0u8; PAGE_SIZE];
    page[0..4].copy_from_slice(&self.magic.to_le_bytes());
    page[4..8].copy_from_slice(&self.version.to_le_bytes());
    page[8..12].copy_from_slice(&self.page_count.to_le_bytes());
    page[12..16].copy_from_slice(&self.root_node_page.to_le_bytes());
    page[16..20].copy_from_slice(&self.root_edge_page.to_le_bytes());
    page[20..28].copy_from_slice(&self.next_node_id.to_le_bytes());
    page[28..36].copy_from_slice(&self.next_edge_id.to_le_bytes());
    page[36..44].copy_from_slice(&self.version_store_root_page.to_le_bytes());
    page[44..48].copy_from_slice(&self.feature_flags.to_le_bytes());
    #[cfg(feature = "subgraph")]
    {
        page[48..56].copy_from_slice(&self.subgraph_root_page.to_le_bytes());
        page[56..64].copy_from_slice(&self.next_subgraph_id.to_le_bytes());
    }
    page
}
```

### Deserialization & Migration

**from_page() Method** (lines 218-276):
```rust
pub fn from_page(page: &[u8; PAGE_SIZE]) -> Self {
    let version = u32::from_le_bytes([page[4], page[5], page[6], page[7]]);

    // W-004: Auto-migrate v1 headers (bytes 36-43 are zero = no version store)
    let version_store_root_page = if version >= 2 {
        u64::from_le_bytes([...])
    } else {
        0 // v1 headers have no version store field
    };

    // AA-T2: feature_flags at bytes 44-47 (v3+)
    let feature_flags = if version >= 3 {
        u32::from_le_bytes([...])
    } else {
        // Auto-migrate: v1/v2 databases default to temporal-core only
        Self::FLAG_TEMPORAL_CORE
    };

    // GG-003: subgraph fields at bytes 48-55, 56-63 (v4+)
    #[cfg(feature = "subgraph")]
    let subgraph_root_page = if version >= 4 {
        u64::from_le_bytes([...])
    } else {
        0 // Auto-migrate: pre-v4 databases have no subgraph store
    };
    
    // ... more fields ...
}
```

### Feature Flags

**Flag Definitions** (lines 149-155):
```rust
pub const FLAG_TEMPORAL_CORE: u32 = 1 << 0;    // Bit 0
pub const FLAG_TEMPORAL_EDGE: u32 = 1 << 1;    // Bit 1
#[cfg(feature = "subgraph")]
pub const FLAG_SUBGRAPH: u32 = 1 << 2;          // Bit 2
```

**Compiled Flags** (lines 177-191):
```rust
pub fn compiled_feature_flags() -> u32 {
    let mut flags = 0u32;
    flags |= Self::FLAG_TEMPORAL_CORE;
    #[cfg(feature = "temporal-edge")]
    {
        flags |= Self::FLAG_TEMPORAL_EDGE;
    }
    #[cfg(feature = "subgraph")]
    {
        flags |= Self::FLAG_SUBGRAPH;
    }
    flags
}
```

### For SPEC-DB-007 (Header v5)

**Changes Needed**:

1. **Format version constant** (add conditional):
```rust
#[cfg(feature = "hypergraph")]
pub const FORMAT_VERSION: u32 = 5;
#[cfg(all(feature = "subgraph", not(feature = "hypergraph")))]
pub const FORMAT_VERSION: u32 = 4;
```

2. **DatabaseHeader struct** (add fields):
```rust
#[cfg(feature = "hypergraph")]
pub hyperedge_root_page: u64,         // bytes 64-71
#[cfg(feature = "hypergraph")]
pub next_hyperedge_id: u64,           // bytes 72-79
```

3. **Feature flag** (add bit):
```rust
#[cfg(feature = "hypergraph")]
pub const FLAG_HYPERGRAPH: u32 = 1 << 3;    // Bit 3
```

4. **Serialization** (extend to_page):
```rust
#[cfg(feature = "hypergraph")]
{
    page[64..72].copy_from_slice(&self.hyperedge_root_page.to_le_bytes());
    page[72..80].copy_from_slice(&self.next_hyperedge_id.to_le_bytes());
}
```

5. **Deserialization** (extend from_page):
```rust
#[cfg(feature = "hypergraph")]
let hyperedge_root_page = if version >= 5 {
    u64::from_le_bytes([...])
} else {
    0 // Auto-migrate: pre-v5 databases have no hyperedge store
};
```

---

## Research Area 7: Serialization for Variable-Size Records

### PropertyValue Serialization

**File**: `/Users/epsilondelta/epsilonDelta/projects/CypherLite/crates/cypherlite-core/src/types.rs` (lines 17-36)

**PropertyValue Enum**:
```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PropertyValue {
    Null,                           // type tag 0
    Bool(bool),                     // type tag 1
    Int64(i64),                     // type tag 2
    Float64(f64),                   // type tag 3
    String(String),                 // type tag 4 - VARIABLE SIZE
    Bytes(Vec<u8>),                 // type tag 5 - VARIABLE SIZE
    Array(Vec<PropertyValue>),      // type tag 6 - VARIABLE SIZE, NESTED
    DateTime(i64),                  // type tag 7
}
```

**Type Tag System** (lines 189-200):
```rust
pub fn type_tag(&self) -> u8 {
    match self {
        PropertyValue::Null => 0,
        PropertyValue::Bool(_) => 1,
        PropertyValue::Int64(_) => 2,
        PropertyValue::Float64(_) => 3,
        PropertyValue::String(_) => 4,
        PropertyValue::Bytes(_) => 5,
        PropertyValue::Array(_) => 6,
        PropertyValue::DateTime(_) => 7,
    }
}
```

### Serialization Pattern

**Serde Usage** (`#[derive(Serialize, Deserialize)]`):
- Uses **bincode** crate for binary encoding
- Automatic variable-length encoding for Vec types
- Recursive encoding for nested Arrays

**Storage in Records** (NodeRecord, RelationshipRecord):
```rust
pub properties: Vec<(u32, PropertyValue)>,
```

**Serialization in Tests** (types.rs lines 438-458):
```rust
#[test]
fn test_property_value_serialization_roundtrip() {
    let values = vec![
        PropertyValue::Null,
        PropertyValue::Bool(true),
        PropertyValue::Int64(-999),
        PropertyValue::Float64(2.5_f64),
        PropertyValue::String("test".into()),
        PropertyValue::Bytes(vec![0xDE, 0xAD]),
        PropertyValue::Array(vec![PropertyValue::Int64(1), PropertyValue::Null]),
    ];
    for val in &values {
        let encoded = bincode::serialize(val).expect("serialize");
        let decoded: PropertyValue = bincode::deserialize(&encoded).expect("deserialize");
        assert_eq!(val, &decoded);
    }
}
```

### Vec<PropertyValue> Pattern

**NodeRecord Example**:
```rust
pub struct NodeRecord {
    pub node_id: NodeId,
    pub labels: Vec<u32>,                      // List of label IDs
    pub properties: Vec<(u32, PropertyValue)>, // Key-value pairs
    pub next_edge_id: Option<EdgeId>,
    pub overflow_page: Option<PageId>,
}
```

**Serialization**:
- Vec length encoded as prefix
- Each element serialized in sequence
- PropertyValue includes type tag + variant data
- String/Bytes include length + data

### For HyperEdgeRecord Serialization

**Proposed Structure**:
```rust
#[cfg(feature = "hypergraph")]
pub struct HyperEdgeRecord {
    pub hyperedge_id: HyperEdgeId,
    pub sources: Vec<GraphEntity>,              // Variable-size list
    pub targets: Vec<GraphEntity>,              // Variable-size list
    pub properties: Vec<(u32, PropertyValue)>,  // Variable-size, nested
}
```

**Serialization Strategy**:
1. **GraphEntity encoding**: Prefix byte (0=Node, 1=Subgraph, 2=HyperEdge)
2. **Vec<GraphEntity>**: Length prefix + each entity
3. **Properties**: Reuse existing Vec<(u32, PropertyValue)> pattern

**Example Encoding**:
```
HyperEdgeRecord {
    id: 42,
    sources: [Node(10), Subgraph(5)],
    targets: [Node(20)],
    properties: [(1, Int64(99))]
}

Bytes:
[42 encoded as u64]
[2 sources: len=2] [Tag 0][10 encoded] [Tag 1][5 encoded]
[1 target: len=1] [Tag 0][20 encoded]
[1 property: len=1] [key=1] [tag=2][value=99 encoded]
```

---

## Research Area 8: VersionStore for TemporalRef

### VersionStore Implementation

**File**: `/Users/epsilondelta/epsilonDelta/projects/CypherLite/crates/cypherlite-storage/src/version/mod.rs` (260 lines)

**Structure**:
```rust
pub struct VersionStore {
    /// Storage: (entity_id, version_seq) -> VersionRecord
    versions: BTreeMap<(u64, u64), VersionRecord>,
    /// Next version sequence number per entity.
    next_seq: BTreeMap<u64, u64>,
}

pub enum VersionRecord {
    Node(NodeRecord),
    Relationship(RelationshipRecord),
}
```

**API**:
- `pub fn snapshot_node(&mut self, entity_id: u64, record: NodeRecord) -> u64` - Capture before update, return version seq
- `pub fn snapshot_relationship(&mut self, entity_id: u64, record: RelationshipRecord) -> u64` - Same for edges
- `pub fn get_version(&self, entity_id: u64, version_seq: u64) -> Option<&VersionRecord>` - Get specific version
- `pub fn get_latest_version(&self, entity_id: u64) -> Option<&VersionRecord>` - Most recent snapshot
- `pub fn get_version_chain(&self, entity_id: u64) -> Vec<(u64, &VersionRecord)>` - Full history
- `pub fn version_count(&self, entity_id: u64) -> u64` - Count versions

### Integration with StorageEngine

**File**: `/Users/epsilondelta/epsilonDelta/projects/CypherLite/crates/cypherlite-storage/src/lib.rs` (lines 171-180)

```rust
pub fn update_node(
    &mut self,
    node_id: NodeId,
    properties: Vec<(u32, PropertyValue)>,
) -> Result<()> {
    // Capture old properties for index removal
    let old_node = self.node_store.get_node(node_id).cloned();

    // W-002: Pre-update snapshot into VersionStore
    if self.config.version_storage_enabled {
        if let Some(ref old) = old_node {
            self.version_store.snapshot_node(node_id.0, old.clone());
        }
    }
    // ... update node ...
    Ok(())
}
```

### TemporalRef Resolution Pattern

**Currently Absent**: No TemporalRef type yet. But VersionStore demonstrates the pattern:
1. **Capture**: Before update, snapshot the entity
2. **Index**: Store by (entity_id, sequence)
3. **Query**: Look up by timestamp (requires matching between sequence and creation time)

### Tests Demonstrate Usage

**File**: `tests/version_storage.rs` (cypherlite-query)

Example test flow:
1. Create nodes/edges
2. Update nodes/edges (triggers snapshots)
3. Query `node AT TIME timestamp`
4. Executor matches timestamp to version sequence

### For TemporalRef in SPEC-DB-007

**Concept: TemporalRef**

```rust
pub enum TemporalRef {
    /// Reference to a node at a specific point in time
    NodeAtTime { node_id: NodeId, timestamp: i64 },
    /// Reference to a subgraph at a specific point in time
    SubgraphAtTime { subgraph_id: SubgraphId, timestamp: i64 },
}
```

**Usage in Queries**:
```cypher
MATCH (n:Person AT TIME 1700000000000)
RETURN n
```

**Execution**:
1. Parser: Convert `AT TIME` to TemporalRef in pattern
2. Planner: Create AsOfScan plan node
3. Executor: 
   - Get version chain for node
   - Find version created at or before timestamp
   - Return that version

**Storage Integration**:
- Snapshots captured in VersionStore
- Temporal anchor in SubgraphRecord (already exists)
- New: Temporal anchor in HyperEdgeRecord?

---

## Research Area 9: Test Patterns

### Feature-Gated Tests

**Example: Subgraph Tests** (types.rs lines 610-967)

```rust
#[cfg(feature = "subgraph")]
mod subgraph_tests {
    use super::*;

    #[test]
    fn test_subgraph_id_creation_and_equality() {
        let id1 = SubgraphId(1);
        let id2 = SubgraphId(1);
        assert_eq!(id1, id2);
    }
    
    // ... more tests ...
}
```

**Pattern**:
- Entire module wrapped in `#[cfg(feature = "subgraph")]`
- Runs only when feature compiled
- Fails gracefully if feature disabled (tests skipped)

### Proptest Property-Based Tests

**File**: `cypherlite-storage/tests/proptest_storage.rs` (500+ lines)

**Strategy Generators**:
```rust
fn arb_property_value() -> impl Strategy<Value = PropertyValue> {
    prop_oneof![
        8 => arb_property_value_leaf(),
        2 => prop::collection::vec(arb_property_value_leaf(), 0..8)
            .prop_map(PropertyValue::Array),
    ]
}

fn arb_node_record() -> impl Strategy<Value = NodeRecord> {
    (
        any::<u64>(),
        prop::collection::vec(any::<u32>(), 0..8),
        prop::collection::vec((any::<u32>(), arb_property_value()), 0..8),
        // ... more fields ...
    )
        .prop_map(|(id, labels, properties, ...)| NodeRecord { ... })
}
```

**Invariant Tests**:
```rust
proptest!(
    #[test]
    fn prop_btree_insert_search_consistency(record in arb_node_record()) {
        let mut store = NodeStore::new(1);
        let id = store.create_node(record.labels.clone(), record.properties.clone());
        let retrieved = store.get_node(id).expect("stored");
        prop_assert_eq!(retrieved.labels, record.labels);
        prop_assert_eq!(retrieved.properties, record.properties);
    }
);
```

### Integration Tests

**File**: `cypherlite-query/tests/subgraph.rs`

**Setup Pattern**:
```rust
#[test]
fn test_create_snapshot_basic() {
    let dir = tempdir().expect("tempdir");
    let config = DatabaseConfig {
        path: dir.join("test.cyl"),
        wal_sync_mode: SyncMode::Normal,
        ..Default::default()
    };
    let mut engine = StorageEngine::open(config).expect("open");
    
    // Test operations
    engine.create_node(vec![1], vec![(1, PropertyValue::String("Alice".into()))]);
    // ...
}
```

### Test File Organization

**Storage Tests** (`crates/cypherlite-storage/tests/`):
- `crud_operations.rs` - Node/edge create/read/update/delete
- `acid_compliance.rs` - Transaction consistency
- `concurrency.rs` - Multi-thread safety
- `proptest_storage.rs` - Property-based invariants

**Query Tests** (`crates/cypherlite-query/tests/`):
- `subgraph.rs` - Subgraph CRUD + queries
- `proptest_subgraph.rs` - Property-based subgraph tests
- `temporal_query.rs` - AT TIME queries
- `version_storage.rs` - Version snapshots + retrieval

### Benchmarks

**File**: `cypherlite-query/benches/subgraph.rs` (required-features = ["subgraph"])

```rust
#[cfg(feature = "subgraph")]
fn bench_subgraph_creation(c: &mut Criterion) {
    c.bench_function("create_1000_subgraphs", |b| {
        b.iter(|| {
            let mut engine = test_engine();
            for _ in 0..1000 {
                engine.create_subgraph(vec![], None);
            }
        });
    });
}
```

### For SPEC-DB-007 Tests

**Structure to Create**:

1. **Unit tests** (within source files):
   ```rust
   #[cfg(feature = "hypergraph")]
   mod hypergraph_tests {
       // Test HyperEdgeId, HyperEdgeRecord, GraphEntity extensions
   }
   ```

2. **Integration tests** (new file):
   - `tests/hypergraph.rs` - HYPEREDGE CRUD
   - `tests/proptest_hypergraph.rs` - Property-based invariants

3. **Benchmarks** (new file):
   - `benches/hypergraph.rs` (required-features = ["hypergraph"])

4. **Test patterns**:
   - All wrapped in `#[cfg(feature = "hypergraph")]`
   - Use tempdir() for StorageEngine creation
   - Proptest for invariant validation
   - Criterion for performance baseline

---

## Research Area 10: Existing Edge Store Patterns

### EdgeStore Structure

**File**: `/Users/epsilondelta/epsilonDelta/projects/CypherLite/crates/cypherlite-storage/src/btree/edge_store.rs` (450+ lines)

```rust
pub struct EdgeStore {
    tree: BTree<u64, RelationshipRecord>,
    next_id: u64,
}
```

**CRUD API**:
- `pub fn create_edge(start: NodeId, end: NodeId, rel_type_id: u32, props: Vec<...>, node_store: &mut NodeStore) -> Result<EdgeId>` - Allocate and maintain adjacency chains
- `pub fn get_edge(&self, edge_id: EdgeId) -> Option<&RelationshipRecord>` - Direct lookup
- `pub fn update_edge(&mut self, edge_id: EdgeId, properties: Vec<...>) -> Result<()>` - Update properties
- `pub fn get_edges_for_node(&self, node_id: NodeId, node_store: &NodeStore) -> Vec<&RelationshipRecord>` - Query by node
- `pub fn delete_edge(&mut self, edge_id: EdgeId, node_store: &mut NodeStore) -> Result<RelationshipRecord>` - Delete + cleanup

### Adjacency Chain Management

**Pattern** (lines 30-77):
1. Get current head from source node: `start_record.next_edge_id`
2. Create new edge with old head as `next_out_edge`
3. Update source node's `next_edge_id` to new edge
4. Prepend to adjacency chain (LIFO stack)

```rust
let start_record = node_store.get_node(start_node)...;
let prev_out_edge = start_record.next_edge_id;

let record = RelationshipRecord {
    edge_id,
    start_node,
    end_node,
    rel_type_id,
    direction: Direction::Outgoing,
    next_out_edge: prev_out_edge,    // Link to old head
    // ...
};

self.tree.insert(edge_id.0, record);
node_store.set_next_edge(start_node, Some(edge_id))?;  // Update head
```

### Direction & Bipartite Edges

**RelationshipRecord Fields** (lines 54-67):
```rust
pub edge_id: EdgeId,
pub start_node: NodeId,
pub end_node: NodeId,
pub rel_type_id: u32,
pub direction: Direction,
pub next_out_edge: Option<EdgeId>,      // Outgoing chain
pub next_in_edge: Option<EdgeId>,       // Incoming chain
pub properties: Vec<(u32, PropertyValue)>,
#[cfg(feature = "subgraph")]
pub start_is_subgraph: bool,
#[cfg(feature = "subgraph")]
pub end_is_subgraph: bool,
```

**Direction Enum** (types.rs):
```rust
pub enum Direction {
    Outgoing,
    Incoming,
    Both,
}
```

### Edge Indexing

**EdgeIndexManager** (`src/index/edge_index.rs` lines 84-120):
```rust
pub struct EdgeIndexManager {
    /// Map from index name to (definition, edge_property_index).
    indexes: HashMap<String, (EdgeIndexDefinition, EdgePropertyIndex)>,
}

pub fn create_index(&mut self, name: String, rel_type_id: u32, prop_key_id: u32) -> Result<()> {
    let def = EdgeIndexDefinition { name: name.clone(), rel_type_id, prop_key_id };
    self.indexes.insert(name, (def, EdgePropertyIndex::new()));
    Ok(())
}

pub fn find_index_mut(&mut self, rel_type_id: u32, prop_key_id: u32)
    -> Option<&mut EdgePropertyIndex>
{
    // Find index matching rel_type_id + prop_key_id
}
```

### Constraints & Maintenance

**Delete Adjacency Chain Update** (lines 140-160):
- If deleted edge was head of chain, update node's next_edge_id to deleted.next_edge
- Walk chain and update next pointers if needed
- Clean up node store references

### For HyperEdgeStore

**Cannot directly reuse** EdgeStore pattern because:
1. Edges are binary (source → target)
2. Hyperedges are n-ary (multiple sources → multiple targets)
3. Adjacency chains work for binary edges, not n-ary

**However, can adopt**:
1. BTree<u64, HyperEdgeRecord> pattern
2. Auto-increment ID allocation
3. Dual indexes (forward/reverse) like MembershipIndex
4. PropTest-based invariant validation

---

## Dependency Maps

### Module Dependency Graph

```
cypherlite-core (types, config, traits)
    ↓ (used by)
cypherlite-storage
    ├─ btree/{node_store, edge_store, property_store}
    ├─ subgraph/{mod, membership}
    ├─ version/mod
    ├─ index/{edge_index, ...}
    ├─ page/{page_manager, buffer_pool}
    ├─ wal/{reader, writer, recovery, checkpoint}
    └─ transaction/mvcc
         ↓ (used by)
cypherlite-query
    ├─ lexer/mod
    ├─ parser/{ast, clause, pattern, expression, mod}
    ├─ semantic/{symbol_table, mod}
    ├─ planner/{mod, optimize}
    └─ executor/{mod, operators/*, eval}
```

### Feature Dependency Chain (Compilation)

```
temporal-core (base, always enabled by default)
    ↓ enables (pulls in all items below)
    - PropertyValue::DateTime
    - TemporalEdgeRecord
    - Version store capability
    ↓
temporal-edge (v0.5)
    ↓ enables
    - TemporalPredicate in AST
    - AsOfScan, TemporalRangeScan in LogicalPlan
    - Temporal executor operators
    ↓
subgraph (v0.6) ← CURRENT
    ↓ enables
    - SubgraphId, SubgraphRecord in types
    - GraphEntity enum
    - SubgraphStore module
    - MembershipIndex
    - CreateSnapshotClause in parser
    - SubgraphScan, CreateSnapshotOp in planner
    - Value::Subgraph in executor
    ↓
hypergraph (v0.7) ← TARGET
    ↓ enables
    - HyperEdgeId, HyperEdgeRecord in types
    - Extend GraphEntity with HyperEdge variant
    - HyperEdgeStore module
    - CreateHyperedgeClause in parser
    - HyperEdgeCreateOp in planner
    - Value::Hyperedge in executor
    ↓
full-temporal (combines all above)
```

### Cross-Crate Call Paths

**Example: MATCH (sg:Subgraph)**

```
cypherlite-query/parser/mod.rs (parse_statement)
  ↓
  parse_pattern()
  ↓
  GraphPattern { variable, entities: [(var_name, node_pattern)] }
  ↓
cypherlite-query/planner/mod.rs (plan_pattern)
  ↓
  Check entity.label == "Subgraph"? Yes
  ↓
  Create LogicalPlan::SubgraphScan { variable }
  ↓
cypherlite-query/executor/mod.rs (execute)
  ↓
  match plan { LogicalPlan::SubgraphScan { variable } }
  ↓
  execute_subgraph_scan(variable, engine)
  ↓
  engine.scan_subgraphs() ← StorageEngine method
  ↓
cypherlite-storage/lib.rs (scan_subgraphs)
  ↓
  self.subgraph_store.all()
  ↓
cypherlite-storage/subgraph/mod.rs (SubgraphStore::all)
  ↓
  Returns iterator over BTreeMap values
```

---

## Risks & Constraints Discovered

### Risk 1: Header Page Overflow

**Severity**: Medium | **Likelihood**: Low

**Issue**: v5 fields (bytes 64-79) use reserved space. If header grows beyond 80 bytes, would need to expand or reorganize.

**Mitigation**:
- Current: 4096-byte page size, using ~80 bytes
- Plenty of room for future extensions (up to 4096 bytes available)
- Recommendation: Comment future expansion plan in header structure

### Risk 2: In-Memory SubgraphStore Not Persisted

**Severity**: High | **Likelihood**: High

**Issue**: SubgraphStore currently uses in-memory BTreeMap, not persisted to disk. On restart, subgraphs are lost.

**Mitigation**:
- Document as limitation in SPEC
- Recommend for SPEC-DB-008: Persist SubgraphStore to B-tree pages
- HyperEdgeStore should follow same pattern (persist or not-persist consistently)

### Risk 3: Adjacency Chain Complexity for N-ary Hyperedges

**Severity**: Medium | **Likelihood**: Medium

**Issue**: Standard adjacency chain (linked list) works for binary edges. Hyperedges with arbitrary sources/targets don't fit this model.

**Mitigation**:
- Don't use adjacency chains for hyperedges
- Use dual indexes (forward: source → targets, reverse: target → sources)
- Pattern already proven in MembershipIndex

### Risk 4: GraphEntity Serialization in Properties

**Severity**: Low | **Likelihood**: Low

**Issue**: Graph entities (Node, Subgraph, HyperEdge) cannot be stored as PropertyValue. But code explicitly returns error for this case.

**Mitigation**:
- Current behavior is correct (entities stored separately, not as properties)
- Already tested and validated
- No action needed

### Risk 5: Feature Flag Ordering

**Severity**: Low | **Likelihood**: Low

**Issue**: If developer enables `hypergraph` without enabling `temporal-edge`, compilation will fail with unclear errors.

**Mitigation**:
- Cargo.toml dependencies enforce correct ordering
- Feature flags in Cargo.toml already correctly chained
- No additional action needed

### Constraint 1: BTree Pages Not Yet Implemented

**Issue**: BTree module claims page-based storage, but uses in-memory BTreeMap.

**Impact**: HyperEdgeStore will also be in-memory initially (consistent with subgraph).

**Recommendation**: Document as "Phase 1: in-memory storage; Phase 2: page-based persistence" in SPEC.

### Constraint 2: No Distributed Transactions

**Issue**: StorageEngine is single-threaded (not Send/Sync in current form).

**Impact**: HyperEdge operations cannot be atomic across multiple entities without wrapping in TransactionManager.

**Recommendation**: Use existing TransactionManager for multi-entity hyperedge operations.

### Constraint 3: Subgraph Membership Not Bidirectional

**Issue**: add_member/remove_member require manual membership maintenance. No automatic updates when nodes deleted.

**Impact**: Deleting a node leaves dangling references in subgraph memberships.

**Recommendation**: Design HyperEdgeStore to not reference nodes directly; use edge IDs instead. Or implement cleanup cascade.

---

## Implementation Approach Recommendations

### Phase 1: Core Storage (MVP)

1. **Extend types.rs**:
   - Add HyperEdgeId newtype
   - Add HyperEdgeRecord struct with Vec<GraphEntity> sources/targets
   - Extend GraphEntity enum with HyperEdge variant
   - Add to feature flags

2. **Add HyperEdgeStore module**:
   - In-memory BTreeMap<u64, HyperEdgeRecord>
   - CRUD operations matching SubgraphStore pattern
   - Add/remove dual indexes
   - Integrate with StorageEngine

3. **Update DatabaseHeader**:
   - Add hyperedge_root_page, next_hyperedge_id fields (v5)
   - Update serialization/deserialization
   - Add FLAG_HYPERGRAPH feature flag

4. **Tests**:
   - Unit tests within source files
   - Proptest invariants for dual index consistency
   - Integration tests for hyperedge CRUD

### Phase 2: Query Support

1. **Lexer**: Add HYPEREDGE, SOURCES, TARGETS keywords

2. **Parser**: Add CreateHyperedgeClause AST node

3. **Planner**: Add CreateHyperedgeOp LogicalPlan variant

4. **Executor**: 
   - Implement execute_create_hyperedge_op()
   - Add Value::Hyperedge variant
   - Implement MATCH (he:Hyperedge) pattern support

5. **Tests**: Integration tests for HYPEREDGE queries

### Phase 3: Temporal References (Optional v0.7.5)

1. **Add TemporalRef** concept to types
2. **Extend HyperEdgeRecord** with optional temporal_anchor
3. **Query support**: `(he:Hyperedge AT TIME expr)`
4. **Tests**: Temporal hyperedge queries

---

## Summary of Key Findings

| Area | Finding | Impact |
|------|---------|--------|
| GraphEntity | Extensible, solves node+subgraph+hyperedge need | **High** - Core abstraction reusable |
| SubgraphStore | Working reference implementation, in-memory | **High** - Template for HyperEdgeStore |
| B-Tree | Flexible, page-ready, currently in-memory | **Medium** - HyperEdgeStore can follow same pattern |
| Query Pipeline | Lexer→Parser→AST→Planner→Executor chain proven | **High** - Straightforward extension path |
| Features | Correctly chained, v5 space available | **High** - No technical blockers |
| Header v5 | 16 bytes available (64-79), room for growth | **High** - Sufficient space |
| Serialization | Variable-length Vec patterns proven with PropertyValue | **High** - Vec<GraphEntity> serializes cleanly |
| VersionStore | Captures snapshots per entity with timestamps | **High** - Foundation for TemporalRef |
| Tests | Proptest + integration test patterns established | **High** - Test framework ready |
| EdgeStore | Binary edge pattern; hyperedges need dual indexes | **Medium** - Adapt MembershipIndex pattern instead |

---

## Files to Modify for Implementation

### Core Types (cypherlite-core/src/types.rs)
- Add HyperEdgeId(pub u64)
- Add HyperEdgeRecord struct
- Extend GraphEntity enum
- Add tests with #[cfg(feature = "hypergraph")]

### Storage (cypherlite-storage/src/)
- New: `subgraph/hyperedge.rs` and `subgraph/hyperedge_index.rs` (or `hyperedge/mod.rs`)
- `lib.rs`: Add HyperEdgeStore field, extend StorageEngine API
- `page/mod.rs`: Extend DatabaseHeader for v5

### Query (cypherlite-query/src/)
- `lexer/mod.rs`: Add keywords
- `parser/ast.rs`: Add CreateHyperedgeClause
- `parser/mod.rs`: Add parse_create_hyperedge_clause()
- `planner/mod.rs`: Add CreateHyperedgeOp variant
- `executor/mod.rs`: Add Value::Hyperedge, extend execute match
- New: `executor/operators/hyperedge_create.rs` (or extend create.rs)

### Tests
- New: `cypherlite-storage/tests/hypergraph.rs`
- New: `cypherlite-storage/tests/proptest_hypergraph.rs`
- New: `cypherlite-query/tests/hypergraph.rs`
- New: `cypherlite-query/benches/hypergraph.rs` (with required-features)

---

**Research Complete. Ready for SPEC-DB-007 implementation planning.**
