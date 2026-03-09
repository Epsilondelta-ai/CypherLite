# CypherLite Master Technical Design Overview

**Document Date**: March 10, 2026
**Version**: 1.0 (Design Phase)
**Status**: Comprehensive Architecture & Implementation Roadmap
**Audience**: Architects, Lead Engineers, Project Stakeholders

---

## Executive Summary

CypherLite is a **lightweight, embedded, single-file graph database** designed to bring SQLite-like simplicity and deployment to the graph database ecosystem. It combines:

- **SQLite's architectural principles**: Single-file storage, page-based B-trees, ACID transactions via WAL
- **Neo4j's graph semantics**: Native property graphs, Cypher query language, index-free adjacency
- **Modern optimization**: Multi-level caching, query planning, temporal support, extensible plugin architecture
- **Embedded focus**: Zero-config setup, minimal dependencies, <50MB binary size target

CypherLite fills a critical gap: the absence of a lightweight, native graph database for edge computing, IoT, mobile applications, agent memory systems, and local-first computing. It enables developers to embed powerful graph querying directly into their applications without server setup, complex dependencies, or large resource footprints.

---

## 1. Project Vision & Goals

### 1.1 Problem Statement

The modern application landscape demands graph database capabilities in environments where traditional systems (Neo4j, Kùzu, etc.) are impractical:

- **Edge Computing**: IoT gateways, autonomous vehicles, and edge servers need queryable state without cloud dependency
- **LLM Agent Memory**: AI agents require persistent, structured, queryable memory for multi-turn reasoning and decision tracking
- **Mobile/Embedded**: Applications need rich data relationships without complex server setup
- **Local-First Computing**: Privacy-conscious applications require on-device graph processing
- **Development & Testing**: Teams need quick graph prototyping without database infrastructure

**The Gap**: No production-ready, embedded, single-file graph database with native Cypher support exists. SQLite dominates embedded relational databases, but graph queries in SQLite are cumbersome. Alternatives (KùzuDB, DuckDB) require complex setup or lack native graph semantics.

### 1.2 Target Users & Use Cases

#### Primary Use Cases
1. **LLM Agent Memory Systems**
   - Agents store decisions, interactions, and learned state in queryable graphs
   - Fast retrieval of relevant context across multi-turn conversations
   - Temporal queries to track decision evolution

2. **Knowledge Graph Applications**
   - Semantic search over domain knowledge (GraphRAG, enterprise knowledge bases)
   - Relationship discovery and reasoning
   - Integration with RAG pipelines for enhanced LLM prompting

3. **Edge Computing & IoT**
   - Local sensor data with relationship context (sensor A depends on sensor B)
   - Embedded analytics without cloud connectivity
   - Autonomous decision-making based on local state

4. **Application-Embedded Graphs**
   - Social networks embedded in mobile apps
   - Personal finance (transaction graph, merchant relationships)
   - Recommendation engines with relationship context

5. **Testing & Development**
   - Fast graph prototyping without infrastructure
   - Behavioral testing with graph assertions
   - Local development environments

#### Secondary Use Cases
- GraphRAG pipeline components (local knowledge extraction)
- Semantic layer for enterprise applications
- Temporal data warehouses with relationship context
- Federated graph systems (local processing before sync)

### 1.3 Design Principles

**ZERO-CONFIG**: Developers should create a graph with a single function call. No network, no configuration files, no schema migration scripts.

```rust
// That's it. No server setup.
let db = CypherLite::open("app.cyl")?;
```

**SINGLE-FILE**: The entire database (data, indices, WAL) fits in one `.cyl` file. Backup by copying one file. Deploy by distributing one file.

**ACID COMPLIANT**: Full ACID guarantees via WAL (Write-Ahead Logging). Crash recovery is automatic. Concurrent readers work while writes are pending.

**EMBEDDED**: Runs in-process as a Rust library. Available via FFI for Python, Node.js, C/C++. No server processes, no external dependencies.

**EXTENSIBLE**: Plugin architecture enables domain-specific layers (semantic layer, vector indices, custom procedures) without core bloat.

**TEMPORAL-NATIVE**: Time-aware queries built into storage engine. Track entity history, query state at specific timestamps, analyze temporal patterns.

**CYPHER-STANDARD**: Implement openCypher subset for developer familiarity. Path toward GQL compatibility for standards alignment.

---

## 2. Architecture Summary

### 2.1 High-Level Architecture Diagram

```
┌─────────────────────────────────────────────────────────────────┐
│                    APPLICATION LAYER                            │
│  (Python/Node.js/C FFI bindings)                                │
└────────────────────────┬────────────────────────────────────────┘
                         │
┌────────────────────────▼────────────────────────────────────────┐
│                      API LAYER                                   │
│  ┌──────────────────┐  ┌──────────────────┐  ┌────────────────┐ │
│  │  Cypher API      │  │  Native API      │  │ Connection Pool│ │
│  │ (MATCH/CREATE)   │  │ (Direct node/rel)│  │ (R+W mgmt)     │ │
│  └──────────────────┘  └──────────────────┘  └────────────────┘ │
└────────────────────────┬────────────────────────────────────────┘
                         │
┌────────────────────────▼────────────────────────────────────────┐
│                    QUERY ENGINE                                  │
│  ┌─────────────┐  ┌─────────────┐  ┌──────────────────────────┐ │
│  │ Lexer       │  │ Parser      │  │ Logical Plan            │ │
│  │ (Tokenize)  │  │ (AST)       │  │ (Cost-based optimization)│ │
│  └─────────────┘  └─────────────┘  └──────────────────────────┘ │
│  ┌──────────────────────┐  ┌──────────────────────────────────┐ │
│  │ Physical Executor    │  │ Row Iterator / Result Streaming  │ │
│  │ (Node/Edge scan)     │  │ (Incremental result delivery)    │ │
│  └──────────────────────┘  └──────────────────────────────────┘ │
└────────────────────────┬────────────────────────────────────────┘
                         │
┌────────────────────────▼────────────────────────────────────────┐
│                   STORAGE LAYER                                  │
│  ┌──────────────────────┐  ┌──────────────────┐                 │
│  │ Buffer Pool Manager  │  │ Page Cache (LRU) │                 │
│  │ (Page allocation)    │  │ (Hot data)       │                 │
│  └──────────────────────┘  └──────────────────┘                 │
│  ┌──────────────────────────────────────────────────────────┐   │
│  │         Core Storage Structures                          │   │
│  │  • Node Store (B-tree of pages)                         │   │
│  │  • Edge Store (Adjacency chains)                        │   │
│  │  • Property Store (Overflow handling)                   │   │
│  │  • Index Pages (B-tree indices, Label scan)            │   │
│  │  • Free Space Map (Page allocation tracking)           │   │
│  └──────────────────────────────────────────────────────────┘   │
└────────────────────────┬────────────────────────────────────────┘
                         │
┌────────────────────────▼────────────────────────────────────────┐
│                  TRANSACTION LAYER                               │
│  ┌─────────────────────┐  ┌─────────────────────────────────┐   │
│  │ WAL (Write-Ahead)   │  │ Transaction Manager (MVCC)      │   │
│  │ • Frame recording   │  │ • Isolation levels              │   │
│  │ • Checkpointing     │  │ • Snapshot consistency          │   │
│  └─────────────────────┘  └─────────────────────────────────┘   │
└────────────────────────┬────────────────────────────────────────┘
                         │
┌────────────────────────▼────────────────────────────────────────┐
│                   FILE SYSTEM LAYER                              │
│  ┌─────────────────────┐  ┌─────────────────────────────────┐   │
│  │   Primary File      │  │   WAL File                      │   │
│  │   (app.cyl)         │  │   (app.cyl-wal)                 │   │
│  │   All committed     │  │   Uncommitted transactions      │   │
│  │   data + indices    │  │   Memory index for frames       │   │
│  └─────────────────────┘  └─────────────────────────────────┘   │
└────────────────────────────────────────────────────────────────┘

Plugin System (Orthogonal to above):
┌────────────────────────────────────────────────────────────────┐
│  Storage Plugins │ Index Plugins │ Query Plugins │ Event Hooks  │
│  (Custom format) │ (Vector, FT)  │ (Procedures)  │ (Validation) │
└────────────────────────────────────────────────────────────────┘
```

### 2.2 Key Design Decisions & Rationale

| Decision | Rationale | Trade-off |
|----------|-----------|-----------|
| **Single file format** | Simplicity, atomic backup, portable | Concurrent writes limited |
| **Page-based storage (4KB)** | Cache efficiency, proven by SQLite | Fixed overhead per page |
| **Index-free adjacency** | O(1) traversal, no index overhead | More pointer dereferencing |
| **WAL for transactions** | Concurrent readers, crash recovery | Extra write complexity |
| **Cypher subset v1.0** | Rapid delivery, 80% use cases | Some advanced queries unsupported |
| **Minimal core + plugins** | Flexibility, avoids bloat | Plugin management complexity |
| **Temporal as v0.4 feature** | Proven storage model first | Delayed temporal capability |
| **Rust implementation** | Safety, performance, FFI support | Learning curve for team |

### 2.3 Technology Stack

#### Core Language & Runtime
- **Primary**: Rust 1.70+
  - Memory safety without GC
  - Excellent performance and FFI capabilities
  - Strong ecosystem for embedded systems

#### Key Crates
- **Storage**:
  - `parking_lot` — Fast RwLock for concurrent access
  - `crossbeam` — Channel primitives for work distribution
  - `dashmap` — Concurrent hashmap for page cache

- **Parsing & AST**:
  - `logos` — Hand-optimized lexer generation
  - Hand-coded recursive descent parser (no external dependency)

- **Temporal**:
  - `chrono` — Date/time operations
  - `time` — Duration management

- **Data Serialization**:
  - `bincode` — Binary encoding for internal structures
  - `serde` — Framework for plugin configuration

- **FFI & Bindings**:
  - `cbindgen` — C header generation from Rust
  - PyO3 (planned) — Python bindings
  - `neon` (planned) — Node.js bindings

- **Testing**:
  - `criterion` — Benchmark framework
  - `proptest` — Property-based testing
  - `tempfile` — Temporary files for tests

---

## 3. Component Summary Matrix

| Component | Design Doc | Responsibility | Status | Priority |
|-----------|-----------|-----------------|--------|----------|
| **Core Architecture** | `01_core_architecture.md` | System design, module structure, concurrency model | v1.0 | CRITICAL |
| **Storage Engine** | `02_storage_engine.md` | File format, page structures, B-trees, WAL, recovery | v1.0 | CRITICAL |
| **Query Engine** | `03_query_engine.md` | Cypher parsing, AST, logical/physical planning, execution | v1.0 | CRITICAL |
| **Plugin Architecture** | `04_plugin_architecture.md` | Extension mechanism, trait system, lifecycle | v0.3 | HIGH |
| **Existing Tech Research** | `01_existing_technologies.md` | Landscape analysis, architecture learnings | Research | REFERENCE |
| **Cypher & RDF Research** | `02_cypher_rdf_temporal.md` | Cypher spec, RDF semantics, temporal models | Research | REFERENCE |
| **GraphRAG & Agent Research** | `03_graphrag_agent_usecases.md` | Use case validation, agent memory requirements | Research | REFERENCE |

### Legend
- **Status**: v1.0 (implement now), v0.X (design only, implement later), Research (reference only)
- **Priority**: CRITICAL (blocks release), HIGH (should have), MEDIUM (nice to have), REFERENCE (context)

---

## 4. Data Flow Analysis

### 4.1 Query Processing Pipeline (Read Path)

```
User Query (Cypher String)
    │
    ▼
┌─────────────────────────────────┐
│ 1. LEXICAL ANALYSIS             │
│ • Tokenize input string         │
│ • Identify keywords, operators  │
│ • Handle literals, variables    │
│ Output: Token stream            │
└────────────┬────────────────────┘
             │
             ▼
┌─────────────────────────────────┐
│ 2. SYNTAX ANALYSIS (PARSING)    │
│ • Recursive descent parser      │
│ • Build Abstract Syntax Tree    │
│ • Validate clause combinations  │
│ Output: AST                     │
└────────────┬────────────────────┘
             │
             ▼
┌─────────────────────────────────┐
│ 3. SEMANTIC ANALYSIS            │
│ • Scope resolution (var binding)│
│ • Type inference                │
│ • Validate function calls       │
│ Output: Decorated AST           │
└────────────┬────────────────────┘
             │
             ▼
┌─────────────────────────────────┐
│ 4. LOGICAL PLANNING             │
│ • Convert AST to logical ops    │
│ • Identify optimization points  │
│ • Estimate cardinalities        │
│ Output: Logical Query Plan      │
└────────────┬────────────────────┘
             │
             ▼
┌─────────────────────────────────┐
│ 5. COST OPTIMIZATION            │
│ • Evaluate alternative plans    │
│ • Join order selection          │
│ • Index selection               │
│ • Filter pushdown               │
│ Output: Best Physical Plan      │
└────────────┬────────────────────┘
             │
             ▼
┌─────────────────────────────────┐
│ 6. CODE GENERATION              │
│ • Compile plan to execution ops │
│ • Allocate cursor variables     │
│ • Prepare storage layer calls   │
│ Output: Executable Plan         │
└────────────┬────────────────────┘
             │
             ▼
┌─────────────────────────────────┐
│ 7. EXECUTION                    │
│ • Iterator/Pull-based model    │
│ • Fetch rows from storage       │
│ • Apply filters, projections    │
│ • Stream results to client      │
│ Output: Result rows             │
└────────────┬────────────────────┘
             │
             ▼
        Result Set
```

**Example Query**:
```cypher
MATCH (a:Person)-[:KNOWS]->(b)
WHERE a.age > 25
RETURN a.name, b.name
LIMIT 10
```

**Execution Flow**:
1. Lexer → Token stream: `[MATCH, LPAREN, a, COLON, Person, RPAREN, ...]`
2. Parser → AST: `MatchClause { patterns: [...], where: ... }`
3. Semantic → Validate `a` and `b` bound in MATCH before RETURN
4. Logical Plan:
   ```
   Limit(10,
     Project([a.name, b.name],
       Filter(a.age > 25,
         Expand(a, KNOWS, b))))
   ```
5. Physical Plan (add index selection, cardinality estimation)
6. Execution:
   - Scan Person nodes with label index
   - For each a, fetch outgoing KNOWS relationships
   - Filter by a.age > 25
   - Read target node b
   - Project name properties
   - Return up to 10 rows

### 4.2 Write Path (CREATE/MERGE → Disk)

```
User Mutation (Cypher String: CREATE/MERGE/SET/DELETE)
    │
    ▼
┌──────────────────────────────────────────┐
│ 1-6: SAME AS READ PATH (Parsing → Plan)  │
└──────────────────┬───────────────────────┘
                   │
                   ▼
┌──────────────────────────────────────────┐
│ 7. VALIDATION & PLANNING                 │
│ • Semantic layer validation (if enabled) │
│ • Constraint checking                    │
│ • Plan memory allocations                │
└──────────────────┬───────────────────────┘
                   │
                   ▼
┌──────────────────────────────────────────┐
│ 8. TRANSACTION BEGIN                     │
│ • Allocate transaction ID                │
│ • Acquire write lock (single writer)     │
│ • Create transaction context             │
└──────────────────┬───────────────────────┘
                   │
                   ▼
┌──────────────────────────────────────────┐
│ 9. EXECUTE MUTATIONS                     │
│ ┌─────────────────────────────────────┐  │
│ │ For CREATE:                         │  │
│ │ • Allocate node/relationship IDs    │  │
│ │ • Add to in-memory node map         │  │
│ │ • Update adjacency chains           │  │
│ │ • Mark index pages dirty            │  │
│ └─────────────────────────────────────┘  │
│ ┌─────────────────────────────────────┐  │
│ │ For MERGE:                          │  │
│ │ • Execute pattern match (see above) │  │
│ │ • On match: Execute ON MATCH SET   │  │
│ │ • On no match: CREATE + ON CREATE  │  │
│ └─────────────────────────────────────┘  │
│ ┌─────────────────────────────────────┐  │
│ │ For SET/REMOVE:                     │  │
│ │ • Update property values in memory  │  │
│ │ • Mark entities dirty               │  │
│ └─────────────────────────────────────┘  │
└──────────────────┬───────────────────────┘
                   │
                   ▼
┌──────────────────────────────────────────┐
│ 10. EVENT DISPATCH (Plugin Hooks)        │
│ • Trigger Before* events                 │
│ • Allow plugins to intercept/veto        │
│ • Collect mutations from callbacks       │
└──────────────────┬───────────────────────┘
                   │
                   ▼
┌──────────────────────────────────────────┐
│ 11. WAL WRITE (Write-Ahead Log)         │
│ • Serialize mutation pages               │
│ • Create WAL frames with checksums      │
│ • Write frames to .cyl-wal file         │
│ • Sync to disk (PRAGMA synchronous)     │
│ • Advance WAL head pointer              │
└──────────────────┬───────────────────────┘
                   │
                   ▼
┌──────────────────────────────────────────┐
│ 12. POST-MUTATION CLEANUP                │
│ • Invalidate affected cache entries      │
│ • Update index structures                │
│ • Trigger After* events                  │
│ • Collect downstream mutations           │
└──────────────────┬───────────────────────┘
                   │
                   ▼
┌──────────────────────────────────────────┐
│ 13. TRANSACTION COMMIT                   │
│ • Set commit timestamp in WAL             │
│ • Release write lock                      │
│ • Notify waiting readers                 │
└──────────────────┬───────────────────────┘
                   │
                   ▼
┌──────────────────────────────────────────┐
│ 14. CHECKPOINT (Periodic)                │
│ • When WAL reaches threshold (~1000 pages)
│ • Transfer WAL frames to primary file    │
│ • Update primary file headers            │
│ • Recycle WAL file                       │
└──────────────────┬───────────────────────┘
                   │
                   ▼
        Mutation persisted on disk
```

**Example Mutation**:
```cypher
CREATE (a:Person {name: 'Alice'})
CREATE (b:Person {name: 'Bob'})
CREATE (a)-[:KNOWS]->(b)
```

**Execution Sequence**:
1. Parse → Create three mutations
2. Transaction begin: Acquire write lock
3. Execute mutations:
   - Allocate node IDs: a=1001, b=1002
   - Create edge ID 5001, link a→b
   - Update page cache
4. Event hooks: Validate semantically if enabled
5. WAL write: Write 3 new frames to .cyl-wal
6. Cache invalidation: Mark label index for Person as dirty
7. Commit: Set commit timestamp
8. Checkpoint (if needed): Transfer frames to app.cyl

### 4.3 Temporal Query Path (Future: v0.4+)

```
Temporal Query: "Find all decisions active at 2024-01-15"

Query: MATCH (d:Decision)
       AT TIME datetime('2024-01-15')
       RETURN d

       │
       ▼
─────────────────────────────────────
Standard query path (1-6 as above)
─────────────────────────────────────
       │
       ▼
┌─────────────────────────────────────┐
│ TEMPORAL CONSTRAINT BINDING         │
│ • Extract time expression           │
│ • Convert to temporal range         │
│ • Add to query context              │
└─────────┬───────────────────────────┘
          │
          ▼
┌─────────────────────────────────────┐
│ TEMPORAL INDEX SELECTION            │
│ • Consult temporal B-tree           │
│ • Find nodes valid at timestamp     │
│ • Fetch versions from version store │
└─────────┬───────────────────────────┘
          │
          ▼
┌─────────────────────────────────────┐
│ EXECUTE WITH TEMPORAL VIEW          │
│ • Present historical snapshot       │
│ • Follow relationships at time      │
│ • Relationship validity checks      │
└─────────┬───────────────────────────┘
          │
          ▼
    Result: Decisions valid at time
```

**Temporal Storage Structure**:
- **Version Store**: Append-only log of entity versions
- **Temporal B-tree**: (Entity ID, timestamp) → version offset
- **Validity Range**: Each version has [valid_from, valid_to]
- **Relationship Snapshots**: Edge validity independent of node validity

---

## 5. API Design

### 5.1 Rust Native API Examples

```rust
// ============ BASIC USAGE ============

use cypherlite::{CypherLite, Value};

// Open or create database
let mut db = CypherLite::open("agents.cyl")?;

// -------- CYPHER QUERIES (Primary) --------

// Simple query
let results = db.query(
    "MATCH (n:Agent) RETURN n.name, n.status"
)?;
for row in results {
    println!("Agent: {}", row["n.name"]);
}

// Query with parameters
let results = db.query_with_params(
    "MATCH (a:Agent)-[:MANAGES]->(t:Task) \
     WHERE a.id = $agent_id \
     RETURN t.title",
    params! {"agent_id": 42}
)?;

// Mutation
db.execute(
    "CREATE (a:Agent {id: 1, name: 'Alice', status: 'active'})"
)?;

// MERGE (upsert)
db.execute(
    "MERGE (d:Decision {id: 100}) \
     ON CREATE SET d.created = timestamp() \
     ON MATCH SET d.updated = timestamp()"
)?;

// -------- NATIVE API (Performance-Critical) --------

use cypherlite::graph::{Node, Relationship};

// Create node directly (bypasses Cypher parser)
let agent_id = db.create_node(
    &["Agent"],
    vec![
        ("id", Value::Integer(1)),
        ("name", Value::String("Alice".to_string())),
        ("memory", Value::String("{}".to_string())),
    ],
)?;

// Create relationship
let task_id = db.create_node(&["Task"], vec![...])?;
let rel_id = db.create_relationship(
    agent_id,
    "MANAGES",
    task_id,
    vec![("since", Value::Integer(1708000000))],
)?;

// Query by ID (O(1) access)
let agent = db.get_node(agent_id)?;
println!("Agent: {:?}", agent);

// Get node properties
let name: String = agent.get_property("name")?;
let status: String = agent.get_property("status")?;

// List adjacent nodes (index-free adjacency)
let managed_tasks = db.expand_node(
    agent_id,
    Some("MANAGES"),  // Filter by relationship type
    cypherlite::Direction::Outgoing,
)?;

for (rel, task_id) in managed_tasks {
    let task = db.get_node(task_id)?;
    println!("Task: {}", task.get_property::<String>("title")?);
}

// -------- TRANSACTIONS --------

let mut tx = db.begin_transaction()?;

// Multiple operations within transaction
tx.create_node(&["Event"], vec![...])?;
tx.create_node(&["Log"], vec![...])?;
tx.create_relationship(/* ... */)?;

// Auto-rollback if not committed
tx.commit()?;  // Atomically persist all changes

// -------- COMPLEX PATTERNS --------

// Agent memory pattern: Update decision with reasoning
db.execute(
    "MATCH (a:Agent {id: $agent_id})-[:MADE]->(d:Decision {id: $decision_id}) \
     SET d.reasoning = $reasoning, d.confidence = $confidence, \
         d.updated_at = timestamp()"
,
    params! {
        "agent_id": 1,
        "decision_id": 100,
        "reasoning": "Based on analysis of options",
        "confidence": 0.85
    }
)?;

// Knowledge graph pattern: Find related entities
let results = db.query(
    "MATCH (e:Entity {name: 'Kubernetes'})-[:RELATED]->(r:Entity) \
     RETURN r.name, r.category \
     ORDER BY r.relevance DESC \
     LIMIT 10"
)?;

// Temporal pattern (future)
let results = db.query(
    "MATCH (d:Decision) \
     AT TIME datetime('2024-01-15') \
     WHERE d.status = 'active' \
     RETURN d.title"
)?;
```

### 5.2 Python Bindings Examples

```python
# ============ PYTHON API ============

from cypherlite import CypherLite, params

# Open database
db = CypherLite("agents.cyl")

# -------- CYPHER QUERIES --------

# Simple match
results = db.query("MATCH (n:Agent) RETURN n.name")
for row in results:
    print(f"Agent: {row['n.name']}")

# Parameterized query
results = db.query(
    "MATCH (a:Agent)-[:MANAGES]->(t:Task) "
    "WHERE a.id = $agent_id "
    "RETURN t.title",
    params(agent_id=42)
)

# Mutations
db.execute("CREATE (a:Agent {id: 1, name: 'Alice'})")

# MERGE with conditional sets
db.execute(
    "MERGE (d:Decision {id: 100}) "
    "ON CREATE SET d.created = timestamp() "
    "ON MATCH SET d.updated = timestamp()"
)

# -------- NATIVE API --------

# Create node
agent_id = db.create_node(
    labels=["Agent"],
    properties={
        "id": 1,
        "name": "Alice",
        "status": "active",
        "memory": {}
    }
)

# Get node
agent = db.get_node(agent_id)
print(f"Agent status: {agent['status']}")

# Update properties
db.set_property(agent_id, "last_action", "took_decision")

# List adjacent nodes
adjacent = db.expand_node(agent_id, rel_type="MANAGES")
for rel, target_id in adjacent:
    target = db.get_node(target_id)
    print(f"Manages: {target['title']}")

# -------- TRANSACTIONS --------

with db.transaction() as tx:
    tx.create_node(["Event"], {"type": "decision"})
    tx.create_node(["Log"], {"message": "decided"})
    # Auto-commit on successful exit

# -------- VECTOR SEARCH (Plugin) --------

# Requires vector_index plugin loaded
results = db.query(
    "MATCH (n:Document) "
    "WHERE vector.similarity(n.embedding, $query) > 0.8 "
    "RETURN n.text, vector.similarity(...) AS score "
    "ORDER BY score DESC "
    "LIMIT 5",
    params(query=[0.1, 0.2, 0.3, ...])  # 768-dim vector
)

# -------- SCHEMA VALIDATION (Plugin) --------

# Requires semantic_layer plugin
schema = db.get_schema()
print(schema.object_types())  # ['Agent', 'Task', 'Decision', ...]

# Create with validation
try:
    db.execute(
        "CREATE (a:Agent {id: 1, role: 'invalid_role'})"
    )
except db.ValidationError as e:
    print(f"Schema violation: {e}")
```

### 5.3 Node.js Bindings Examples

```javascript
// ============ NODE.JS API ============

const CypherLite = require("cypherlite");

const db = new CypherLite("agents.cyl");

// -------- CYPHER QUERIES --------

// Promise-based API
db.query("MATCH (n:Agent) RETURN n.name, n.status")
  .then(results => {
    results.forEach(row => {
      console.log(`Agent: ${row["n.name"]} (${row["n.status"]})`);
    });
  });

// Async/await
const results = await db.query(
  "MATCH (a:Agent {id: $agent_id})-[:MANAGES]->(t:Task) RETURN t.title",
  { agent_id: 42 }
);

// Mutations
await db.execute(
  "CREATE (a:Agent {id: 1, name: 'Alice', created: timestamp()})"
);

// -------- NATIVE API --------

// Create nodes/relationships directly
const agentId = await db.createNode(["Agent"], {
  id: 1,
  name: "Alice",
  status: "active"
});

const taskId = await db.createNode(["Task"], {
  title: "Analyze logs",
  priority: "high"
});

const relId = await db.createRelationship(
  agentId,
  "MANAGES",
  taskId,
  { since: Date.now() }
);

// -------- STREAMING & ITERATION --------

// Stream large result sets
const stream = db.stream(
  "MATCH (d:Decision) RETURN d.id, d.title"
);

stream.on("data", row => {
  console.log(`Processing decision: ${row.id}`);
});

stream.on("end", () => {
  console.log("All decisions processed");
});

// -------- TRANSACTIONS --------

const tx = await db.beginTransaction();
try {
  await tx.execute("CREATE (e:Event {type: 'test'})");
  await tx.execute("CREATE (l:Log {message: 'test'})");
  await tx.commit();
} catch (e) {
  await tx.rollback();
  throw e;
}

// -------- TEMPORAL QUERIES (Future) --------

const atTime = new Date("2024-01-15T00:00:00Z");
const results = await db.query(
  "MATCH (d:Decision) AT TIME $time RETURN d.title, d.status",
  { time: atTime }
);
```

### 5.4 C FFI Examples

```c
// ============ C API ============

#include <cypherlite.h>
#include <stdio.h>

int main() {
    // Open database
    CypherliteDB db = cypherlite_open("agents.cyl");
    if (!db) {
        fprintf(stderr, "Failed to open database\n");
        return 1;
    }

    // -------- CYPHER QUERIES --------

    // Execute query
    CypherliteResults results = cypherlite_query(
        db,
        "MATCH (n:Agent) RETURN n.id, n.name"
    );

    if (!results) {
        fprintf(stderr, "Query failed\n");
        cypherlite_close(db);
        return 1;
    }

    // Iterate results
    while (cypherlite_results_next(results)) {
        int64_t id = cypherlite_get_int(results, 0);
        const char* name = cypherlite_get_string(results, 1);
        printf("Agent %ld: %s\n", id, name);
    }
    cypherlite_results_free(results);

    // -------- MUTATIONS --------

    // Execute with error handling
    CypherliteError error = NULL;
    int ok = cypherlite_execute(
        db,
        "CREATE (a:Agent {id: 1, name: 'Alice'})",
        &error
    );

    if (!ok) {
        fprintf(stderr, "Mutation failed: %s\n",
                cypherlite_error_message(error));
        cypherlite_error_free(error);
    }

    // -------- NATIVE API --------

    // Create node directly
    uint64_t agent_id = cypherlite_create_node(db, "Agent", 2);
    cypherlite_set_property_int(db, agent_id, "id", 1);
    cypherlite_set_property_string(db, agent_id, "name", "Bob");

    // Get node property
    CypherliteValue name_val = cypherlite_get_property(
        db, agent_id, "name"
    );
    printf("Agent name: %s\n", cypherlite_value_as_string(name_val));

    // Create relationship
    uint64_t task_id = cypherlite_create_node(db, "Task", 1);
    cypherlite_create_relationship(
        db, agent_id, "MANAGES", task_id
    );

    // -------- TRANSACTIONS --------

    CypherliteTx tx = cypherlite_begin_tx(db, &error);
    if (!tx) {
        fprintf(stderr, "Failed to begin transaction\n");
        goto cleanup;
    }

    cypherlite_tx_execute(tx, "CREATE (e:Event {type: 'start'})", &error);
    cypherlite_tx_execute(tx, "CREATE (l:Log {msg: 'started'})", &error);

    if (error) {
        cypherlite_tx_rollback(tx);
        fprintf(stderr, "Transaction failed: %s\n",
                cypherlite_error_message(error));
    } else {
        cypherlite_tx_commit(tx, &error);
    }

cleanup:
    cypherlite_close(db);
    return 0;
}
```

### 5.5 Agent Memory Use Case (Practical Example)

```python
# Agent using CypherLite for structured memory

from cypherlite import CypherLite, params
from datetime import datetime

class AgentMemory:
    def __init__(self, agent_id: int):
        self.agent_id = agent_id
        self.db = CypherLite("agent_memory.cyl")

    def record_decision(self, title: str, reasoning: str, options: list):
        """Log a decision made by the agent"""
        decision_id = self.db.create_node(
            labels=["Decision"],
            properties={
                "agent_id": self.agent_id,
                "title": title,
                "reasoning": reasoning,
                "created": datetime.now().isoformat(),
                "status": "pending"
            }
        )

        # Store considered options
        for opt in options:
            opt_id = self.db.create_node(
                labels=["Option"],
                properties={"text": opt["description"], "score": opt["score"]}
            )
            self.db.create_relationship(
                decision_id, "CONSIDERS", opt_id
            )

        return decision_id

    def finalize_decision(self, decision_id: int, outcome: str):
        """Mark decision as completed"""
        self.db.set_property(decision_id, "status", "finalized")
        self.db.set_property(decision_id, "outcome", outcome)

    def find_relevant_context(self, topic: str, limit: int = 5):
        """Find relevant past decisions for context"""
        results = self.db.query(
            "MATCH (d:Decision)-[:CONSIDERS]->(o:Option) "
            "WHERE d.agent_id = $agent_id AND o.text CONTAINS $topic "
            "RETURN d.title, d.reasoning, d.outcome, d.created "
            "ORDER BY d.created DESC "
            "LIMIT $limit",
            params(agent_id=self.agent_id, topic=topic, limit=limit)
        )
        return results

    def get_recent_activity(self, hours: int = 24):
        """Get recent decisions for context window"""
        results = self.db.query(
            "MATCH (d:Decision) "
            "WHERE d.agent_id = $agent_id AND "
            "      timestamp() - datetime(d.created) < $hours * 3600000 "
            "RETURN d.title, d.status, d.outcome "
            "ORDER BY d.created DESC",
            params(agent_id=self.agent_id, hours=hours)
        )
        return results

    def analyze_patterns(self):
        """Identify decision patterns"""
        results = self.db.query(
            "MATCH (d:Decision) "
            "WHERE d.agent_id = $agent_id AND d.status = 'finalized' "
            "RETURN d.outcome, COUNT(*) as count "
            "ORDER BY count DESC",
            params(agent_id=self.agent_id)
        )
        return results

# Usage
memory = AgentMemory(agent_id=1)

# Agent makes a decision
decision_id = memory.record_decision(
    title="Scale database replicas",
    reasoning="High query latency detected",
    options=[
        {"description": "Add read replicas", "score": 0.9},
        {"description": "Increase cache", "score": 0.7},
        {"description": "Optimize queries", "score": 0.8}
    ]
)

# ... decision is executed ...

# Agent records outcome
memory.finalize_decision(decision_id, "Scaled to 3 replicas, latency reduced 40%")

# Next decision: Retrieve context
context = memory.find_relevant_context("database performance")
print("Relevant past decisions:", context)
```

---

## 6. File Format Summary

### 6.1 Single-File Architecture (.cyl)

```
┌────────────────────────────────────┐
│      CypherLite Database File      │
│          (app.cyl)                 │
├────────────────────────────────────┤
│  File Header (4 KB, Page 0)        │
│  • Magic: "CYLL" (4 bytes)        │
│  • Version: 1.0 (2 bytes)          │
│  • Page size: 4096 (2 bytes)       │
│  • Checksum: SHA256 of header      │
│  • Created timestamp               │
│  • Last checkpoint timestamp       │
│  • Total pages: N                  │
│  • Free space map location         │
│  • Primary index locations         │
├────────────────────────────────────┤
│  Metadata Pages (Pages 1-4)        │
│  • Schema definitions              │
│  • Label indices                   │
│  • Relationship type indices       │
│  • Plugin metadata                 │
│  • Configuration                   │
├────────────────────────────────────┤
│  Free Space Map (Pages 5-10)       │
│  • Bitmap of available pages       │
│  • Page allocation tracking        │
├────────────────────────────────────┤
│  Index Pages (Pages 11-...)        │
│  • B-tree for label index          │
│  • B-tree for type index           │
│  • B-tree for property indices     │
├────────────────────────────────────┤
│  Data Pages (Pages ...-N)          │
│  • Node records (B-tree pages)     │
│  • Edge/Relationship records       │
│  • Property pages (overflow)       │
│  • Temporal version records        │
├────────────────────────────────────┤
│  Plugin Storage Pages (Variable)   │
│  • Storage plugin allocations      │
│  • Vector index data (if enabled)  │
│  • Full-text index data            │
└────────────────────────────────────┘

File Size: Grows as data added (sparse allocation)
```

### 6.2 Write-Ahead Log (.cyl-wal)

```
┌────────────────────────────────────┐
│   Write-Ahead Log File             │
│       (app.cyl-wal)                │
├────────────────────────────────────┤
│  WAL Header (32 bytes)             │
│  • Magic: 0x377f0682 (little-endian)
│  • Format version                  │
│  • Checkpoint size (pages)         │
│  • Max frame index                 │
│  • Padding to 32 bytes             │
├────────────────────────────────────┤
│  Frame 1 (4KB page + header)       │
│  • Frame header (24 bytes):        │
│    - Page number (4 bytes)         │
│    - Commit marker (4 bytes)       │
│    - Checksum (4 bytes)            │
│    - Padding (12 bytes)            │
│  • Page content (4096 bytes)       │
│  • Frame checksum (4 bytes)        │
│                                     │
│  Frame 2, Frame 3, ...             │
├────────────────────────────────────┤
│  Commit Record (When needed)       │
│  • Marks end of transaction        │
│  • Timestamp                       │
│  • Transaction ID                  │
└────────────────────────────────────┘

Size: Grows with writes, truncated on checkpoint
Frames: Multiple pages can be dirty in one transaction
```

### 6.3 Page Structure (4KB)

**Generic Page Header (32 bytes)**:
```
Offset  Size  Field
0       4     Magic (0xCAFEBABE for data pages)
4       4     Page Type (0=data, 1=index, 2=overflow, ...)
8       4     Page ID (sequential number in file)
12      4     Parent Page ID (for tree navigation)
16      2     Number of records on page
18      2     Free space offset
20      4     Page LSN (Log Sequence Number for WAL)
24      4     CRC32 checksum
28      4     Padding
```

**Node Record (Variable length)**:
```
Offset  Size  Field
0       8     Node ID (8 bytes for large graphs)
8       4     First relationship pointer (PageID+offset)
12      4     First property pointer
16      2     Label bitmask or label page reference
18      2     Number of inline properties
20-31   N     Property slots (key-value pairs)
```

**Relationship Record**:
```
Offset  Size  Field
0       8     Relationship ID
8       8     Start node ID
16      8     End node ID
24      4     Type ID (enum)
28      4     First prev rel (for start node doubly-linked list)
32      4     First next rel
36      4     First prev rel (for end node)
40      4     First next rel
44      4     Property pointer
48      N     Property slots
```

**Index Page (B-tree interior/leaf)**:
```
If leaf:
  [Key1, RecordPtr1, Key2, RecordPtr2, ...]
If interior:
  [ChildPtr0, Key1, ChildPtr1, Key2, ChildPtr2, ...]
```

### 6.4 Temporal Extension (.cyl w/ temporal bit)

If temporal support enabled:
```
Additional Metadata Pages:
├─ Temporal configuration page
├─ Timestamp index (maps timestamps to versions)
└─ Version history store

Each node/relationship:
├─ Valid_from timestamp
├─ Valid_to timestamp (NULL = current)
└─ Version chain pointer
```

---

## 7. Implementation Roadmap

### Phase 1: Storage Engine (v0.1) — Weeks 1-4

**Goals**: Foundational storage, basic CRUD, no query language

**Deliverables**:
- [x] File format specification (in design docs)
- [ ] Page allocator & buffer pool
- [ ] B-tree implementation for node/edge storage
- [ ] WAL implementation with checkpointing
- [ ] Basic read/write transactions
- [ ] ACID compliance verification

**Key Milestones**:
- Create/read nodes with properties
- Create/read relationships (edges)
- Atomic transactions with rollback
- Crash recovery via WAL

**Testing**: Unit tests for storage primitives, crash recovery scenarios

**Estimated Effort**: 4 weeks (1 person) or 2 weeks (2 people)

---

### Phase 2: Cypher Query Engine (v0.2) — Weeks 5-10

**Goals**: Full Cypher parser, logical planning, basic execution

**Deliverables**:
- [ ] Lexer for Cypher tokenization
- [ ] Recursive descent parser → AST
- [ ] Semantic analysis (scope, type checking)
- [ ] Logical plan builder (convert AST to operators)
- [ ] Physical plan generation (basic)
- [ ] Basic execution (node scan, filter, expand)

**Key Milestones**:
- Parse and execute: `MATCH (n) RETURN n`
- Filter: `WHERE n.age > 25`
- Pattern matching: `(a)-[:KNOWS]->(b)`
- Basic aggregation: `COUNT(*)`

**Testing**: Parser unit tests, query execution tests on sample graphs

**Estimated Effort**: 6 weeks (2 people) or 12 weeks (1 person)

---

### Phase 3: Indexing & Optimization (v0.3) — Weeks 11-14

**Goals**: Label indices, query optimization, plugin architecture

**Deliverables**:
- [ ] Label scan index (B-tree on labels)
- [ ] Cost-based query plan optimizer
- [ ] Filter pushdown
- [ ] Index selection in planner
- [ ] Plugin trait definitions & registry
- [ ] Storage plugin interface
- [ ] Index plugin interface
- [ ] Query plugin interface

**Key Milestones**:
- Queries use label index to avoid full scans
- Multi-pattern queries optimized
- Plugin system can extend functionality
- First plugin: custom index type

**Testing**: Benchmark existing queries, verify optimization

**Estimated Effort**: 4 weeks

---

### Phase 4: Temporal Dimension (v0.4) — Weeks 15-18

**Goals**: Time-aware queries and storage

**Deliverables**:
- [ ] Temporal data model design
- [ ] Timestamp tracking in storage
- [ ] Version storage
- [ ] AT TIME syntax in Cypher
- [ ] Temporal index (B-tree with time range)
- [ ] Temporal query execution

**Key Milestones**:
- Store node versions with timestamps
- Query: `MATCH (n) AT TIME datetime('2024-01-15') RETURN n`
- Temporal relationship validity

**Testing**: Temporal correctness tests

**Estimated Effort**: 4 weeks

---

### Phase 5: FFI Bindings (v0.5) — Weeks 19-22

**Goals**: Python, Node.js, C interfaces

**Deliverables**:
- [ ] C FFI layer with cypherlite.h
- [ ] cbindgen configuration for auto-header generation
- [ ] Python binding via PyO3
- [ ] Node.js binding via neon
- [ ] Basic error handling across FFI

**Key Milestones**:
- Create nodes/relationships from Python
- Execute queries from Node.js
- Call via C API
- Error messages propagate cleanly

**Testing**: Integration tests with each language

**Estimated Effort**: 4 weeks (can parallelize by language)

---

### Phase 6: Production Hardening (v0.6-v1.0) — Weeks 23+

**Goals**: Stability, performance, documentation

**Deliverables**:
- [ ] Comprehensive test suite (unit + integration)
- [ ] Benchmark suite (vs KùzuDB, DuckDB)
- [ ] Error handling & recovery paths
- [ ] Configuration options (page size, cache size, etc.)
- [ ] Documentation (API docs, examples, performance tuning)
- [ ] Release artifacts (pre-built binaries, npm/PyPI packages)

**Key Milestones**:
- 90%+ test coverage
- Performance targets met (see Section 10)
- Zero known critical bugs
- Documentation complete

**Testing**: Fuzzing, stress tests, long-running scenarios

**Estimated Effort**: Open-ended (ongoing)

---

### Future Phases: Plugin Ecosystem (Post-v1.0)

**Vector Index Plugin** (Weeks TBD)
- HNSW implementation
- Semantic search in graphs
- Hybrid vector+graph queries

**Semantic Layer Plugin** (Weeks TBD)
- Type definitions
- Constraint validation
- Schema introspection

**Kinetic Layer Plugin** (Weeks TBD)
- Action definitions
- Authorization
- Workflow management

**Full-Text Index Plugin** (Weeks TBD)
- Inverted index
- Phrase search
- Text ranking

**GraphRAG Plugin** (Weeks TBD)
- Community detection
- Hierarchical summarization
- LLM integration

---

### Timeline Summary

```
Phase 1: Storage (v0.1)    ████░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░  Week 4
Phase 2: Query (v0.2)      ████████████░░░░░░░░░░░░░░░░░░░░░░░░  Week 10
Phase 3: Indexing (v0.3)   ████░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░  Week 14
Phase 4: Temporal (v0.4)   ████░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░  Week 18
Phase 5: Bindings (v0.5)   ████░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░  Week 22
Phase 6: Hardening (v1.0)  ██████░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░  Week 28+

Estimated MVP (Phase 1-3): 14 weeks
Estimated v1.0 (All Phases): 28+ weeks
```

**Recommended Staffing**:
- **Fast Track (v1.0 in 16 weeks)**: 3 engineers (Storage + Query + Bindings in parallel)
- **Balanced (v1.0 in 28 weeks)**: 1 engineer (sequential phases)

---

## 8. Rust Project Structure

### 8.1 Workspace Organization

```
cypherlite/
├── Cargo.toml (workspace manifest)
├── Cargo.lock (dependency lock)
├── README.md
├── LICENSE
│
├── cypherlite-core/              # Main library (phase 1-4)
│   ├── Cargo.toml
│   ├── src/
│   │   ├── lib.rs                # Library root
│   │   ├── storage/              # PHASE 1
│   │   │   ├── mod.rs
│   │   │   ├── buffer_pool.rs    # Page cache management
│   │   │   ├── btree.rs          # B-tree index implementation
│   │   │   ├── node_store.rs     # Node page storage
│   │   │   ├── edge_store.rs     # Relationship storage
│   │   │   ├── property_store.rs # Property pages & overflow
│   │   │   ├── wal.rs            # Write-ahead log
│   │   │   ├── checkpoint.rs     # WAL checkpoint logic
│   │   │   ├── file.rs           # File operations (.cyl format)
│   │   │   ├── page.rs           # Page structure definitions
│   │   │   └── recovery.rs       # Crash recovery
│   │   │
│   │   ├── transaction/          # PHASE 1
│   │   │   ├── mod.rs
│   │   │   ├── manager.rs        # Transaction lifecycle
│   │   │   ├── lock.rs           # Single-writer concurrency
│   │   │   ├── isolation.rs      # Isolation levels
│   │   │   └── mvcc.rs           # Multi-version concurrency
│   │   │
│   │   ├── graph/                # PHASE 1 & 2
│   │   │   ├── mod.rs
│   │   │   ├── node.rs           # Node data structure
│   │   │   ├── edge.rs           # Relationship data structure
│   │   │   ├── property.rs       # Property values (Value enum)
│   │   │   ├── label.rs          # Node labels/types
│   │   │   └── adjacency.rs      # Index-free adjacency chains
│   │   │
│   │   ├── query/                # PHASE 2
│   │   │   ├── mod.rs
│   │   │   ├── lexer.rs          # Tokenizer
│   │   │   ├── parser.rs         # Recursive descent parser
│   │   │   ├── ast.rs            # AST node definitions
│   │   │   ├── semantic.rs       # Semantic analysis & scope
│   │   │   ├── logical_plan.rs   # Logical operators
│   │   │   ├── optimizer.rs      # Cost-based optimization
│   │   │   ├── physical_plan.rs  # Physical operators
│   │   │   ├── executor.rs       # Query execution engine
│   │   │   └── functions.rs      # Built-in functions
│   │   │
│   │   ├── index/                # PHASE 3
│   │   │   ├── mod.rs
│   │   │   ├── label_index.rs    # Label scan B-tree
│   │   │   ├── type_index.rs     # Relationship type index
│   │   │   ├── btree_index.rs    # Generic B-tree for properties
│   │   │   └── hash_index.rs     # Hash index for equality
│   │   │
│   │   ├── plugin/               # PHASE 3+
│   │   │   ├── mod.rs
│   │   │   ├── registry.rs       # Plugin discovery & loading
│   │   │   ├── traits.rs         # Plugin trait definitions
│   │   │   ├── storage.rs        # StoragePlugin trait
│   │   │   ├── index.rs          # IndexPlugin trait
│   │   │   ├── query_ext.rs      # QueryPlugin trait
│   │   │   ├── serializer.rs     # SerializerPlugin trait
│   │   │   ├── event.rs          # EventPlugin trait
│   │   │   ├── loader.rs         # Dynamic plugin loading
│   │   │   └── error.rs          # Plugin errors
│   │   │
│   │   ├── temporal/             # PHASE 4
│   │   │   ├── mod.rs
│   │   │   ├── versioning.rs     # Version storage
│   │   │   ├── time_index.rs     # Temporal B-tree
│   │   │   ├── snapshot.rs       # Graph snapshots at time T
│   │   │   └── syntax.rs         # AT TIME parsing
│   │   │
│   │   ├── api/                  # PHASE 1-2
│   │   │   ├── mod.rs
│   │   │   ├── cypher.rs         # Cypher query API
│   │   │   ├── native.rs         # Native node/edge API
│   │   │   ├── transaction.rs    # Transaction API
│   │   │   └── error.rs          # Error types
│   │   │
│   │   ├── config/               # Configuration
│   │   │   ├── mod.rs
│   │   │   └── settings.rs       # Tunable parameters
│   │   │
│   │   ├── error.rs              # Error handling
│   │   ├── value.rs              # Value types (Int, String, List, etc.)
│   │   └── lib.rs                # Library exports
│   │
│   └── tests/                    # Integration tests
│       ├── storage_tests.rs
│       ├── query_tests.rs
│       ├── transaction_tests.rs
│       └── e2e_tests.rs
│
├── cypherlite-ffi/               # PHASE 5
│   ├── Cargo.toml
│   ├── src/
│   │   ├── lib.rs                # FFI root
│   │   ├── c.rs                  # C FFI bindings
│   │   ├── error.rs              # Error mapping to C
│   │   └── types.rs              # C type marshalling
│   ├── include/
│   │   └── cypherlite.h           # Generated C header (cbindgen)
│   └── tests/
│       └── c_tests.c             # C integration tests
│
├── cypherlite-python/            # PHASE 5
│   ├── Cargo.toml
│   ├── src/
│   │   ├── lib.rs
│   │   ├── db.rs                 # CypherLite class wrapper
│   │   ├── query.rs              # Query result iterator
│   │   └── error.rs              # Python exception mapping
│   ├── Makefile
│   └── tests/
│       └── test_*.py             # Python integration tests
│
├── cypherlite-nodejs/            # PHASE 5
│   ├── Cargo.toml
│   ├── src/
│   │   ├── lib.rs
│   │   ├── db.rs
│   │   └── query.rs
│   ├── package.json
│   ├── index.js
│   └── tests/
│       └── test_*.js             # Node.js tests
│
├── benches/                      # Benchmarks
│   ├── storage_bench.rs
│   ├── query_bench.rs
│   └── comparison_bench.rs       # vs KùzuDB, DuckDB
│
└── docs/
    ├── 00_master_overview.md     # This file
    ├── 01_core_architecture.md
    ├── 02_storage_engine.md
    ├── 03_query_engine.md
    ├── 04_plugin_architecture.md
    ├── research/
    │   ├── 01_existing_technologies.md
    │   ├── 02_cypher_rdf_temporal.md
    │   └── 03_graphrag_agent_usecases.md
    ├── API.md                    # API documentation
    ├── PERFORMANCE.md            # Tuning guide
    └── EXAMPLES.md               # Code examples
```

### 8.2 Cargo Workspace Configuration

**Root `Cargo.toml`**:
```toml
[workspace]
members = [
    "cypherlite-core",
    "cypherlite-ffi",
    "cypherlite-python",
    "cypherlite-nodejs",
]
resolver = "2"

[workspace.package]
version = "0.1.0"
edition = "2021"
authors = ["CypherLite Team"]
license = "Apache-2.0 OR MIT"
repository = "https://github.com/cypherlite/cypherlite"
homepage = "https://cypherlite.io"

[workspace.dependencies]
parking_lot = "0.12"
crossbeam = "0.8"
dashmap = "5.5"
logos = "0.14"
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
chrono = "0.4"
bincode = "1.3"
thiserror = "1.0"
tracing = "0.1"
tracing-subscriber = "0.3"
```

**`cypherlite-core/Cargo.toml`**:
```toml
[package]
name = "cypherlite-core"
version.workspace = true
edition.workspace = true
authors.workspace = true
license.workspace = true

[dependencies]
parking_lot.workspace = true
crossbeam.workspace = true
dashmap.workspace = true
logos.workspace = true
serde.workspace = true
serde_json.workspace = true
chrono.workspace = true
bincode.workspace = true
thiserror.workspace = true
tracing.workspace = true

[dev-dependencies]
criterion = "0.5"
proptest = "1.3"
tempfile = "3.8"

[[bench]]
name = "storage"
harness = false

[[bench]]
name = "query"
harness = false
```

### 8.3 Module Dependencies

```
lib.rs (public API)
  │
  ├── api/ (public interfaces)
  │   ├── cypher.rs (depends on query/)
  │   ├── native.rs (depends on graph/, storage/)
  │   └── transaction.rs (depends on transaction/)
  │
  ├── query/ (query execution)
  │   ├── lexer.rs (standalone)
  │   ├── parser.rs (depends on lexer)
  │   ├── ast.rs (standalone)
  │   ├── semantic.rs (depends on ast, graph)
  │   ├── logical_plan.rs (depends on ast, index)
  │   ├── optimizer.rs (depends on logical_plan)
  │   ├── physical_plan.rs (depends on logical_plan)
  │   ├── executor.rs (depends on physical_plan, storage, graph)
  │   └── functions.rs (standalone, used by executor)
  │
  ├── storage/ (core storage layer)
  │   ├── page.rs (standalone)
  │   ├── file.rs (depends on page)
  │   ├── buffer_pool.rs (depends on page, file)
  │   ├── btree.rs (depends on page, buffer_pool)
  │   ├── node_store.rs (depends on btree, graph)
  │   ├── edge_store.rs (depends on btree, graph)
  │   ├── property_store.rs (depends on btree, value)
  │   ├── wal.rs (depends on page, file)
  │   ├── checkpoint.rs (depends on wal, buffer_pool)
  │   └── recovery.rs (depends on wal, file, buffer_pool)
  │
  ├── transaction/ (concurrency control)
  │   ├── lock.rs (standalone)
  │   ├── manager.rs (depends on lock, wal, storage)
  │   ├── isolation.rs (depends on manager)
  │   └── mvcc.rs (depends on lock, isolation)
  │
  ├── graph/ (data structures)
  │   ├── property.rs (standalone)
  │   ├── label.rs (standalone)
  │   ├── node.rs (depends on property, label)
  │   ├── edge.rs (depends on property, label)
  │   └── adjacency.rs (depends on node, edge)
  │
  ├── index/ (indexing)
  │   ├── label_index.rs (depends on btree, graph)
  │   ├── type_index.rs (depends on btree, graph)
  │   └── btree_index.rs (depends on btree)
  │
  ├── plugin/ (extensibility)
  │   ├── traits.rs (standalone)
  │   ├── registry.rs (depends on traits)
  │   └── loader.rs (depends on registry)
  │
  ├── temporal/ (future)
  │   ├── versioning.rs (depends on storage, graph)
  │   └── time_index.rs (depends on btree)
  │
  └── Error/Config/Value (cross-cutting)
```

---

## 9. Competitive Positioning

### 9.1 Comparison Matrix

| Feature | CypherLite | Neo4j | KùzuDB | DuckDB+PGQ | SQLite |
|---------|-----------|-------|--------|-----------|--------|
| **Embedding Model** | ✓ In-process | ✗ Server | ✓ Embedded | ✓ Embedded | ✓ In-process |
| **Single File** | ✓ Yes (.cyl) | ✗ Directory | ✓ Yes (.kuzu) | ✓ Yes | ✓ Yes (.db) |
| **Cypher Support** | ✓ v1.0 subset | ✓ Full | ✓ Full | ✗ SQL/PGQ | ✗ SQL only |
| **Native Graph** | ✓ Yes | ✓ Yes | ✓ Yes | ◐ Extended | ✗ No |
| **Index-Free Adjacency** | ✓ Yes | ✓ Yes | ? Unknown | ✗ No | ✗ No |
| **ACID Transactions** | ✓ Yes (WAL) | ✓ Yes | ✓ Yes | ✓ Yes | ✓ Yes |
| **Production Ready** | ◐ v1.0 target | ✓ Yes | ✗ Archived | ✓ Yes | ✓ Yes |
| **Lightweight** | ✓ <50MB | ✗ 500MB+ | ✓ Small | ✓ Small | ✓ <1MB |
| **Temporal Queries** | ◐ Planned | ◐ Enterprise | ✗ No | ✗ No | ✗ No |
| **Plugin System** | ✓ Yes (v0.3) | ◐ Procedures | ? Unknown | ◐ Extensions | ✗ No |
| **FFI Available** | ✓ C/Python/JS | ✗ Bolt protocol | ◐ Limited | ◐ Limited | ✓ Wide |
| **License** | TBD Apache | Commercial | Archived | MIT | Public domain |
| **Ideal Use Case** | Embedded agent memory, edge computing, local-first | Enterprise graph analytics | ~~Embedded analytics~~ | SQL analytics with graph | Relational data |

### 9.2 CypherLite's Unique Value Proposition

**For Developers**:
1. **Deploy as Single File**: Copy `agents.cyl` to production. No database server setup.
2. **Native Graph Queries**: Cypher is more intuitive for graph problems than SQL joins.
3. **Embedded in Process**: No network latency, perfect for agents and edge devices.
4. **Tiny Footprint**: ~50MB binary, suitable for mobile and IoT.

**For AI/Agent Applications**:
1. **Structured Memory**: Store agent reasoning, decisions, and context as queryable graphs.
2. **Temporal Tracking**: Follow decision evolution: `MATCH (d:Decision) AT TIME $past_time`.
3. **Relationship Discovery**: Find related entities: `(a)-[:INFLUENCED_BY]->(b)`.
4. **Local-First Privacy**: Agent memory stays on device, no cloud sync required.

**For Edge Computing**:
1. **Autonomous Decisions**: Query relationships without cloud connectivity.
2. **Lightweight**: Resource-constrained IoT gateways can query relationship context.
3. **Offline-First**: Sync to cloud only when connectivity available.

**For Knowledge Graphs**:
1. **GraphRAG Foundation**: Embed knowledge extraction and semantic search.
2. **Semantic Layer**: Define valid entities and relationships via plugins.
3. **Vector Search**: Optional plugin for semantic similarity.

**vs. Neo4j**: CypherLite is lightweight and embedded; Neo4j is powerful but requires server.
**vs. KùzuDB**: CypherLite has plugin extensibility; KùzuDB (archived) didn't.
**vs. DuckDB**: CypherLite is graph-native; DuckDB is SQL with graph extension.
**vs. SQLite**: CypherLite adds native graph support; SQLite requires complex joins.

---

## 10. Risk Assessment

### 10.1 Technical Risks

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|-----------|
| **Cypher parser complexity** | Medium | Medium | Start with subset (v1.0), incremental expansion. Comprehensive test suite. |
| **WAL implementation bugs** | Medium | High | Study SQLite implementation, extensive recovery testing, fuzzing. |
| **Query optimizer too slow** | Low | Low | Fall back to simple sequential plan, optimize later. |
| **Concurrency bugs** | Medium | High | Use parking_lot (proven), write-heavy test suite, thread sanitizer. |
| **Performance regression** | Medium | Medium | Establish benchmarks early, CI benchmarking, regression detection. |
| **Plugin compatibility breaks** | Low | Medium | Semantic versioning, thorough plugin interface design, test harness. |

### 10.2 Scope Risks

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|-----------|
| **Timeline slippage** | Medium | Medium | Prioritize core (storage → query), defer plugins/temporal. Weekly burndown. |
| **Feature creep** | High | High | Strict v1.0 definition (see Section 7), defer advanced features. |
| **FFI binding complexity** | Low | Low | PyO3/cbindgen handle most complexity, test each binding thoroughly. |
| **Documentation lag** | Medium | Low | Write docs during implementation, maintain living examples. |

### 10.3 Performance Targets

To justify "lightweight" positioning, CypherLite must meet these targets:

**Query Latency** (p99):
- Simple match: < 10ms
- Pattern with 2 hops: < 50ms
- Full-text scan + filter: < 100ms

**Throughput** (1000-node graph):
- Sequential writes: > 1000 nodes/sec
- Concurrent reads (4 threads): > 50K reads/sec

**Resource Usage**:
- Binary size: < 50MB
- Memory per 1M nodes: < 500MB
- Page cache overhead: < 100MB for default

**File Size**:
- 1M nodes, 2M relationships: < 500MB

---

## Conclusion

CypherLite is positioned to become the **SQLite of graph databases**: simple, single-file, embeddable, and production-ready. By combining proven architecture patterns (SQLite's simplicity, Neo4j's graph semantics), modern optimization (query planning, temporal support), and extensibility (plugins), CypherLite will enable a new class of applications that require embedded graph intelligence without server complexity.

The 28-week roadmap to v1.0 balances delivery speed with architectural quality. The plugin system ensures extensibility without core bloat. The research foundation (existing technologies, Cypher standards, agent use cases) validates product-market fit.

**Success metrics**:
- v1.0 released within 28 weeks
- < 10KB/sec documentation
- First production users in agent/edge/knowledge-graph domains
- Active plugin ecosystem by month 12

---

**Document Status**: Architecture & Roadmap Complete
**Next Steps**: Kickoff Phase 1 (Storage Engine)
**Approval Required**: Engineering lead, product manager, architect
