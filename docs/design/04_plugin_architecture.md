# CypherLite Plugin Architecture Design

## Executive Summary

CypherLite adopts a **plugin-first architecture** where the core engine is minimal and all extensibility—from custom storage formats to domain-specific query functions to RAG capabilities—flows through a standardized plugin system. This design enables CypherLite to serve diverse use cases (agent memory, knowledge graphs, GraphRAG pipelines) without baking every feature into the core.

The plugin system is guided by three principles:
1. **Minimal Core:** Core handles only graph storage, basic traversal, and transaction management
2. **Pluggable Features:** Custom index types, query functions, storage formats, and business logic as plugins
3. **Composable:** Plugins communicate via event bus and shared services, enabling combinations like "GraphRAG + Vector Index + Semantic Layer"

---

## 1. Plugin System Overview

### 1.1 Design Philosophy: Core as Runtime, Plugins as Features

CypherLite's architecture inverts the traditional database model:

**Traditional Model:**
```
┌─────────────────────────────────────────┐
│         Database (Monolithic)           │
│  ├─ Storage                             │
│  ├─ Query Execution                     │
│  ├─ Indexing (B-Tree, Hash)            │
│  ├─ Caching                             │
│  └─ Special Features                    │
└─────────────────────────────────────────┘
```

**CypherLite Model:**
```
┌──────────────────────────────────────────────────────────────┐
│              Plugin Host (CypherLite Core)                   │
│  ┌──────────────────────────────────────────────────────┐   │
│  │  Minimal Runtime                                      │   │
│  │  ├─ Transaction Manager (ACID)                       │   │
│  │  ├─ Page Buffer Manager                              │   │
│  │  ├─ Basic Graph Traversal                            │   │
│  │  ├─ Event Dispatcher                                 │   │
│  │  └─ Plugin Loader & Registry                         │   │
│  └──────────────────────────────────────────────────────┘   │
│                                                              │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐      │
│  │   Storage    │  │   Index      │  │   Query      │      │
│  │   Plugins    │  │   Plugins    │  │   Plugins    │      │
│  ├──────────────┤  ├──────────────┤  ├──────────────┤      │
│  │ • File-based │  │ • HNSW       │  │ • Cypher     │      │
│  │ • Mmap       │  │ • IVF Vector │  │   Functions  │      │
│  │ • Custom     │  │ • Full-text  │  │ • Procedures │      │
│  │   Format     │  │ • Spatial    │  │ • Semantic   │      │
│  └──────────────┘  └──────────────┘  └──────────────┘      │
│                                                              │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐      │
│  │  Serializer  │  │   Event      │  │   Business   │      │
│  │   Plugins    │  │   Plugins    │  │   Logic      │      │
│  ├──────────────┤  ├──────────────┤  ├──────────────┤      │
│  │ • RDF/OWL    │  │ • Hooks      │  │ • GraphRAG   │      │
│  │ • JSON-LD    │  │ • Triggers   │  │ • Semantic   │      │
│  │ • GraphML    │  │ • Validation │  │   Layer      │      │
│  │ • CSV        │  │ • Logging    │  │ • Kinetic    │      │
│  └──────────────┘  └──────────────┘  │   Layer      │      │
│                                        │ • Dynamic    │      │
│                                        │   Layer      │      │
│                                        └──────────────┘      │
└──────────────────────────────────────────────────────────────┘
```

### 1.2 Plugin Lifecycle

Every plugin follows a standard lifecycle:

```
1. DISCOVER
   └─ Plugin registry scans plugin directory
   └─ Reads plugin metadata (name, version, dependencies)
   └─ Validates compatibility with core version

2. LOAD
   └─ Statically linked plugins: linked at compile time
   └─ Dynamic plugins: dlopen() loads shared library
   └─ Core holds reference to plugin handle

3. INITIALIZE
   └─ Plugin receives configuration
   └─ Plugin allocates resources (memory, file handles, indices)
   └─ Plugin registers hooks/handlers with core
   └─ Plugin declares supported storage regions in file

4. USE
   └─ Queries route to appropriate plugin based on operation type
   └─ Plugins receive events from event dispatcher
   └─ Plugins call core services (logging, metrics, config)

5. SHUTDOWN
   └─ Plugin cleans up resources (flush indices, close files)
   └─ Plugin unregisters hooks
   └─ Plugin manager confirms clean shutdown
```

### 1.3 Plugin Types

CypherLite defines six plugin types, each addressing different extensibility requirements:

#### **1. Storage Extension Plugins**
Enable alternative storage backends and custom serialization formats.

**Examples:**
- Alternative file layouts (e.g., mmap-based zero-copy access)
- Custom page types (e.g., compressed pages, encrypted pages)
- In-memory storage for testing
- Cloud-backed storage (S3, GCS)

**Use Cases:**
- Performance optimization (mmap vs. traditional paging)
- Security (encryption plugins)
- Durability (multi-replica backends)

#### **2. Index Extension Plugins**
Provide custom index implementations beyond the core B-tree.

**Examples:**
- HNSW vector index for approximate nearest-neighbor search
- Inverted Full-Text Index for text search
- Spatial indexes (R-tree, QuadTree) for geographic queries
- Bitmap indexes for boolean properties
- Trie-based prefix indexes

**Use Cases:**
- Semantic search via vector embeddings
- Full-text search on node descriptions
- Geographic queries ("find entities near coordinates")
- Boolean filters with high cardinality

#### **3. Query Extension Plugins**
Extend Cypher with custom functions, procedures, and aggregations.

**Examples:**
- Custom string functions (`SIMILARITY(s1, s2)`)
- Custom aggregations (`GRAPH_DIAMETER()`, `COMMUNITY_DETECT()`)
- Path-finding algorithms (`SHORTEST_PATH()`, `ALL_PATHS()`)
- Statistical functions (`PERCENTILE()`, `STDDEV()`)
- Domain-specific functions (`SENTIMENT_SCORE()`, `ENTITY_TYPE_PROB()`)

**Use Cases:**
- Domain-specific queries (e.g., bioinformatics has unique functions)
- GraphRAG-specific functions (community detection, summarization)
- Machine learning integration (embeddings, classification)

#### **4. Serializer Extension Plugins**
Enable import/export in various graph formats.

**Examples:**
- RDF/OWL serialization (triple format)
- JSON-LD (linked data format)
- GraphML (XML-based graph format)
- CSV import (bulk-load from CSV files)
- Neo4j export format (for interoperability)

**Use Cases:**
- Data exchange with other tools (Cypher, RDF stores)
- Backup/export workflows
- Bulk data import from CSV sources
- Format conversion pipelines

#### **5. Event Extension Plugins (Hooks)**
React to graph mutations with custom business logic.

**Examples:**
- Validation hooks (enforce constraints before commit)
- Logging hooks (audit trail of all mutations)
- Cache invalidation hooks (update secondary indices)
- Notification hooks (trigger webhooks on certain changes)
- GraphRAG hooks (update community summaries after mutations)

**Use Cases:**
- Schema validation (only allow valid type combinations)
- Audit compliance (log who changed what and when)
- Cache coherency (keep derived structures consistent)
- Downstream notifications (inform agents of changes)

#### **6. Business Logic Plugins**
Higher-level semantic and operational logic.

**Examples:**
- Semantic Layer: Define object types, link types, property schemas
- Kinetic Layer: Define actions (Create, Update, Delete) with authorization
- Dynamic Layer: Define scenarios and simulations
- GraphRAG Pipeline: Entity extraction, community detection, summarization
- HTTP/gRPC API: REST endpoints and WebSocket support

**Use Cases:**
- Domain modeling (define valid entity/relationship types)
- Workflow management (define and execute business processes)
- Agent authorization (agents can only perform specific actions)
- Advanced RAG (full GraphRAG pipeline including LLM integration)

---

## 2. Plugin Interface (Trait System in Rust)

### 2.1 Core Plugin Trait (All Plugins Implement)

```rust
/// Base trait that all plugins must implement
pub trait Plugin: Send + Sync {
    /// Return metadata about this plugin
    fn metadata(&self) -> PluginMetadata;

    /// Initialize plugin with configuration
    fn initialize(&mut self, config: PluginConfig, registry: &PluginRegistry)
        -> Result<(), PluginError>;

    /// Shutdown plugin and clean up resources
    fn shutdown(&mut self) -> Result<(), PluginError>;

    /// Called periodically to allow housekeeping (defragmentation, etc.)
    fn maintenance(&mut self) -> Result<(), PluginError> {
        Ok(())
    }
}

/// Plugin metadata: name, version, dependencies, capabilities
pub struct PluginMetadata {
    pub name: String,
    pub version: semver::Version,
    pub author: String,
    pub description: String,
    pub dependencies: Vec<PluginDependency>,
    pub capabilities: PluginCapabilities,
    pub min_core_version: semver::Version,
    pub max_core_version: Option<semver::Version>,
}

pub struct PluginDependency {
    pub name: String,
    pub version_requirement: semver::VersionReq,
}

pub enum PluginCapabilities {
    Storage { page_types: Vec<String> },
    Index { index_types: Vec<String> },
    Query { functions: Vec<String> },
    Serializer { formats: Vec<String> },
    Event,
    BusinessLogic,
}

pub struct PluginConfig {
    pub settings: std::collections::BTreeMap<String, serde_json::Value>,
}
```

### 2.2 Storage Plugin Trait

```rust
/// Plugin that provides custom storage backends
pub trait StoragePlugin: Plugin {
    /// Create a new page of custom type
    fn create_page(&mut self, page_type: &str, page_id: PageId)
        -> Result<Box<dyn Page>, PluginError>;

    /// Read existing page
    fn read_page(&self, page_id: PageId)
        -> Result<Box<dyn Page>, PluginError>;

    /// Write page to storage
    fn write_page(&mut self, page: &dyn Page)
        -> Result<(), PluginError>;

    /// Delete page
    fn delete_page(&mut self, page_id: PageId)
        -> Result<(), PluginError>;

    /// Flush all pending changes to stable storage
    fn flush(&mut self) -> Result<(), PluginError>;

    /// Allocate page range for this plugin's use
    fn allocate_page_range(&mut self, count: u32)
        -> Result<PageRange, PluginError>;
}

/// Trait that pages must implement
pub trait Page: Send + Sync {
    fn page_id(&self) -> PageId;
    fn page_type(&self) -> &str;
    fn as_bytes(&self) -> &[u8];
    fn as_bytes_mut(&mut self) -> &mut [u8];
}

pub type PageId = u32;

pub struct PageRange {
    pub start: PageId,
    pub end: PageId,
}
```

### 2.3 Index Plugin Trait

```rust
/// Plugin that provides custom index implementations
pub trait IndexPlugin: Plugin {
    /// Index type this plugin provides (e.g., "hnsw_vector", "full_text")
    fn index_type(&self) -> &str;

    /// Create a new index
    fn create_index(&mut self, index_id: IndexId, config: IndexConfig)
        -> Result<(), PluginError>;

    /// Add item to index
    fn insert(&mut self, index_id: IndexId, key: NodeId, value: &IndexValue)
        -> Result<(), PluginError>;

    /// Remove item from index
    fn delete(&mut self, index_id: IndexId, key: NodeId)
        -> Result<(), PluginError>;

    /// Query the index
    fn search(&self, index_id: IndexId, query: &IndexQuery)
        -> Result<Vec<(NodeId, f32)>, PluginError>;

    /// Bulk index operation
    fn build(&mut self, index_id: IndexId, items: Vec<(NodeId, IndexValue)>)
        -> Result<(), PluginError>;

    /// Flush index to storage
    fn flush(&mut self, index_id: IndexId)
        -> Result<(), PluginError>;
}

pub type IndexId = u32;

pub struct IndexConfig {
    pub index_type: String,
    pub settings: std::collections::BTreeMap<String, serde_json::Value>,
}

pub enum IndexValue {
    Vector(Vec<f32>),
    Text(String),
    Point { x: f64, y: f64 },
    Bitmap(Vec<bool>),
    Custom(Vec<u8>),
}

pub enum IndexQuery {
    VectorKNN { vector: Vec<f32>, k: usize },
    FullTextPhrase { phrase: String, limit: usize },
    SpatialRadius { x: f64, y: f64, radius: f64 },
    BitmapBits { bits: Vec<u8>, match_all: bool },
    Custom { payload: Vec<u8> },
}

pub type NodeId = u64;
```

### 2.4 Query Plugin Trait

```rust
/// Plugin that extends Cypher with custom functions/procedures
pub trait QueryPlugin: Plugin {
    /// Register this plugin's functions/procedures
    fn register_functions(&self) -> Vec<FunctionDefinition>;

    /// Execute a custom function
    fn execute_function(&self, name: &str, args: Vec<Value>)
        -> Result<Value, PluginError>;

    /// Execute a custom procedure (may have side effects)
    fn execute_procedure(&mut self, name: &str, args: Vec<Value>)
        -> Result<Vec<Row>, PluginError>;
}

pub struct FunctionDefinition {
    pub name: String,
    pub description: String,
    pub params: Vec<FunctionParam>,
    pub return_type: ValueType,
    pub deterministic: bool, // Can be cached/optimized?
}

pub struct FunctionParam {
    pub name: String,
    pub value_type: ValueType,
    pub optional: bool,
}

pub enum ValueType {
    Number,
    String,
    Boolean,
    List(Box<ValueType>),
    Map,
    Node,
    Relationship,
    Path,
    Any,
}

pub enum Value {
    Null,
    Boolean(bool),
    Integer(i64),
    Float(f64),
    String(String),
    List(Vec<Value>),
    Map(std::collections::BTreeMap<String, Value>),
    // Graph types
    Node(NodeValue),
    Relationship(RelValue),
    Path(PathValue),
}

pub type Row = std::collections::BTreeMap<String, Value>;
```

### 2.5 Serializer Plugin Trait

```rust
/// Plugin that enables import/export in various formats
pub trait SerializerPlugin: Plugin {
    /// Format this plugin handles (e.g., "rdf", "json-ld")
    fn format(&self) -> &str;

    /// Export graph to format
    fn export(&self, graph: &GraphSnapshot)
        -> Result<Vec<u8>, PluginError>;

    /// Import graph from format, returning list of mutations
    fn import(&self, data: &[u8])
        -> Result<Vec<GraphMutation>, PluginError>;

    /// Can this plugin handle streaming import?
    fn supports_streaming(&self) -> bool {
        false
    }

    /// Import streaming data (if supported)
    fn import_stream<R: std::io::Read>(&self, reader: R)
        -> Result<Vec<GraphMutation>, PluginError> {
        Err(PluginError::Unsupported("Streaming not supported".to_string()))
    }
}

pub struct GraphSnapshot {
    pub nodes: Vec<NodeSnapshot>,
    pub relationships: Vec<RelSnapshot>,
}

pub struct NodeSnapshot {
    pub id: NodeId,
    pub labels: Vec<String>,
    pub properties: std::collections::BTreeMap<String, Value>,
}

pub struct RelSnapshot {
    pub id: RelId,
    pub rel_type: String,
    pub source_id: NodeId,
    pub target_id: NodeId,
    pub properties: std::collections::BTreeMap<String, Value>,
}

pub enum GraphMutation {
    CreateNode { id: NodeId, labels: Vec<String>, properties: BTreeMap<String, Value> },
    CreateRel { id: RelId, rel_type: String, source: NodeId, target: NodeId, properties: BTreeMap<String, Value> },
    UpdateNode { id: NodeId, properties: BTreeMap<String, Value> },
    UpdateRel { id: RelId, properties: BTreeMap<String, Value> },
    DeleteNode { id: NodeId },
    DeleteRel { id: RelId },
}
```

### 2.6 Event Plugin Trait

```rust
/// Plugin that reacts to graph mutations
pub trait EventPlugin: Plugin {
    /// Register which events this plugin wants to handle
    fn subscribed_events(&self) -> Vec<EventType>;

    /// Handle a graph event
    fn on_event(&mut self, event: &GraphEvent)
        -> Result<EventResponse, PluginError>;
}

pub enum EventType {
    BeforeCreateNode,
    AfterCreateNode,
    BeforeUpdateNode,
    AfterUpdateNode,
    BeforeDeleteNode,
    AfterDeleteNode,
    BeforeCreateRel,
    AfterCreateRel,
    BeforeUpdateRel,
    AfterUpdateRel,
    BeforeDeleteRel,
    AfterDeleteRel,
    TransactionCommit,
    TransactionRollback,
}

pub struct GraphEvent {
    pub event_type: EventType,
    pub timestamp: std::time::SystemTime,
    pub transaction_id: u64,
    pub mutation: GraphMutation,
    pub context: EventContext,
}

pub struct EventContext {
    pub user_id: Option<String>,
    pub source: String, // "query", "api", "import", etc.
}

pub enum EventResponse {
    Allow,
    Block(String), // Reason for blocking
    Intercept(Vec<GraphMutation>), // Modify the mutation
}
```

### 2.7 Plugin Metadata and Configuration

```rust
/// Configuration schema for a plugin
pub struct PluginConfigSchema {
    pub properties: std::collections::BTreeMap<String, ConfigProperty>,
    pub required: Vec<String>,
}

pub struct ConfigProperty {
    pub property_type: ConfigType,
    pub description: String,
    pub default: Option<serde_json::Value>,
}

pub enum ConfigType {
    String { pattern: Option<String> },
    Integer { min: Option<i64>, max: Option<i64> },
    Boolean,
    Number { min: Option<f64>, max: Option<f64> },
    Array { items: Box<ConfigType> },
    Object { schema: Box<PluginConfigSchema> },
}
```

---

## 3. Planned Plugin Modules (Future)

### 3.1 Vector Index Plugin

**Purpose:** Enable semantic search capabilities in graphs, essential for RAG and AI agent memory.

**Core Functionality:**

```rust
pub struct VectorIndexPlugin {
    config: VectorIndexConfig,
    indices: BTreeMap<IndexId, HNSWIndex>,
}

pub struct VectorIndexConfig {
    pub embedding_dim: usize,
    pub max_m: usize,           // HNSW max neighbors
    pub ef_construction: usize,  // HNSW construction parameter
    pub ef_search: usize,        // HNSW search parameter
}
```

**Features:**

1. **HNSW Index Implementation**
   - Approximate nearest-neighbor search
   - O(log N) query time
   - ~20% memory overhead
   - Suitable for high-dimensional embeddings (256-1536 dims typical)

2. **Embedding Storage**
   - Store embeddings as node properties
   - Support multiple embedding fields per node (e.g., title_embedding, description_embedding)
   - Lazy-load embeddings (not all nodes need embeddings)

3. **Cypher Extension**
   ```cypher
   // Search by embedding similarity
   MATCH (n:Decision)
   WHERE vector.similarity(n.embedding, $query_vector) > 0.8
   RETURN n, vector.similarity(n.embedding, $query_vector) AS score
   ORDER BY score DESC
   LIMIT 10

   // Hybrid search: vector + graph structure
   MATCH (n:Decision)
   WHERE vector.similarity(n.embedding, $query_vector) > 0.75
   MATCH (n) -[:AFFECTS]-> (s:Service)
   RETURN n, s, vector.similarity(n.embedding, $query_vector) AS relevance
   ```

4. **Hybrid Search Pattern**
   - Use vector similarity to identify candidate region of graph
   - Apply graph structure filtering on candidates
   - Combine relevance scores from both dimensions
   - Example: "Find decisions related to (vector) performance that affect the (graph) API service"

5. **Update Strategy**
   - Online insertion: Add new embeddings without full rebuild
   - Batch updates: Periodic HNSW index reconstruction for large batches
   - Deletion: Mark embeddings as inactive, periodic compaction

**Use Cases in CypherLite:**
- Agent memory: Semantic search over decision history
- GraphRAG: Find semantically similar communities
- Knowledge graph: Discover related entities by semantic meaning

**Configuration:**
```yaml
plugins:
  vector_index:
    enabled: true
    embedding_dim: 768  # e.g., for all-MiniLM-L6-v2
    max_m: 16
    ef_construction: 200
    ef_search: 100
```

---

### 3.2 Semantic Layer Plugin (Palantir-Inspired)

**Purpose:** Define valid entity types, relationship types, and constraints; enable schema validation and discovery.

**Core Functionality:**

```rust
pub struct SemanticLayerPlugin {
    object_types: BTreeMap<String, ObjectType>,
    link_types: BTreeMap<String, LinkType>,
    property_schemas: BTreeMap<String, PropertySchema>,
}

pub struct ObjectType {
    pub name: String,
    pub description: String,
    pub parent_types: Vec<String>, // Inheritance hierarchy
    pub properties: Vec<PropertySchema>,
    pub constraints: Vec<Constraint>,
}

pub struct LinkType {
    pub name: String,
    pub description: String,
    pub source_type: String,      // Required source entity type
    pub target_type: String,      // Required target entity type
    pub directionality: Directionality,
    pub multiplicity: Multiplicity,
}

pub enum Directionality {
    Unidirectional,
    Bidirectional,
}

pub enum Multiplicity {
    OneToOne,
    OneToMany,
    ManyToOne,
    ManyToMany,
}

pub struct PropertySchema {
    pub name: String,
    pub data_type: DataType,
    pub required: bool,
    pub constraints: Vec<Constraint>,
}

pub enum Constraint {
    Unique,
    NotNull,
    Pattern(String), // Regex
    Range { min: Option<f64>, max: Option<f64> },
    Enum(Vec<String>),
    Custom(String),
}
```

**Features:**

1. **Schema Definition Language**
   ```yaml
   # domain_schema.yaml
   objectTypes:
     Person:
       properties:
         - name: String (required)
         - email: String (unique)
         - role: Enum(Engineer, Manager, Product)

     Decision:
       properties:
         - title: String (required)
         - rationale: String
         - status: Enum(Proposed, Accepted, Rejected)
         - date: Date

     Service:
       properties:
         - name: String (required, unique)
         - owner: Person (required)

   linkTypes:
     AUTHORED:
       source: Person
       target: Decision
       directionality: Unidirectional
       multiplicity: OneToMany

     DEPENDS_ON:
       source: Service
       target: Service
       directionality: Unidirectional
       multiplicity: ManyToMany
   ```

2. **Validation Hooks**
   - Before creating node: Validate against object type schema
   - Before creating relationship: Validate types match link type constraints
   - On property update: Validate against property constraints
   - Block invalid mutations, provide clear error messages

3. **Schema Discovery API**
   ```rust
   // Query what types exist
   pub fn list_object_types(&self) -> Vec<&ObjectType>;
   pub fn list_link_types(&self) -> Vec<&LinkType>;

   // Query constraints for a type
   pub fn get_object_type(&self, name: &str) -> Option<&ObjectType>;
   pub fn get_link_type(&self, name: &str) -> Option<&LinkType>;

   // Check if a mutation is valid
   pub fn validate_create_node(&self, labels: &[String], props: &Map)
       -> Result<(), ValidationError>;
   pub fn validate_create_rel(&self, rel_type: &str, source_label: &str, target_label: &str)
       -> Result<(), ValidationError>;
   ```

4. **Cypher Extensions**
   ```cypher
   // Query schema
   CALL schema.types() YIELD objectType, properties

   CALL schema.linkTypes() YIELD sourceType, linkType, targetType

   // Type-safe creation (validated)
   CREATE (p:Person {name: 'Alice', role: 'Engineer'})
   // Fails if 'role' is not in Enum(Engineer, Manager, Product)
   ```

5. **Schema Evolution**
   - Add new object/link types without breaking queries
   - Make property optional (backward compatible)
   - Add constraints (validated on future mutations)
   - Deprecate types (mark as deprecated, allow reads but block new creations)

**Use Cases in CypherLite:**
- Agent memory: Define what entity types agents can create (Task, Decision, Service, etc.)
- Knowledge graphs: Enforce valid relationships (Decision affects Service, not Decision affects Person)
- Multi-domain graphs: Different plugins provide different semantic layers

---

### 3.3 Kinetic Layer Plugin

**Purpose:** Define operational procedures and actions; enable agent-safe mutations with authorization.

**Core Functionality:**

```rust
pub struct KineticLayerPlugin {
    actions: BTreeMap<String, ActionDefinition>,
    functions: BTreeMap<String, FunctionDefinition>,
    roles: BTreeMap<String, RoleDefinition>,
}

pub struct ActionDefinition {
    pub name: String,
    pub description: String,
    pub input_schema: PluginConfigSchema,
    pub output_schema: PluginConfigSchema,
    pub handler: Box<dyn Fn(&Value) -> Result<Vec<GraphMutation>, PluginError>>,
    pub required_role: Option<String>,
    pub preconditions: Vec<Precondition>,
    pub postconditions: Vec<Postcondition>,
}

pub enum Precondition {
    NodeExists { query: String },
    RelExists { query: String },
    PropertyValue { query: String, property: String, expected: Value },
}

pub enum Postcondition {
    NodeCreated { label: String },
    PropertyUpdated { label: String, property: String },
    RelCreated { rel_type: String },
}

pub struct RoleDefinition {
    pub name: String,
    pub description: String,
    pub allowed_actions: Vec<String>,
    pub allowed_read_types: Vec<String>,
}
```

**Features:**

1. **Action Definitions**
   ```yaml
   # actions.yaml
   actions:
     CreateDecision:
       description: "Create a new architectural decision"
       inputs:
         title: String (required)
         rationale: String (required)
         relatedService: String (required, must be ServiceId)
       handler: |
         CREATE (d:Decision {
           title: $title,
           rationale: $rationale,
           status: 'Proposed',
           date: NOW()
         })
         MATCH (s:Service {id: $relatedService})
         CREATE (d) -[:AFFECTS]-> (s)
       preconditions:
         - NodeExists: "MATCH (s:Service {id: $relatedService})"
       allowedRoles: [Engineer, Manager]

     UpdateDecisionStatus:
       inputs:
         decisionId: String (required)
         newStatus: Enum(Proposed, Accepted, Rejected)
       handler: |
         MATCH (d:Decision {id: $decisionId})
         SET d.status = $newStatus
       preconditions:
         - NodeExists: "MATCH (d:Decision {id: $decisionId})"
       allowedRoles: [Manager]

   functions:
     CalculateImpact:
       inputs:
         decisionId: String
       output: Number (impact_score 0-100)
       logic: |
         Count affected services, team members, etc.

     RecommendNextStep:
       inputs:
         decisionId: String
       output: String
       logic: |
         If status = Proposed, recommend "Request Review"
         If status = Accepted, recommend "Implement"
   ```

2. **Authorization & Access Control**
   - Agents are assigned roles
   - Actions declare required roles
   - Agent can only invoke authorized actions
   - Audit log: Who performed what action when

3. **Transaction Safety**
   - Action is atomic transaction
   - Preconditions checked at start
   - Mutations applied together
   - Postconditions validated before commit
   - Rollback if any precondition fails

4. **Cypher Extension**
   ```cypher
   // Call kinetic action
   CALL kinetic.createDecision({
     title: 'Migrate to PostgreSQL',
     rationale: 'Better scalability',
     relatedService: 'backend-api'
   }) YIELD decisionId, status
   RETURN decisionId

   // Query available actions for agent role
   CALL kinetic.availableActions($agentRole)
   YIELD actionName, description, requiredInputs

   // Execute business function
   CALL kinetic.calculateImpact(decisionId)
   YIELD impactScore
   ```

**Use Cases in CypherLite:**
- Agent memory: Define safe mutations agents can perform (create Task, update Task status, link Decision)
- Workflow automation: Define multi-step business processes
- Compliance: Enforce who can perform what actions, audit trail

---

### 3.4 Dynamic Layer Plugin

**Purpose:** Enable scenario analysis, what-if reasoning, and simulation without modifying real data.

**Core Functionality:**

```rust
pub struct DynamicLayerPlugin {
    scenarios: BTreeMap<ScenarioId, Scenario>,
    simulations: BTreeMap<SimulationId, SimulationState>,
}

pub struct Scenario {
    pub id: ScenarioId,
    pub name: String,
    pub description: String,
    pub base_graph_snapshot: GraphSnapshot,
    pub mutations: Vec<SpeculativeMutation>,
    pub created_at: SystemTime,
    pub created_by: String,
}

pub enum SpeculativeMutation {
    Assume { condition: String },           // Assume a node/rel exists
    Simulate { action: String, params: Map }, // Simulate an action result
    Modify { query: String, updates: Map },   // Modify node properties in scenario
}

pub struct SimulationState {
    pub scenario_id: ScenarioId,
    pub result_graph: GraphSnapshot,
    pub metrics: SimulationMetrics,
}

pub struct SimulationMetrics {
    pub affected_nodes: usize,
    pub affected_rels: usize,
    pub estimated_effort: f64,
    pub risk_score: f64,
    pub cost_impact: f64,
}
```

**Features:**

1. **Scenario Creation (Copy-on-Write)**
   ```cypher
   // Create a scenario starting from current graph state
   CALL dynamic.createScenario({
     name: 'Migrate to PostgreSQL',
     description: 'What if we switch from MySQL to PostgreSQL?'
   }) YIELD scenarioId

   // Within scenario context, apply speculative mutations
   CALL dynamic.inScenario(scenarioId, {
     MATCH (s:Service {name: 'backend-api'})
     SET s.database = 'PostgreSQL', s.migrationDate = '2026-04-01'
   })

   // Simulate downstream impacts
   CALL dynamic.simulateImpact(scenarioId)
   YIELD affectedServices, estimatedEffort, risks
   ```

2. **What-If Analysis**
   ```rust
   pub fn what_if(&self, scenario: &Scenario)
       -> Result<SimulationMetrics, PluginError> {
       // Simulate the scenario, compute metrics without changing real data
       // Return estimated effort, cost, risk, affected components
   }
   ```

3. **Scenario Comparison**
   ```cypher
   CALL dynamic.compareScenarios({
     scenario1: 'Scenario1',
     scenario2: 'Scenario2'
   }) YIELD differences, tradeoffs

   // Returns:
   // - What changes between scenarios
   // - Cost/effort comparison
   // - Risk profile comparison
   // - Recommendation
   ```

4. **Scenario Merge**
   ```cypher
   // Accept scenario mutations, merge back to main graph
   CALL dynamic.acceptScenario(scenarioId)
   YIELD appliedMutations, newState

   // This atomically applies all scenario mutations to real graph
   ```

5. **Temporal Simulation**
   ```cypher
   // "How will the system look after these actions?"
   CALL dynamic.simulateTimeline({
     scenario: scenarioId,
     duration: '3 months'
   }) YIELD projectedState, milestones
   ```

**Use Cases in CypherLite:**
- Agent memory: "What if I rename this service? What would break?"
- Planning: Explore architectural alternatives before committing
- Risk analysis: "What if this person leaves? What's the impact?"
- Forecasting: "What will the system look like in 6 months if we follow this plan?"

---

### 3.5 GraphRAG Plugin

**Purpose:** Implement the full Microsoft GraphRAG pipeline for knowledge graph extraction and hierarchical summarization.

**Core Functionality:**

```rust
pub struct GraphRAGPlugin {
    extractor: EntityRelationshipExtractor,
    community_detector: CommunityDetector,
    summarizer: LLMSummarizer,
}

pub struct EntityRelationshipExtractor {
    // LLM-driven extraction of entities and relationships from text
}

pub struct CommunityDetector {
    // Leiden algorithm for hierarchical community detection
}

pub struct LLMSummarizer {
    // Generate summaries for each community at each level
}

pub struct CommunityHierarchy {
    pub levels: Vec<CommunityLevel>,
}

pub struct CommunityLevel {
    pub communities: Vec<Community>,
    pub abstraction_level: u32,
}

pub struct Community {
    pub id: CommunityId,
    pub entities: Vec<NodeId>,
    pub summary: String,
    pub parent_community: Option<CommunityId>,
    pub relationships: Vec<String>,
}
```

**Features:**

1. **Entity & Relationship Extraction**
   ```rust
   pub fn extract_from_text(&mut self, text: &str)
       -> Result<ExtractionResult, PluginError> {
       // Use LLM to extract entities and relationships from unstructured text
       // Return nodes and edges to add to graph
   }
   ```

2. **Hierarchical Community Detection**
   ```rust
   pub fn detect_communities(&mut self, graph: &Graph)
       -> Result<CommunityHierarchy, PluginError> {
       // Apply Leiden algorithm recursively
       // Create hierarchy: leaf communities -> intermediate -> root
       // Each level represents different abstraction level
   }
   ```

3. **Generative Summarization**
   ```rust
   pub fn summarize_community(&self, community: &Community)
       -> Result<String, PluginError> {
       // Use LLM to generate natural language summary
       // Captures: entities, relationships, key themes
       // Used for global search: instead of traversing all nodes,
       //   query community summaries first
   }
   ```

4. **Cypher Extensions**
   ```cypher
   // Local search: Find entity and its immediate context
   CALL graphrag.localSearch({
     entity: 'Backend API',
     searchType: 'entity_context'
   }) YIELD context, entities, relationships

   // Global search: Find relevant communities using hierarchical structure
   CALL graphrag.globalSearch({
     query: 'What are the major system architecture patterns?',
     abstraction_level: 1  // Mid-level communities
   }) YIELD relevantCommunities, answers

   // Community-filtered graph traversal
   MATCH (c:Community {name: 'Backend Infrastructure'})
   WHERE graphrag.isInCommunity(c)
   MATCH (n:Service) -[:DEPENDS_ON]-> (m:Service)
   RETURN n, m

   // Extract communities for a subgraph
   MATCH (n:Entity)
   WHERE graphrag.extractedFromDocument($docId)
   CALL graphrag.detectCommunities()
   YIELD communityId, summary, entities
   ```

5. **Workflow Integration**
   ```rust
   pub fn ingest_documents(&mut self, docs: Vec<Document>)
       -> Result<IngestResult, PluginError> {
       // 1. Extract text units from documents
       // 2. Extract entities and relationships from each unit
       // 3. Add extracted nodes/edges to graph
       // 4. Detect communities (Leiden algorithm)
       // 5. Summarize each community
       // Return statistics: entities created, communities detected, etc.
   }
   ```

6. **Hybrid Search**
   ```cypher
   // Vector-guided global search
   MATCH (c:Community)
   WHERE vector.similarity(c.summaryEmbedding, $queryVector) > 0.75
   CALL graphrag.expandCommunity(c)
   YIELD entities, relationships
   RETURN entities, relationships
   ```

**Use Cases in CypherLite:**
- Document knowledge graphs: Extract structure from PDFs, Markdown, web pages
- Agent memory: Automatically extract decisions and relationships from past conversations
- RAG pipelines: Hierarchical search over large document corpora
- Research tools: Build concept maps from scientific literature

---

### 3.6 Serving Plugin (HTTP/gRPC API)

**Purpose:** Expose CypherLite graph via modern web APIs, enabling agent integration and tooling.

**Core Functionality:**

```rust
pub struct ServingPlugin {
    http_server: Option<HttpServer>,
    grpc_server: Option<GrpcServer>,
    auth_handler: AuthHandler,
}

pub struct AuthHandler {
    pub auth_type: AuthType,
    // JWT validation, OAuth2, API keys, etc.
}

pub enum AuthType {
    None,
    ApiKey(String),
    JWT(String),   // Public key for JWT validation
    OAuth2(OAuth2Config),
    Bearer,
}
```

**Features:**

1. **REST API**
   ```
   GET  /api/v1/nodes/:id
   POST /api/v1/nodes
   PUT  /api/v1/nodes/:id
   DELETE /api/v1/nodes/:id

   GET /api/v1/relationships/:id
   POST /api/v1/relationships
   PUT /api/v1/relationships/:id
   DELETE /api/v1/relationships/:id

   POST /api/v1/query (Cypher)
   GET  /api/v1/schema (List types, constraints)
   ```

2. **WebSocket Support**
   ```
   WS /api/v1/subscribe

   Client subscribes to changes on specific node/relationship types
   Server pushes mutations in real-time
   Useful for agents monitoring changes
   ```

3. **Streaming Query Results**
   ```
   POST /api/v1/query (with stream=true)

   Response: newline-delimited JSON
   Each line is a result row
   Allows agents to start processing before full result set
   ```

4. **Batch Operations**
   ```
   POST /api/v1/batch
   {
     "operations": [
       { "op": "create_node", "labels": ["Person"], "properties": {...} },
       { "op": "create_rel", "type": "KNOWS", "source": 1, "target": 2 },
       ...
     ]
   }

   Returns: Applied mutations with assigned IDs
   ```

5. **Authentication & Authorization**
   ```rust
   // Each API request validated against auth handler
   // API key: Check against configured keys
   // JWT: Validate signature, extract claims (user_id, roles)
   // OAuth2: Validate against authorization server

   // User roles/permissions used to filter returned data
   // Actions restricted by authorization level
   ```

6. **Monitoring & Metrics**
   ```
   GET /api/v1/metrics

   Returns:
   - Queries per second
   - Average query latency
   - Cache hit rate
   - Connected clients
   - Database size
   ```

7. **Documentation**
   ```
   GET /api/docs (OpenAPI specification)
   GET /api/ui (Swagger UI)

   Auto-generated from schema
   ```

**Configuration:**
```yaml
plugins:
  serving:
    enabled: true
    http:
      port: 8000
      bind: "127.0.0.1"
    grpc:
      port: 9000
      enabled: false
    auth:
      type: jwt
      public_key: "/path/to/public.pem"
    cors:
      allowed_origins: ["http://localhost:3000"]
```

**Use Cases in CypherLite:**
- Agent tooling: Expose graph API to LLM agents via MCP
- Web dashboard: Query graph from frontend
- Third-party integrations: Allow other services to read/write graph
- GraphQL wrapper: Built on top of REST API

---

## 4. Plugin Discovery & Loading

### 4.1 Static Linking (Compile-Time Features)

For performance-critical and widely-used plugins, CypherLite supports compile-time static linking:

```toml
# Cargo.toml
[features]
default = ["vector-index", "semantic-layer"]
vector-index = ["hnsw_lib"]
semantic-layer = []
graphrag = ["llm-client"]

[dependencies]
vector-index = { path = "plugins/vector-index", optional = true }
semantic-layer = { path = "plugins/semantic-layer", optional = true }
graphrag = { path = "plugins/graphrag", optional = true }
```

**Benefits:**
- Zero runtime overhead
- Compile-time optimization
- No FFI (Foreign Function Interface) complexity

**Tradeoffs:**
- Must be recompiled to change plugin set
- Larger binary size
- Suitable for core plugins only

### 4.2 Dynamic Loading (Runtime Plugins)

For extensibility without recompilation, CypherLite supports dynamic plugin loading:

```rust
pub struct PluginManager {
    plugins: BTreeMap<String, LoadedPlugin>,
    plugin_dir: PathBuf,
}

pub struct LoadedPlugin {
    handle: libloading::Library,
    instance: Box<dyn Plugin>,
    metadata: PluginMetadata,
}

impl PluginManager {
    pub fn discover_plugins(&self) -> Result<Vec<PluginMetadata>, PluginError> {
        // Scan plugin_dir for shared libraries (.so, .dll, .dylib)
        // Load metadata from each library
        // Validate compatibility
        // Return list of available plugins
    }

    pub fn load_plugin(&mut self, name: &str, config: PluginConfig)
        -> Result<(), PluginError> {
        // Find plugin file
        // dlopen() to load shared library
        // Call plugin's initialization function
        // Store in plugins map
    }

    pub fn unload_plugin(&mut self, name: &str)
        -> Result<(), PluginError> {
        // Call plugin shutdown
        // dlclose() to unload library
        // Remove from map
    }
}
```

**Plugin Library Convention:**

Each plugin shared library must export these symbols:

```rust
// In plugin crate
#[no_mangle]
pub extern "C" fn plugin_metadata() -> PluginMetadata {
    PluginMetadata {
        name: "my-plugin".to_string(),
        version: semver::Version::parse("1.0.0").unwrap(),
        // ...
    }
}

#[no_mangle]
pub extern "C" fn plugin_create() -> *mut dyn Plugin {
    Box::into_raw(Box::new(MyPlugin::new()))
}

#[no_mangle]
pub extern "C" fn plugin_destroy(plugin: *mut dyn Plugin) {
    unsafe {
        let _ = Box::from_raw(plugin);
    }
}
```

### 4.3 Plugin Registry & Version Compatibility

```rust
pub struct PluginRegistry {
    plugins: BTreeMap<String, PluginInfo>,
    dependency_graph: DependencyGraph,
}

pub struct PluginInfo {
    pub metadata: PluginMetadata,
    pub status: PluginStatus,
    pub load_time: SystemTime,
}

pub enum PluginStatus {
    Loaded,
    Failed(String), // Error reason
    Disabled,
}

pub struct DependencyGraph {
    edges: BTreeMap<String, Vec<String>>, // plugin -> dependencies
}

impl PluginRegistry {
    pub fn validate_compatibility(&self, metadata: &PluginMetadata)
        -> Result<(), CompatibilityError> {
        // Check core version compatibility
        // Check plugin dependencies are loaded
        // Check for version conflicts
    }

    pub fn resolve_load_order(&self, plugins: Vec<&str>)
        -> Result<Vec<String>, PluginError> {
        // Topological sort based on dependencies
        // Return load order ensuring dependencies load first
    }

    pub fn list_plugins(&self) -> Vec<&PluginInfo>;

    pub fn get_plugin_info(&self, name: &str)
        -> Option<&PluginInfo>;
}
```

**Startup Sequence:**

```
1. Load core configuration (cypherlite.yaml)
2. Discover available plugins in plugin_dir
3. Validate each plugin's compatibility
4. Resolve load order (topological sort by dependencies)
5. Load plugins in order:
   a. Call plugin_create()
   b. Call plugin.initialize(config)
   c. Register hooks with event dispatcher
   d. Register functions/procedures with query engine
6. Validate final state (all dependencies satisfied)
7. Ready for queries
```

---

## 5. Plugin Storage Allocation

### 5.1 Physical Layout in Single File

CypherLite's single-file architecture reserves dedicated page ranges for each plugin:

```
CypherLite Single File Layout:
┌─────────────────────────────────┐
│  File Header (4 KB)             │
│  ├─ Magic number                │
│  ├─ Core version                │
│  ├─ Metadata pages location     │
│  └─ Plugin registry offset      │
├─────────────────────────────────┤
│  Core Graph Pages (variable)    │
│  ├─ Node pages                  │
│  ├─ Relationship pages          │
│  ├─ Property pages              │
│  ├─ B-tree index pages          │
│  └─ Transaction log             │
├─────────────────────────────────┤
│  Plugin 1 Data Range            │
│  ├─ Plugin metadata pages       │
│  ├─ Index data (HNSW, etc.)    │
│  └─ Custom format data          │
├─────────────────────────────────┤
│  Plugin 2 Data Range            │
│  ├─ Plugin metadata pages       │
│  └─ ...                         │
├─────────────────────────────────┤
│  Plugin 3 Data Range            │
│  └─ ...                         │
├─────────────────────────────────┤
│  Free Space                     │
└─────────────────────────────────┘
```

### 5.2 Plugin Registry Page

Metadata about loaded plugins stored in dedicated registry page:

```rust
pub struct PluginRegistryPage {
    pub next_plugin_page_start: PageId,
    pub plugins: Vec<PluginEntry>,
}

pub struct PluginEntry {
    pub name: String,
    pub version: semver::Version,
    pub page_range: PageRange,
    pub metadata: PluginMetadata,
    pub status: PluginStatus,
    pub last_updated: SystemTime,
}
```

### 5.3 Dynamic Page Allocation

When a plugin initializes, it requests a page range:

```rust
pub trait StoragePlugin: Plugin {
    fn allocate_page_range(&mut self, count: u32)
        -> Result<PageRange, PluginError>;
}
```

**Allocation Strategy:**

1. Plugin calls `allocate_page_range(requested_pages)`
2. Core allocates contiguous pages after existing allocations
3. Pages added to plugin's registry entry
4. Plugin stores allocation in its own metadata pages
5. On shutdown: pages are released back to free pool
6. On upgrade: existing pages reused if possible

```rust
pub struct PageAllocator {
    next_available_page: PageId,
    allocations: BTreeMap<String, PageRange>, // plugin_name -> range
}

impl PageAllocator {
    pub fn allocate(&mut self, plugin_name: &str, count: u32)
        -> Result<PageRange, AllocationError> {
        let start = self.next_available_page;
        let end = start + count;
        self.allocations.insert(plugin_name.to_string(), PageRange { start, end });
        self.next_available_page = end;
        Ok(PageRange { start, end })
    }

    pub fn deallocate(&mut self, plugin_name: &str) {
        self.allocations.remove(plugin_name);
    }
}
```

### 5.4 Plugin Data Migration on Version Upgrade

When a plugin version changes, migration logic ensures data compatibility:

```rust
pub trait Plugin: Send + Sync {
    /// Migrate plugin data from old version to new version
    fn migrate(&mut self, from_version: &semver::Version)
        -> Result<(), PluginError>;
}

pub struct PluginUpgrade {
    pub plugin_name: String,
    pub from_version: semver::Version,
    pub to_version: semver::Version,
}

pub fn upgrade_plugin(&mut self, upgrade: PluginUpgrade)
    -> Result<(), PluginError> {
    // 1. Load old plugin version
    let old_plugin = self.load_plugin_version(&upgrade.plugin_name, &upgrade.from_version)?;

    // 2. Read old plugin data
    let old_data = old_plugin.read_all_pages()?;

    // 3. Load new plugin version
    let new_plugin = self.load_plugin_version(&upgrade.plugin_name, &upgrade.to_version)?;

    // 4. Call migration handler
    new_plugin.migrate(&upgrade.from_version)?;

    // 5. Rewrite plugin data
    new_plugin.write_all_pages(&old_data)?;

    // 6. Update registry
    self.update_plugin_registry(&upgrade.plugin_name, &upgrade.to_version)?;
}
```

**Example: Vector Index Migration**

```rust
// Old version: HNSW with M=8
// New version: HNSW with M=16, ef_construction=300

impl VectorIndexPlugin {
    fn migrate(&mut self, from_version: &semver::Version)
        -> Result<(), PluginError> {
        if from_version < "2.0.0" {
            // Rebuild HNSW index with new parameters
            self.rebuild_indices()?;
        }
        Ok(())
    }
}
```

---

## 6. Inter-Plugin Communication

### 6.1 Event Bus for Cross-Plugin Events

Plugins communicate through a shared event bus, enabling loose coupling:

```rust
pub trait EventBus: Send + Sync {
    /// Subscribe to events of a certain type
    fn subscribe(&self, event_type: EventType, handler: Box<dyn EventHandler>)
        -> SubscriptionId;

    /// Unsubscribe from events
    fn unsubscribe(&self, subscription_id: SubscriptionId);

    /// Publish an event (synchronous, handlers called immediately)
    fn publish(&self, event: &Event) -> Result<(), EventError>;

    /// Publish event asynchronously
    fn publish_async(&self, event: Event);
}

pub trait EventHandler: Send + Sync {
    fn handle(&self, event: &Event) -> Result<(), EventError>;
}

pub enum Event {
    GraphMutation(GraphEvent),
    PluginLoaded(PluginLoadedEvent),
    PluginUnloaded(PluginUnloadedEvent),
    IndexUpdated(IndexUpdateEvent),
    SchemaChanged(SchemaChangeEvent),
    Custom(String, serde_json::Value),
}

pub struct PluginLoadedEvent {
    pub plugin_name: String,
    pub capabilities: PluginCapabilities,
}

pub struct IndexUpdateEvent {
    pub index_id: IndexId,
    pub operation: IndexOperation,
}

pub enum IndexOperation {
    Created,
    Updated { nodes_updated: usize },
    Deleted,
}
```

**Example: GraphRAG Plugin Listening to Graph Mutations**

```rust
pub struct GraphRAGPlugin {
    event_bus: Arc<EventBus>,
    subscription_id: Option<SubscriptionId>,
}

impl Plugin for GraphRAGPlugin {
    fn initialize(&mut self, config: PluginConfig, registry: &PluginRegistry)
        -> Result<(), PluginError> {
        // Subscribe to graph mutations
        let handler = Box::new(self as &dyn EventHandler);
        self.subscription_id = Some(
            registry.event_bus().subscribe(EventType::GraphMutation, handler)
        );
        Ok(())
    }
}

impl EventHandler for GraphRAGPlugin {
    fn handle(&self, event: &Event) -> Result<(), EventError> {
        match event {
            Event::GraphMutation(graph_event) => {
                // When entities are created, extract communities
                // When relationships are created, update community relationships
                // Trigger async recomputation of summaries
                Ok(())
            }
            _ => Ok(())
        }
    }
}
```

### 6.2 Shared Services (Logging, Metrics, Configuration)

Plugins access shared services through a service registry:

```rust
pub trait ServiceRegistry: Send + Sync {
    fn get_logger(&self) -> Arc<dyn Logger>;
    fn get_metrics(&self) -> Arc<dyn Metrics>;
    fn get_config(&self) -> Arc<dyn ConfigService>;
    fn get_cache(&self) -> Arc<dyn Cache>;
}

pub trait Logger: Send + Sync {
    fn debug(&self, message: &str);
    fn info(&self, message: &str);
    fn warn(&self, message: &str);
    fn error(&self, message: &str);
}

pub trait Metrics: Send + Sync {
    fn increment_counter(&self, name: &str, value: u64);
    fn record_histogram(&self, name: &str, value: f64);
    fn gauge(&self, name: &str, value: f64);
}

pub trait ConfigService: Send + Sync {
    fn get(&self, key: &str) -> Option<serde_json::Value>;
    fn get_plugin_config(&self, plugin_name: &str) -> PluginConfig;
}

pub trait Cache: Send + Sync {
    fn get(&self, key: &str) -> Option<Vec<u8>>;
    fn set(&self, key: String, value: Vec<u8>, ttl: Duration);
    fn evict(&self, key: &str);
}
```

**Example: Vector Index Plugin Using Metrics**

```rust
impl VectorIndexPlugin {
    fn search(&self, index_id: IndexId, query: &IndexQuery)
        -> Result<Vec<(NodeId, f32)>, PluginError> {
        let start = std::time::Instant::now();
        let results = self.hnsw_index.search(query)?;
        let elapsed = start.elapsed().as_secs_f64();

        // Record metrics
        self.metrics.record_histogram("vector_index.search_latency_ms", elapsed * 1000.0);
        self.metrics.increment_counter("vector_index.searches", 1);
        self.logger.debug(&format!("Vector search returned {} results in {:.2}ms",
            results.len(), elapsed * 1000.0));

        Ok(results)
    }
}
```

### 6.3 Dependency Injection Pattern

Plugins receive dependencies through constructor injection:

```rust
pub fn create_plugin(
    event_bus: Arc<EventBus>,
    services: Arc<ServiceRegistry>,
    storage: Arc<dyn StoragePlugin>,
) -> Box<dyn Plugin> {
    Box::new(MyPlugin::new(event_bus, services, storage))
}

pub struct MyPlugin {
    event_bus: Arc<EventBus>,
    services: Arc<ServiceRegistry>,
    storage: Arc<dyn StoragePlugin>,
}

impl MyPlugin {
    fn new(
        event_bus: Arc<EventBus>,
        services: Arc<ServiceRegistry>,
        storage: Arc<dyn StoragePlugin>,
    ) -> Self {
        MyPlugin {
            event_bus,
            services,
            storage,
        }
    }
}
```

This approach ensures:
- **Loose Coupling:** Plugins don't directly depend on concrete implementations
- **Testability:** Can inject mock implementations for testing
- **Flexibility:** Core can provide different implementations based on configuration

---

## 7. Example Plugin Compositions

### 7.1 GraphRAG + Vector Index + Semantic Layer

**Use Case:** LLM Agent Memory with Full GraphRAG Pipeline

```yaml
# cypherlite.yaml
plugins:
  # Core plugins for agent memory
  semantic_layer:
    enabled: true
    schema_file: "domain_schema.yaml"

  vector_index:
    enabled: true
    embedding_dim: 768
    max_m: 16

  graphrag:
    enabled: true
    llm_provider: "openai"
    llm_model: "gpt-4"
    community_min_size: 3

  kinetic_layer:
    enabled: true
    actions_file: "actions.yaml"

  serving:
    enabled: true
    http:
      port: 8000
```

**Data Flow:**

```
Agent Session Start
  ↓
[Semantic Layer] Load schema (valid entity/link types)
  ↓
[Serving Plugin] GET /api/v1/schema → Agent validates types
  ↓
Agent creates decision via Kinetic Layer
  POST /api/v1/kinetic/createDecision
  ↓
[Event Bus] GraphMutation event published
  ↓
[GraphRAG Plugin] Receives mutation, triggers community recomputation
[Vector Index Plugin] Updates embeddings for affected nodes
  ↓
Agent queries for related decisions
  POST /api/v1/query (Cypher with vector.similarity)
  ↓
[Vector Index Plugin] KNN search → candidate decisions
[GraphRAG Plugin] Local search → expand to related services
[Semantic Layer] Filter by valid link types
  ↓
Results returned to agent
```

### 7.2 Scientific Knowledge Graph with Semantic Layer + GraphML Export

**Use Case:** Build and export knowledge graph from research papers

```yaml
plugins:
  semantic_layer:
    enabled: true
    schema_file: "scientific_schema.yaml"

  graphrag:
    enabled: true
    llm_provider: "openai"
    entity_extraction: true

  serializer_graphml:
    enabled: true
```

**Workflow:**

```
1. Ingest research papers
   → GraphRAG extracts entities (Concepts, Methods, Authors, Results)
   → Semantic Layer validates types
   → Communities detected hierarchically

2. Query and analyze
   MATCH (m:Method) -[:USED_IN]-> (e:Experiment)
   WHERE semantic.label(m) IN ['MachineLearning', 'NeuralNetwork']
   RETURN m, e

3. Export to GraphML for visualization
   CALL serializer.exportGraphML({
     output: "knowledge_graph.graphml"
   })
```

### 7.3 DevOps Graph with Dynamic Layer + Kinetic Layer

**Use Case:** Explore system architecture changes and operational procedures

```yaml
plugins:
  semantic_layer:
    enabled: true
    schema_file: "devops_schema.yaml"

  kinetic_layer:
    enabled: true
    actions_file: "devops_actions.yaml"

  dynamic_layer:
    enabled: true

  vector_index:
    enabled: true
```

**Scenario Analysis:**

```cypher
// Create scenario: "What if we migrate Service A to new infrastructure?"
CALL dynamic.createScenario({
  name: 'Migrate Service A',
  description: 'Explore impact of moving Service A to Kubernetes'
}) YIELD scenarioId

// Within scenario, modify infrastructure
CALL dynamic.inScenario(scenarioId, {
  MATCH (s:Service {name: 'ServiceA'})
  SET s.infrastructure = 'Kubernetes', s.deploymentDate = '2026-06-01'
  MATCH (s) -[:DEPENDS_ON]-> (d:Service)
  MATCH (d) -[:DEPLOYED_ON]-> (i:Infrastructure)
  CREATE (s) -[:DEPENDS_ON_INFRASTRUCTURE]-> (i)
})

// Simulate impact
CALL dynamic.simulateImpact(scenarioId)
YIELD affectedServices, estimatedEffort, risks

// Execute business logic within scenario
CALL dynamic.executeAction(scenarioId, 'planMigration')
YIELD timeline, risks, cost
```

---

## 8. Plugin Security Considerations

### 8.1 Sandboxing & Capabilities

```rust
pub enum PluginCapability {
    ReadGraph,           // Can query graph
    WriteGraph,          // Can mutate graph
    AccessStorage,       // Can access storage pages
    AccessNetwork,       // Can make HTTP requests
    ExecuteCode,         // Can execute LLM or external code
    AccessFileSystem,    // Can read/write files
}

pub struct PluginSandbox {
    allowed_capabilities: BTreeSet<PluginCapability>,
}

impl PluginManager {
    pub fn load_plugin_with_capabilities(
        &mut self,
        name: &str,
        capabilities: BTreeSet<PluginCapability>,
    ) -> Result<(), PluginError> {
        // Load plugin but restrict its capabilities
        // Intercept all operations, check against allowed set
    }
}
```

### 8.2 Resource Limits

```rust
pub struct PluginResourceLimits {
    pub max_memory_mb: u32,
    pub max_cpu_seconds: u32,
    pub max_storage_pages: u32,
    pub max_network_requests_per_minute: u32,
}

pub fn enforce_resource_limits(
    plugin_name: &str,
    limits: &PluginResourceLimits,
) -> Result<(), ResourceError> {
    // Monitor plugin resource usage
    // Terminate or throttle if exceeds limits
}
```

### 8.3 Audit Logging

All plugin operations logged for compliance:

```rust
pub struct PluginAuditLog {
    pub timestamp: SystemTime,
    pub plugin_name: String,
    pub operation: String,
    pub resource_accessed: String,
    pub result: Result<(), String>,
}
```

---

## 9. Plugin Development Guide

### 9.1 Template for Custom Plugin

```rust
// my_plugin/src/lib.rs
use cypherlite_plugin_api::*;

pub struct MyPlugin {
    config: PluginConfig,
    logger: Arc<dyn Logger>,
}

impl Plugin for MyPlugin {
    fn metadata(&self) -> PluginMetadata {
        PluginMetadata {
            name: "my-plugin".to_string(),
            version: semver::Version::parse("1.0.0").unwrap(),
            author: "Your Name".to_string(),
            description: "Does something useful".to_string(),
            dependencies: vec![],
            capabilities: PluginCapabilities::BusinessLogic,
            min_core_version: semver::Version::parse("1.0.0").unwrap(),
            max_core_version: None,
        }
    }

    fn initialize(&mut self, config: PluginConfig, registry: &PluginRegistry)
        -> Result<(), PluginError> {
        self.config = config;
        self.logger = registry.services().get_logger();
        self.logger.info("MyPlugin initialized");
        Ok(())
    }

    fn shutdown(&mut self) -> Result<(), PluginError> {
        self.logger.info("MyPlugin shutting down");
        Ok(())
    }
}

// Export plugin constructor
#[no_mangle]
pub extern "C" fn plugin_create() -> *mut dyn Plugin {
    Box::into_raw(Box::new(MyPlugin {
        config: Default::default(),
        logger: todo!(),
    }))
}

#[no_mangle]
pub extern "C" fn plugin_destroy(plugin: *mut dyn Plugin) {
    unsafe {
        let _ = Box::from_raw(plugin);
    }
}
```

### 9.2 Testing Plugins

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plugin_initialization() {
        let mut plugin = MyPlugin::new();
        let config = PluginConfig::default();
        let registry = MockPluginRegistry::new();

        assert!(plugin.initialize(config, &registry).is_ok());
    }
}
```

---

## 10. Conclusion

CypherLite's plugin architecture achieves:

1. **Extensibility:** Core stays small; all features are plugins
2. **Composability:** Plugins work together via event bus and shared services
3. **Compatibility:** Version management and migration support
4. **Performance:** Static linking for core plugins, dynamic loading for extensions
5. **Security:** Sandboxing and capability-based access control
6. **Developer Experience:** Clear traits, templates, and documentation

The six plugin types (Storage, Index, Query, Serializer, Event, Business Logic) plus planned modules (Vector Index, Semantic Layer, Kinetic Layer, Dynamic Layer, GraphRAG, Serving) position CypherLite to support diverse use cases from agent memory to knowledge graphs to RAG pipelines—all within a single-file embedded architecture.
