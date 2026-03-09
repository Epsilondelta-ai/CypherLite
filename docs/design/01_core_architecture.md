# CypherLite Core Architecture Design

**Document Status**: Architecture Design Phase
**Target Language**: Rust
**Date**: March 2026
**Document Purpose**: Define the complete core architecture for CypherLite, a SQLite-like single-file embedded graph database engine with Cypher query support.

---

## Table of Contents

1. [System Overview](#1-system-overview)
2. [Module Structure](#2-module-structure)
3. [Core Components](#3-core-components)
4. [Concurrency Model](#4-concurrency-model)
5. [Error Handling & Recovery](#5-error-handling--recovery)
6. [Configuration & Initialization](#6-configuration--initialization)
7. [Language Choice Justification](#7-language-choice-justification)
8. [Implementation Roadmap](#8-implementation-roadmap)

---

## 1. System Overview

### 1.1 Architecture Diagram

```
┌─────────────────────────────────────────────────────────────────────┐
│                        APPLICATION LAYER                            │
│  (Python/Node.js/C bindings via FFI)                               │
└────────────────────────────────┬────────────────────────────────────┘
                                 │
┌────────────────────────────────▼────────────────────────────────────┐
│                          API LAYER                                   │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────────────────┐  │
│  │ Cypher API   │  │ Native API    │  │ Connection Pool Manager  │  │
│  │ (MATCH/      │  │ (Nodes/Rels/  │  │ (Multi-reader, Single    │  │
│  │  CREATE/     │  │  Properties)  │  │  writer semantics)       │  │
│  │  MERGE/etc)  │  │               │  │                          │  │
│  └──────────────┘  └──────────────┘  └──────────────────────────┘  │
└────────────────────────────────┬────────────────────────────────────┘
                                 │
┌────────────────────────────────▼────────────────────────────────────┐
│                      QUERY ENGINE LAYER                              │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────────────────┐  │
│  │ Cypher       │  │ Query        │  │ Transaction Manager      │  │
│  │ Parser       │  │ Optimizer    │  │ (MVCC/WAL-based)        │  │
│  │ (Lexer/AST)  │  │ (Plan cost   │  │ (Isolation levels)      │  │
│  │              │  │  estimation) │  │                          │  │
│  └──────────────┘  └──────────────┘  └──────────────────────────┘  │
│                                                                      │
│  ┌──────────────────────────────┐  ┌──────────────────────────┐    │
│  │ Query Execution Engine        │  │ Catalog (Schema)         │    │
│  │ (Pattern matching, traversal, │  │ (Labels, relationships,  │    │
│  │  aggregation, sorting)        │  │  properties metadata)    │    │
│  └──────────────────────────────┘  └──────────────────────────┘    │
└────────────────────────────────┬────────────────────────────────────┘
                                 │
┌────────────────────────────────▼────────────────────────────────────┐
│                    STORAGE ENGINE LAYER                              │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────────────────┐  │
│  │ Node Store   │  │ Relationship  │  │ Property Store           │  │
│  │ (B-tree of   │  │ Store         │  │ (Inline + overflow       │  │
│  │  node pages) │  │ (B-tree of    │  │  pages)                  │  │
│  │              │  │  rel pages)   │  │                          │  │
│  └──────────────┘  └──────────────┘  └──────────────────────────┘  │
│                                                                      │
│  ┌──────────────────────────────┐  ┌──────────────────────────┐    │
│  │ Index Store (B-tree)         │  │ Write-Ahead Log (WAL)    │    │
│  │ (Label indices, relationship │  │ (Transaction frames)     │    │
│  │  type indices)               │  │                          │    │
│  └──────────────────────────────┘  └──────────────────────────┘    │
└────────────────────────────────┬────────────────────────────────────┘
                                 │
┌────────────────────────────────▼────────────────────────────────────┐
│                    BUFFER POOL & CACHE LAYER                         │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────────────────┐  │
│  │ Page Cache   │  │ Hot Node     │  │ Relationship Adjacency   │  │
│  │ (LRU         │  │ Cache (LRU)  │  │ Cache                    │  │
│  │  eviction)   │  │              │  │ (Node → [Rel IDs])       │  │
│  └──────────────┘  └──────────────┘  └──────────────────────────┘  │
└────────────────────────────────┬────────────────────────────────────┘
                                 │
┌────────────────────────────────▼────────────────────────────────────┐
│                      FILE I/O LAYER                                  │
│  ┌──────────────────────────────┐  ┌──────────────────────────┐    │
│  │ Single Database File Format   │  │ Memory-Mapped I/O (mmap)│    │
│  │ (Pages, checksums, magic     │  │ (Optional for large      │    │
│  │  numbers, metadata)          │  │  read-heavy workloads)   │    │
│  └──────────────────────────────┘  └──────────────────────────┘    │
└─────────────────────────────────────────────────────────────────────┘
```

### 1.2 Layer Decomposition

#### API Layer
- **Responsibility**: Surface-level interface for applications
- **Components**:
  - Cypher query API (parse and execute Cypher strings)
  - Native API (direct node/relationship/property manipulation)
  - Connection pooling and session management
  - Parameter binding and result streaming

#### Query Engine Layer
- **Responsibility**: Transform user queries into executable plans
- **Components**:
  - Cypher parser (lexer + recursive descent parser for AST)
  - Query optimizer (cost-based plan selection)
  - Transaction manager (isolation, MVCC)
  - Execution engine (runtime for pattern matching and traversal)
  - Catalog (schema metadata and validation)

#### Storage Engine Layer
- **Responsibility**: Persistent graph data management
- **Components**:
  - Node store (B-tree of node records)
  - Relationship store (B-tree of relationship records)
  - Property store (inline small properties + overflow pages)
  - Index store (label and type indices)
  - WAL (write-ahead log for durability)

#### Buffer Pool & Cache Layer
- **Responsibility**: Minimize disk I/O through intelligent caching
- **Components**:
  - Page cache (LRU-evicted buffer pool)
  - Hot node cache (frequently accessed nodes)
  - Relationship adjacency cache
  - Cache coherency and invalidation strategies

#### File I/O Layer
- **Responsibility**: Low-level disk access and memory mapping
- **Components**:
  - File format handling (page headers, checksums)
  - Memory-mapped I/O for read-heavy workloads
  - Page locking and synchronization

### 1.3 Design Principles

**Zero-Config**: Database should work with minimal configuration
- Sensible defaults (4KB pages, 10MB cache, rollback mode)
- Auto-initialization on first connection
- No complex setup rituals

**Single-File**: Entire database in one portable file
- Copy file = backup database
- Copy file = transport database
- Atomic snapshots via single file

**ACID Transactions**: Full ACID guarantees via WAL
- Atomicity: All-or-nothing commits via WAL frames
- Consistency: Schema validation, referential integrity
- Isolation: Snapshot isolation for readers; serializable for writers
- Durability: WAL ensures crash recovery

**Embedded-First**: Designed to run in-process
- No server, no network overhead
- FFI bindings for C, Python, Node.js
- Memory-conscious for resource-constrained environments
- Single connection pool shared with host process

**Graph-Native**: Storage and queries optimized for graph patterns
- Index-free adjacency (pointer-based traversal)
- Cypher as primary query language
- Efficient pattern matching and path finding
- Property graphs (nodes + relationships + properties)

---

## 2. Module Structure

### 2.1 Rust Crate Organization

```
cypherlite/
├── Cargo.toml                          # Workspace root
│
├── cypherlite-core/                    # Main library crate
│   ├── Cargo.toml
│   ├── src/
│   │   ├── lib.rs                      # Crate root
│   │   │
│   │   ├── api/                        # API Layer
│   │   │   ├── mod.rs
│   │   │   ├── connection.rs           # Connection and session management
│   │   │   ├── cypher_api.rs           # Cypher query execution
│   │   │   ├── native_api.rs           # Direct node/rel/property access
│   │   │   └── result.rs               # Result types and streaming
│   │   │
│   │   ├── query/                      # Query Engine Layer
│   │   │   ├── mod.rs
│   │   │   ├── parser/
│   │   │   │   ├── mod.rs
│   │   │   │   ├── lexer.rs            # Tokenization
│   │   │   │   └── ast.rs              # Abstract Syntax Tree
│   │   │   ├── planner.rs              # Query plan generation
│   │   │   ├── optimizer.rs            # Cost-based optimization
│   │   │   ├── executor.rs             # Plan execution engine
│   │   │   ├── catalog.rs              # Schema metadata
│   │   │   └── transaction.rs          # Transaction management
│   │   │
│   │   ├── storage/                    # Storage Engine Layer
│   │   │   ├── mod.rs
│   │   │   ├── format.rs               # File format definitions
│   │   │   ├── node_store.rs           # Node record store
│   │   │   ├── relationship_store.rs   # Relationship store
│   │   │   ├── property_store.rs       # Property storage
│   │   │   ├── index_store.rs          # Index management
│   │   │   ├── wal.rs                  # Write-ahead log
│   │   │   ├── checkpoint.rs           # WAL checkpointing
│   │   │   └── recovery.rs             # Crash recovery
│   │   │
│   │   ├── cache/                      # Buffer Pool & Cache Layer
│   │   │   ├── mod.rs
│   │   │   ├── page_cache.rs           # LRU page buffer
│   │   │   ├── node_cache.rs           # Hot node LRU cache
│   │   │   ├── adjacency_cache.rs      # Relationship adjacency
│   │   │   └── policy.rs               # Eviction and replacement policies
│   │   │
│   │   ├── io/                         # File I/O Layer
│   │   │   ├── mod.rs
│   │   │   ├── file.rs                 # File handle management
│   │   │   ├── mmap.rs                 # Memory-mapped I/O wrapper
│   │   │   ├── page_io.rs              # Page reading/writing
│   │   │   └── checksum.rs             # Integrity verification
│   │   │
│   │   ├── error.rs                    # Error types
│   │   ├── types.rs                    # Core type definitions
│   │   ├── config.rs                   # Configuration
│   │   └── lib.rs                      # Public API exports
│   │
│   └── tests/                          # Integration tests
│       ├── basic_operations_test.rs
│       ├── transactions_test.rs
│       ├── concurrency_test.rs
│       └── recovery_test.rs
│
├── cypherlite-ffi/                     # FFI bindings (C API)
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── c_api.rs                    # C function definitions
│       └── conversions.rs              # Rust ↔ C type conversions
│
├── cypherlite-python/                  # Python bindings
│   ├── Cargo.toml
│   ├── Makefile
│   ├── pyproject.toml
│   └── src/
│       └── lib.rs                      # PyO3 bindings
│
├── cypherlite-node/                    # Node.js bindings
│   ├── Cargo.toml
│   ├── package.json
│   ├── index.js
│   └── src/
│       └── lib.rs                      # NAPI-RS bindings
│
├── cypherlite-cli/                     # Command-line tool
│   ├── Cargo.toml
│   └── src/
│       └── main.rs
│
└── README.md
```

### 2.2 Dependency Graph

```
cypherlite-core
├── (internal: query → storage → cache → io → types → error → config)
│
├── [External Deps]
│   ├── rusqlite             (SQLite, for reference/learning)
│   ├── byteorder            (Endianness handling)
│   ├── crc32fast            (Checksum computation)
│   ├── parking_lot          (Fast RwLock and Mutex)
│   ├── lru                  (LRU cache eviction)
│   ├── indexmap             (Insertion-order hashmap)
│   ├── nom                  (Parser combinator for Cypher lexer)
│   ├── thiserror            (Error handling)
│   └── tracing              (Structured logging)

cypherlite-ffi
├── cypherlite-core
└── libc                     (C standard library types)

cypherlite-python
├── cypherlite-core
└── pyo3                     (Python FFI bindings)

cypherlite-node
├── cypherlite-core
└── napi-rs                  (Node.js FFI bindings)

cypherlite-cli
├── cypherlite-core
├── clap                     (CLI argument parsing)
└── prettytable-rs           (Terminal table formatting)
```

### 2.3 Public API Surface

```rust
// Connection management
pub struct Database { ... }
pub struct Connection { ... }
pub struct Transaction { ... }
pub struct Session { ... }

impl Database {
    pub fn open(path: &str) -> Result<Self>
    pub fn open_in_memory() -> Result<Self>
    pub fn create_connection(&self) -> Result<Connection>
    pub fn close(&mut self) -> Result<()>
    pub fn checkpoint(&self) -> Result<()>
}

impl Connection {
    pub fn execute_cypher(&self, query: &str, params: &Value)
        -> Result<CypherResult>
    pub fn transaction(&self) -> Result<Transaction>
    pub fn create_node(&self, labels: &[&str]) -> Result<Node>
    pub fn get_node(&self, id: NodeId) -> Result<Option<Node>>
    pub fn create_index(&self, label: &str, property: &str)
        -> Result<Index>
}

impl Transaction {
    pub fn execute(&self, query: &str) -> Result<CypherResult>
    pub fn commit(&self) -> Result<()>
    pub fn rollback(&self) -> Result<()>
}

// Result types
pub struct CypherResult { ... }
pub struct ResultSet { ... }

impl ResultSet {
    pub fn next(&mut self) -> Result<Option<Row>>
    pub fn collect(&mut self) -> Result<Vec<Row>>
}

// Graph types
pub struct Node { ... }
pub struct Relationship { ... }
pub struct Property { ... }

pub enum Value {
    Null,
    Boolean(bool),
    Integer(i64),
    Float(f64),
    String(String),
    Bytes(Vec<u8>),
    List(Vec<Value>),
    Map(HashMap<String, Value>),
}

pub type NodeId = u64;
pub type RelationshipId = u64;
```

### 2.4 FFI Layer Design

#### C API (cypherlite-ffi)

```c
// Opaque handles
typedef void CypherLiteDB;
typedef void CypherLiteConn;
typedef void CypherLiteTxn;
typedef void CypherLiteResult;

// Database operations
CypherLiteDB* cypherlite_open(const char* path, CypherLiteErr* err);
CypherLiteDB* cypherlite_open_memory(CypherLiteErr* err);
void cypherlite_close(CypherLiteDB* db);
void cypherlite_free(void* ptr);

// Connection operations
CypherLiteConn* cypherlite_connect(CypherLiteDB* db, CypherLiteErr* err);
void cypherlite_disconnect(CypherLiteConn* conn);

// Query execution
CypherLiteResult* cypherlite_execute(
    CypherLiteConn* conn,
    const char* query,
    CypherLiteErr* err
);

// Result iteration
int cypherlite_result_next(CypherLiteResult* result, CypherLiteErr* err);
const char* cypherlite_result_get_string(
    CypherLiteResult* result,
    const char* column,
    CypherLiteErr* err
);
void cypherlite_result_free(CypherLiteResult* result);

// Error handling
const char* cypherlite_error_message(CypherLiteErr* err);
void cypherlite_error_free(CypherLiteErr* err);
```

---

## 3. Core Components

### 3.1 Connection Manager

**Design Pattern**: SQLite-style connection pooling

```rust
pub struct Database {
    inner: Arc<RwLock<DatabaseInner>>,
}

struct DatabaseInner {
    file: File,
    config: DatabaseConfig,
    catalog: Catalog,
    wal: WriteAheadLog,

    // Global state
    connections: HashMap<u64, ConnectionHandle>,
    next_connection_id: u64,

    // Synchronization
    write_lock: Mutex<()>,           // Single writer
    readers_count: AtomicUsize,      // Concurrent readers
    shutdown: AtomicBool,
}

pub struct Connection {
    db: Arc<RwLock<DatabaseInner>>,
    id: u64,
    cache_layer: CacheManager,
    isolation_level: IsolationLevel,
}
```

**Key Features**:
- Multiple concurrent readers
- Single exclusive writer (serialization)
- WAL enables readers to see committed data while writer works
- Connection pooling with reference counting
- Per-connection cache for hot data

**Connection Lifecycle**:
```
Open → Acquire Read Lock → Execute Queries → Release Lock
    ↓
    (If Write) → Acquire Write Lock → Execute & Commit → Release Lock
    ↓
Close
```

### 3.2 Transaction Manager

**Implementation**: Write-Ahead Log (WAL) with MVCC

```rust
pub enum IsolationLevel {
    ReadUncommitted,    // Dirty reads OK
    ReadCommitted,      // No dirty reads
    RepeatableRead,     // Snapshot isolation
    Serializable,       // Strongest: full serialization
}

pub struct Transaction {
    id: TransactionId,
    start_lsn: LogSequenceNumber,
    isolation_level: IsolationLevel,

    read_set: HashMap<PageId, PageVersion>,
    write_set: Vec<WritePage>,
    status: TransactionStatus,
}

enum TransactionStatus {
    Active,
    Committing,
    Committed,
    RolledBack,
}

pub struct WriteAheadLog {
    log_file: File,
    in_memory_log: Vec<LogFrame>,
    latest_checkpoint_lsn: LogSequenceNumber,
}

struct LogFrame {
    transaction_id: TransactionId,
    page_id: PageId,
    page_content: Vec<u8>,
    frame_header: FrameHeader,
    checksum: u32,
}

struct FrameHeader {
    magic: u32,              // 0x3d9d04d9
    sequence: u32,
    page_count: u32,
    commit_timestamp: u64,
}
```

**Transaction Lifecycle**:
```
1. BEGIN
   - Acquire read lock on current database state
   - Record start LSN (Log Sequence Number)

2. EXECUTE (Read)
   - Read from snapshot at start LSN
   - All reads see consistent state from START time

3. EXECUTE (Write)
   - Collect changes in write_set
   - Do NOT modify main database yet

4. COMMIT
   - Obtain write lock (exclusive)
   - Validate no conflicts in serializable mode
   - Write WAL frames with all changes
   - Increment LSN
   - Release write lock

5. ROLLBACK
   - Discard write_set
   - Release locks
   - No WAL entries written
```

**Isolation Levels**:

| Level | Dirty Reads | Non-Repeatable Reads | Phantoms |
|-------|-------------|----------------------|----------|
| Read Uncommitted | Yes | Yes | Yes |
| Read Committed | No | Yes | Yes |
| Repeatable Read (Snapshot) | No | No | Yes |
| Serializable | No | No | No |

CypherLite defaults to **Snapshot Isolation** (RepeatableRead):
- Readers see consistent view from transaction start
- Writers don't block readers (via WAL)
- Allows true concurrent reads with single writer

### 3.3 Buffer Pool / Page Cache

**Design**: LRU-evicted buffer pool with configurable size

```rust
pub struct PageCache {
    pages: Arc<RwLock<OrderedMap<PageId, CachedPage>>>,
    eviction_policy: EvictionPolicy,
    config: CacheConfig,
    stats: CacheStats,
}

struct CachedPage {
    id: PageId,
    data: Vec<u8>,
    dirty: bool,
    pin_count: usize,
    last_access: Instant,
    access_count: u64,
}

pub struct CacheConfig {
    pub max_pages: usize,           // Default: 2560 (10MB at 4K pages)
    pub page_size: usize,           // Default: 4096
    pub eviction_policy: String,    // "lru", "lfu", "clock"
}

impl PageCache {
    pub fn get_or_load(
        &self,
        page_id: PageId,
        loader: impl Fn() -> Result<Vec<u8>>,
    ) -> Result<Pin<CachedPage>>

    pub fn mark_dirty(&self, page_id: PageId) -> Result<()>

    pub fn flush_dirty(&self) -> Result<()>

    pub fn evict_if_needed(&self) -> Result<()>
}
```

**Multi-Level Caching Strategy**:

**L1: Page Cache** (4KB pages from disk)
- LRU eviction
- Reduces disk I/O
- Configurable size (10MB default)

**L2: Hot Node Cache** (high-cardinality nodes)
- Fixed-size cache (e.g., 10K nodes)
- Stores deserialized node objects
- Avoids repeated parsing of node records

**L3: Relationship Adjacency Cache**
- Maps Node ID → [Relationship IDs]
- Accelerates graph traversal
- Invalidated on relationship mutations

**Cache Coherency**:
- Write-through: Changes written to disk before cache invalidation
- Invalidation on UPDATE/DELETE operations
- TTL-based expiration for analytical workloads

### 3.4 Catalog (Schema Metadata)

**Purpose**: Track labels, relationship types, and properties

```rust
pub struct Catalog {
    labels: BTreeMap<String, LabelId>,
    relationship_types: BTreeMap<String, RelTypeId>,
    properties: BTreeMap<String, PropertyId>,
    indices: BTreeMap<(String, String), IndexId>, // (label, property)
}

pub struct LabelId(u16);
pub struct RelTypeId(u16);
pub struct PropertyId(u32);
pub struct IndexId(u32);

impl Catalog {
    pub fn get_or_create_label(&mut self, name: &str) -> LabelId

    pub fn get_or_create_rel_type(&mut self, name: &str) -> RelTypeId

    pub fn create_index(
        &mut self,
        label: &str,
        property: &str,
    ) -> Result<IndexId>

    pub fn serialize(&self) -> Result<Vec<u8>>

    pub fn deserialize(data: &[u8]) -> Result<Self>
}

pub struct Index {
    id: IndexId,
    label: LabelId,
    property: PropertyId,
    btree: BTreeIndex,
    stats: IndexStats,
}

pub struct IndexStats {
    cardinality: u64,
    selectivity: f32,
    last_analyzed: Instant,
}
```

**Storage in File**:
- Metadata page 0: Database header + catalog offsets
- Metadata pages 1-N: Catalog serialized data
- Enables efficient schema validation without full scan

### 3.5 Query Pipeline

**Overview**: Parse → Validate → Plan → Optimize → Execute

```
"MATCH (n:User)-[:FOLLOWS]->(m:User) RETURN n.name, m.name"
    ↓
[PARSE]
    → Abstract Syntax Tree (AST)
    → MatchClause { patterns: [...], return: [...] }
    ↓
[VALIDATE]
    → Check labels exist in catalog
    → Check properties exist
    → Type checking on predicates
    ↓
[PLAN]
    → Convert AST to logical plan
    → LogicalPlan::Match { start: Node(User), pattern: [...] }
    ↓
[OPTIMIZE]
    → Cost estimation
    → Index selection
    → Filter pushdown
    → Join reordering
    → PhysicalPlan { operations: [IndexScan, RelScan, Filter, Project] }
    ↓
[EXECUTE]
    → Interpret physical plan
    → Fetch data from storage
    → Execute filters
    → Stream results
    → ResultSet
```

**Parser (Cypher Subset)**:

```rust
pub struct CypherParser {
    lexer: Lexer,
}

pub enum CypherClause {
    Match(MatchClause),
    Create(CreateClause),
    Merge(MergeClause),
    Where(WhereClause),
    Return(ReturnClause),
    OrderBy(OrderByClause),
    Limit(LimitClause),
}

pub struct MatchClause {
    pub patterns: Vec<Pattern>,
    pub optional: bool,
}

pub enum Pattern {
    NodePattern(NodePattern),
    RelationshipPattern(RelationshipPattern),
    PathPattern(Vec<Pattern>),
}

pub struct NodePattern {
    pub var: Option<String>,
    pub labels: Vec<String>,
    pub properties: HashMap<String, Expression>,
}

pub struct RelationshipPattern {
    pub var: Option<String>,
    pub types: Vec<String>,
    pub properties: HashMap<String, Expression>,
    pub direction: Direction,
    pub min_hops: Option<u32>,
    pub max_hops: Option<u32>,
}
```

**Executor**:

```rust
pub struct QueryExecutor {
    conn: Connection,
    cache: CacheManager,
}

impl QueryExecutor {
    pub fn execute(&self, plan: PhysicalPlan) -> Result<ResultSet> {
        match plan {
            PhysicalPlan::IndexScan { label, property, value } => {
                // Use index to find matching nodes
                self.index_scan(label, property, value)
            }
            PhysicalPlan::RelationshipScan { rel_type, src, dst } => {
                // Traverse from src_node through rel_type to dst_node
                self.rel_scan(rel_type, src, dst)
            }
            PhysicalPlan::Filter { input, predicate } => {
                // Apply WHERE clause
                let rows = self.execute(*input)?;
                rows.filter(predicate)
            }
            PhysicalPlan::Projection { input, columns } => {
                // Apply RETURN projection
                let rows = self.execute(*input)?;
                rows.project(columns)
            }
        }
    }
}
```

---

## 4. Concurrency Model

### 4.1 Read/Write Locking Strategy

**Architecture**: SQLite-inspired "readers-writer" lock

```
DATABASE STATE:

┌─────────────────────────────────────────┐
│ Main Database File                      │
│ (Latest committed state)                │
└─────────────────────────────────────────┘

┌─────────────────────────────────────────┐
│ Write-Ahead Log (WAL)                   │
│ (Uncommitted transactions)              │
└─────────────────────────────────────────┘
```

**Lock States**:

```rust
pub enum LockMode {
    Shared,      // Multiple readers
    Exclusive,   // Single writer (exclusive)
    None,        // Unlocked
}

pub struct LockManager {
    write_lock: Mutex<()>,           // Exclusive for writers
    reader_count: Arc<AtomicUsize>,  // Count of active readers
}

impl LockManager {
    pub fn acquire_read(&self) -> ReadGuard {
        // Increment reader count
        // Readers do NOT block each other
        // Writers block until reader_count == 0
    }

    pub fn acquire_write(&self) -> WriteGuard {
        // Acquire exclusive write_lock
        // Wait for reader_count == 0
        // Only one writer at a time
    }
}
```

**Lock Timeline Example**:

```
Time  Writer1     Reader1     Reader2     Writer2
────────────────────────────────────────────────────
 0    →acquire_w
 1    granted
 2               →acquire_r
 3               blocked (waiting for Writer1)
 4    ←release_w
 5    (commit)    granted
 6                           →acquire_r
 7                           granted
 8                ←release_r
 9               →acquire_w
10               blocked (Reader2 active)
11                          ←release_r
12               granted
```

### 4.2 WAL Mode for Concurrent Reads During Writes

**WAL Architecture**:

```
DATABASE
├─ data.db          (main file, committed state)
├─ data.db-wal      (write-ahead log, frame buffer)
└─ data.db-shm      (shared memory index, in-memory WAL index)

READER:
  1. Check WAL for uncommitted frames
  2. Read main DB + committed WAL frames (snapshot)
  3. Never blocks writers

WRITER:
  1. Write new frames to WAL
  2. Increment LSN
  3. When COMMIT: Mark frames as committed
  4. Checkpoint: Move committed frames to main DB
```

**Frame Format**:

```
WAL Frame:
┌──────────────────────────┐
│ Frame Header (24 bytes)  │
│  - Magic: 0x3d9d04d9    │
│  - Sequence: u32        │
│  - Page Count: u32      │
│  - Timestamp: u64       │
├──────────────────────────┤
│ Page Data (4KB default)  │
├──────────────────────────┤
│ Checksum (4 bytes)       │
│  CRC32(header+data)     │
└──────────────────────────┘
```

**Checkpoint Modes**:

| Mode | Behavior | When Used |
|------|----------|-----------|
| PASSIVE | Move committed frames, don't wait for readers | Background, default |
| RESTART | Block new readers, then checkpoint | App idle |
| FULL | Block readers, block writers, then checkpoint | Aggressive |
| OFF | Disable checkpointing | Benchmarking |

### 4.3 Transaction Isolation Levels

**Snapshot Isolation Implementation**:

```rust
pub struct TransactionContext {
    txn_id: u64,
    start_lsn: LogSequenceNumber,  // Start of transaction
    snapshot_version: PageVersion,
    isolation_level: IsolationLevel,
}

// Reader sees all versions >= start_lsn
// Writer collects changes in write_set
// On commit: Write WAL frames, increment LSN

// Example:
// Writer1 commits at LSN 100
// Reader starts at LSN 99 → doesn't see Writer1's changes
// Reader starts at LSN 101 → sees Writer1's changes
```

**Conflict Detection** (for Serializable mode):

```rust
pub fn detect_conflicts(
    txn: &Transaction,
    database: &Database,
) -> Result<ConflictType> {
    // Compare read_set and write_set against other active txns

    // Dirty reads: Read uncommitted data
    // Non-repeatable reads: Same query returns different results
    // Phantom reads: WHERE clause matches different rows

    if serializable_mode {
        return Err(ConflictError::SerializationFailure);
    } else {
        // Snapshot isolation allows, ignore
        return Ok(());
    }
}
```

---

## 5. Error Handling & Recovery

### 5.1 Error Type Hierarchy

```rust
#[derive(Debug)]
pub enum CypherLiteError {
    // File I/O errors
    IoError(std::io::Error),
    FileCorrupted { reason: String },

    // Storage errors
    PageNotFound(PageId),
    RecordNotFound(RecordId),
    StorageQuotaExceeded,

    // Transaction errors
    TransactionAborted,
    SerializationFailure,
    DeadlockDetected,

    // Query errors
    SyntaxError { message: String, position: usize },
    UnresolvedReference { name: String },
    TypeMismatch { expected: String, actual: String },

    // Configuration errors
    InvalidConfiguration { reason: String },

    // Concurrency errors
    LockTimeout,
    LockDeadlock,
}

impl std::error::Error for CypherLiteError { ... }
impl std::fmt::Display for CypherLiteError { ... }

pub type Result<T> = std::result::Result<T, CypherLiteError>;
```

### 5.2 Crash Recovery via WAL

**Recovery Process on Startup**:

```
1. OPEN DATABASE
   ├─ Read main DB header
   ├─ Check for WAL file existence
   │
   ├─ [If WAL exists]
   │  ├─ Scan WAL for committed frames
   │  ├─ Validate checksums
   │  ├─ Determine last good state
   │  │
   │  ├─ [If crash detected]
   │  │  ├─ Replay committed WAL frames to main DB
   │  │  ├─ Discard uncommitted frames
   │  │  ├─ Update catalog
   │  │  └─ Mark recovery complete
   │  │
   │  └─ Move WAL frames to main DB (checkpoint)
   │
   └─ [If no WAL]
      └─ Use main DB as-is

2. VALIDATE STRUCTURES
   ├─ Recompute catalog from metadata pages
   ├─ Verify all indices
   └─ Check referential integrity

3. READY FOR CONNECTIONS
```

**Crash Scenarios Handled**:

1. **Crash during write**: WAL contains uncommitted frames → discard
2. **Crash before checkpoint**: WAL has committed frames → replay
3. **Corrupted main DB**: Can rebuild from WAL
4. **Corrupted WAL**: Fall back to main DB state, lose recent changes
5. **Partial frame write**: Checksum mismatch → mark invalid, skip

### 5.3 Checkpointing Strategy

**Automatic Checkpointing**:

```rust
pub struct CheckpointConfig {
    pub mode: CheckpointMode,
    pub auto_checkpoint: bool,
    pub wal_size_threshold: usize,    // Default: 1000 pages
    pub time_threshold: Duration,     // Default: 5 minutes
}

pub async fn run_checkpoint_daemon(
    db: Arc<Database>,
    config: CheckpointConfig,
) {
    loop {
        tokio::select! {
            _ = tokio::time::sleep(config.time_threshold) => {
                if should_checkpoint(db.wal.size()) {
                    db.checkpoint()?;
                }
            }
        }
    }
}

pub fn checkpoint(&self, mode: CheckpointMode) -> Result<()> {
    // 1. Acquire write lock (or wait for readers in PASSIVE mode)
    // 2. Read committed frames from WAL
    // 3. Apply frames to main DB
    // 4. Update main DB header (LSN, checkpoint timestamp)
    // 5. Truncate WAL (or recycle)
    // 6. Release lock
}
```

**Corruption Detection**:

```rust
pub struct PageHeader {
    pub page_id: PageId,
    pub page_type: PageType,
    pub checksum: u32,
    pub timestamp: u64,
    pub lsn: LogSequenceNumber,
}

pub fn verify_page_integrity(page: &[u8]) -> Result<()> {
    let header = PageHeader::parse(&page[0..24])?;
    let payload = &page[24..];

    let computed_checksum = crc32_fast::hash(payload);

    if computed_checksum != header.checksum {
        return Err(CypherLiteError::FileCorrupted {
            reason: format!(
                "Page {} checksum mismatch: expected {}, got {}",
                header.page_id, header.checksum, computed_checksum
            ),
        });
    }

    Ok(())
}
```

---

## 6. Configuration & Initialization

### 6.1 Database Creation Flow

```rust
pub struct DatabaseConfig {
    // Storage
    pub page_size: usize,                   // 512..65536, default: 4096
    pub data_file_path: PathBuf,

    // Concurrency
    pub wal_enabled: bool,                  // default: true
    pub journal_mode: JournalMode,          // WAL or ROLLBACK
    pub max_readers: usize,                 // default: unlimited

    // Caching
    pub page_cache_size: usize,             // default: 10MB
    pub cache_eviction_policy: EvictionPolicy,
    pub hot_node_cache_size: usize,         // default: 10K nodes

    // Durability
    pub synchronous: SynchronousMode,       // FULL, NORMAL, OFF
    pub checkpoint_mode: CheckpointMode,
    pub checkpoint_interval: Duration,

    // Performance
    pub mmap_size: usize,                   // 0 for disabled, default: 256MB
    pub memory_limit: usize,                // Total memory budget

    // Analysis & Debugging
    pub enable_query_profiling: bool,       // default: false
    pub enable_tracing: bool,               // default: false
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        DatabaseConfig {
            page_size: 4096,
            data_file_path: PathBuf::from("cypherlite.db"),
            wal_enabled: true,
            journal_mode: JournalMode::WAL,
            max_readers: usize::MAX,
            page_cache_size: 10 * 1024 * 1024,  // 10MB
            cache_eviction_policy: EvictionPolicy::LRU,
            hot_node_cache_size: 10_000,
            synchronous: SynchronousMode::NORMAL,
            checkpoint_mode: CheckpointMode::Passive,
            checkpoint_interval: Duration::from_secs(300),
            mmap_size: 256 * 1024 * 1024,       // 256MB
            memory_limit: 1024 * 1024 * 1024,   // 1GB
            enable_query_profiling: false,
            enable_tracing: false,
        }
    }
}

pub enum JournalMode {
    ROLLBACK,  // Traditional rollback journal
    WAL,       // Write-Ahead Logging (better concurrency)
}

pub enum SynchronousMode {
    FULL,      // All syncs explicit, safest
    NORMAL,    // Balance between safety and speed (default)
    OFF,       // No fsync calls, fastest but risky
}

pub enum CheckpointMode {
    PASSIVE,   // Checkpoint without blocking readers
    RESTART,   // Block new readers, checkpoint existing
    FULL,      // Block all readers and writers
}
```

**Creation/Opening**:

```rust
impl Database {
    pub fn create(path: &str, config: DatabaseConfig) -> Result<Self> {
        // 1. Check file doesn't exist
        if Path::new(path).exists() {
            return Err(CypherLiteError::InvalidConfiguration {
                reason: "Database file already exists".to_string(),
            });
        }

        // 2. Create file with header
        let mut file = File::create(path)?;
        write_database_header(&mut file, &config)?;

        // 3. Initialize metadata pages (catalog, free list)
        initialize_metadata_pages(&mut file, &config)?;

        // 4. Wrap in Database struct
        Ok(Database {
            inner: Arc::new(RwLock::new(DatabaseInner {
                file,
                config,
                catalog: Catalog::new(),
                wal: WriteAheadLog::new(),
                ..Default::default()
            })),
        })
    }

    pub fn open(path: &str, config: Option<DatabaseConfig>) -> Result<Self> {
        // 1. Open file in read-write mode
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(path)?;

        // 2. Read and validate header
        let header = DatabaseHeader::read(&mut file)?;
        validate_header(&header)?;

        // 3. Load configuration (merge with provided config)
        let config = config.unwrap_or_default();

        // 4. Recover from WAL if necessary
        let wal = WriteAheadLog::open(&file)?;
        let recovery_needed = wal.has_uncommitted_frames();
        if recovery_needed {
            recover_from_wal(&mut file, &wal)?;
        }

        // 5. Load catalog
        let catalog = Catalog::load_from_file(&mut file)?;

        // 6. Create Database instance
        Ok(Database {
            inner: Arc::new(RwLock::new(DatabaseInner {
                file,
                config,
                catalog,
                wal,
                ..Default::default()
            })),
        })
    }

    pub fn open_in_memory() -> Result<Self> {
        // Create temporary file in memory (using MemFile crate)
        let file = MemFile::new();
        let config = DatabaseConfig {
            wal_enabled: false,  // No WAL for in-memory
            ..Default::default()
        };

        write_database_header(&file, &config)?;
        initialize_metadata_pages(&file, &config)?;

        Ok(Database {
            inner: Arc::new(RwLock::new(DatabaseInner {
                file,
                config,
                ..Default::default()
            })),
        })
    }
}

pub struct DatabaseHeader {
    pub magic: [u8; 16],                    // "CypherLite" + version
    pub page_size: u32,
    pub wal_mode: bool,
    pub checkpoint_lsn: u64,
    pub catalog_offset: u64,
    pub free_list_offset: u64,
    pub user_version: u32,
}
```

### 6.2 Opening Existing Databases

**Validation Checklist**:

1. File exists and readable
2. Magic number matches ("CypherLite\0\0\0\0\0")
3. Page size matches configuration
4. WAL file consistency (if exists)
5. Catalog parseable
6. No corruption detected in first few pages

```rust
pub fn validate_header(header: &DatabaseHeader) -> Result<()> {
    // Check magic
    if &header.magic[0..11] != b"CypherLite\0" {
        return Err(CypherLiteError::FileCorrupted {
            reason: "Invalid magic number".to_string(),
        });
    }

    // Check page size validity
    if header.page_size < 512 || header.page_size > 65536 {
        return Err(CypherLiteError::InvalidConfiguration {
            reason: format!("Invalid page size: {}", header.page_size),
        });
    }

    Ok(())
}
```

---

## 7. Language Choice Justification

### 7.1 Why Rust

**Memory Safety Without Garbage Collection**
- No GC pauses → predictable latency for embedded scenarios
- Eliminates entire classes of bugs: use-after-free, double-free, buffer overflows
- RAII (Resource Acquisition Is Initialization) ensures cleanup
- Safe concurrency with Rust's borrow checker

**Performance**
- Zero-cost abstractions (compile-time polymorphism)
- No runtime overhead for memory management
- Competitive with C/C++ performance
- Excellent for storage engines requiring tight I/O loops

**FFI and Polyglot Deployment**
- First-class C FFI support (rustc generates C-compatible binaries)
- Python bindings via PyO3 (near-native performance)
- Node.js bindings via NAPI (native modules)
- No garbage collector complicates bindings (unlike Java/Go)

**Ecosystem for Systems Programming**
- `tokio` for async I/O (background tasks, checkpointing)
- `parking_lot` for fast synchronization primitives
- `byteorder`, `crc32fast` for binary serialization
- `nom` for parser combinators (Cypher parsing)
- `indexmap`, `lru` for efficient data structures

**Single-Binary Distribution**
- Compile to static library → ship with Python/Node/C projects
- No runtime dependency hell
- Small binary size (~2-5MB for core engine)

**Prevents Common Database Bugs**
- Thread-safety enforced at compile-time
- Mutex/RwLock ownership prevents deadlock patterns
- Slice bounds checked automatically
- Integer overflow caught in debug builds

### 7.2 Key Rust Crates to Leverage

**Core I/O & Serialization**:
- `byteorder`: Endian-aware integer serialization
- `crc32fast`: Fast checksums for page integrity
- `bytes`: Zero-copy byte buffers
- `parking_lot`: Fast RwLock, Mutex (faster than std)

**Data Structures**:
- `lru`: LRU cache eviction (page cache, node cache)
- `indexmap`: Insertion-ordered HashMap (catalog)
- `btree-map` (std): B-tree for indices
- `hashbrown`: High-performance HashMap (stdlib future)

**Parsing**:
- `nom`: Parser combinator library (Cypher lexer/parser)
- `winnow`: Faster nom successor (consider for v2)

**Error Handling**:
- `thiserror`: Ergonomic error types with From impls
- `anyhow`: Flexible error handling for CLI/bindings

**Async & Concurrency**:
- `tokio`: Async runtime (background checkpoint daemon)
- `crossbeam`: Thread-safe queues and synchronization
- `parking_lot`: Better-than-std locking

**Logging & Diagnostics**:
- `tracing`: Structured logging (async-friendly)
- `tracing-subscriber`: Logging configuration
- `criterion`: Benchmarking framework

**FFI Bindings**:
- `pyo3`: Python native extension (high-performance)
- `napi-rs`: Node.js native module bindings
- `libc`: C standard library types

**Testing**:
- `proptest`: Property-based testing
- `tempfile`: Temporary file handling for tests
- `criterion`: Benchmarking (see above)

### 7.3 Avoided Technologies

**Why Not C**?
- Manual memory management → exploit surface, crashes
- No standard package management (cargo)
- Verbose, higher cognitive load
- Longer build times

**Why Not C++**?
- Complexity and steep learning curve
- Still has memory safety issues (despite RAII)
- ABI compatibility headaches
- Harder to generate bindings for other languages

**Why Not Python**?
- Interpreter overhead unsuitable for storage engine
- GC pauses problematic for database latency
- No static typing (harder to catch bugs)
- Poor FFI into other languages

**Why Not Go**?
- Garbage collection unsuitable for embedded DBs
- GC pauses unpredictable (10-100ms typical)
- Larger binary footprint (~5-10MB)
- Weaker type system than Rust

**Why Not Java/JVM**?
- JVM startup time (cold start)
- GC pauses (10-500ms for large heaps)
- Embedded in JVM application mostly necessary
- FFI complexity with Java/Python interop

---

## 8. Implementation Roadmap

### 8.1 Phase 1: Core Foundation (Months 1-2)

**Deliverables**:
- File format specification
- Page-based storage with B-trees
- Basic node/relationship storage
- Single-threaded query execution

**Modules**:
- `storage::format` (page layout, headers)
- `storage::node_store` (node records)
- `storage::relationship_store` (relationship records)
- `io::page_io` (read/write pages)
- `cache::page_cache` (basic LRU)
- `api::native_api` (direct DB access)

**Testing**:
- Unit tests for page I/O
- Integration tests for node/rel creation
- Property storage tests

### 8.2 Phase 2: Transactions & WAL (Months 3-4)

**Deliverables**:
- Write-Ahead Log implementation
- Transaction manager with isolation levels
- Checkpoint mechanism
- Crash recovery

**Modules**:
- `storage::wal` (WAL frame format)
- `storage::checkpoint` (checkpointing logic)
- `storage::recovery` (crash recovery)
- `query::transaction` (transaction boundaries)

**Testing**:
- Crash simulation tests
- Concurrent transaction tests
- WAL frame integrity tests

### 8.3 Phase 3: Query Engine (Months 5-6)

**Deliverables**:
- Cypher parser (subset)
- Query planner and optimizer
- Pattern matching execution
- Basic aggregations

**Modules**:
- `query::parser::lexer` (tokenization)
- `query::parser::ast` (syntax tree)
- `query::planner` (logical plan)
- `query::optimizer` (cost estimation)
- `query::executor` (plan execution)

**Testing**:
- Parser unit tests (lexer, grammar)
- End-to-end query tests
- Query optimization benchmarks

### 8.4 Phase 4: Indices & Performance (Months 7-8)

**Deliverables**:
- Label indices
- Relationship type indices
- Multi-level caching (hot node, adjacency)
- Query result streaming

**Modules**:
- `storage::index_store` (index management)
- `cache::node_cache` (hot node cache)
- `cache::adjacency_cache` (adjacency lists)
- `api::result` (streaming results)

**Testing**:
- Index lookup performance tests
- Cache hit rate benchmarks
- Large result set streaming

### 8.5 Phase 5: FFI & Bindings (Months 9-10)

**Deliverables**:
- C API (cypherlite-ffi)
- Python bindings (PyO3)
- Node.js bindings (NAPI)

**Modules**:
- `cypherlite-ffi/c_api.rs`
- `cypherlite-python/lib.rs`
- `cypherlite-node/lib.rs`

**Testing**:
- Python integration tests
- Node.js integration tests
- Binding API correctness

### 8.6 Phase 6: Production Hardening (Months 11-12)

**Deliverables**:
- Comprehensive error handling
- Configuration validation
- Performance tuning
- Documentation

**Activities**:
- Fuzz testing (AFL for parser)
- Load testing (concurrent readers/writers)
- Memory profiling
- Benchmark suite
- Documentation (API docs, guides)

---

## Architecture Design Summary

CypherLite's core architecture combines **SQLite's proven simplicity** (single-file, zero-config, WAL-based transactions) with **Neo4j's graph-native approach** (index-free adjacency, Cypher queries, native graph storage).

### Key Design Decisions:

1. **Single File**: Portable, atomic backups, simple deployment
2. **Page-Based Storage**: 4KB pages (configurable), following SQLite
3. **B-Tree Indices**: Standard indexing structure for all stores
4. **WAL Transactions**: ACID guarantees + concurrent read support
5. **Multi-Level Caching**: Pages, hot nodes, relationship adjacency
6. **Index-Free Adjacency**: O(1) traversal via relationship pointers
7. **Rust Implementation**: Memory safety, performance, FFI-friendly
8. **Snapshot Isolation**: Default transaction isolation for concurrency

### Target Use Cases:

- **Edge Computing**: Deploy graphs on IoT devices
- **LLM Agents**: Local knowledge base storage (GraphRAG)
- **Mobile Apps**: Embedded graph database on phones
- **Local-First Apps**: Self-contained databases with offline-first sync
- **Analytical Workloads**: In-process graph analysis

This architecture provides a strong foundation for a production-ready embedded graph database while maintaining the simplicity that makes SQLite successful.

---

**Next Steps**:
1. File format specification (storage/format.md)
2. Cypher grammar specification (query/grammar.md)
3. Transaction protocol specification (storage/transactions.md)
4. API reference documentation
5. Performance benchmark suite design
