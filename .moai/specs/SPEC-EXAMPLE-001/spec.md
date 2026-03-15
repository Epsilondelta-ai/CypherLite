# SPEC-EXAMPLE-001: Practical Examples & Integration Tests

| Field | Value |
|-------|-------|
| **SPEC ID** | SPEC-EXAMPLE-001 |
| **Title** | 10 Practical Examples with Integration Tests |
| **Created** | 2026-03-15 |
| **Status** | Planned |
| **Priority** | High |
| **Target Version** | v1.3.0 |

---

## 1. Environment

CypherLite v1.2.0 is feature-complete with 6 crates, 1,490 tests, and 85%+ coverage. However, the existing examples only cover basic CRUD and a simple knowledge graph. Advanced features (temporal, subgraph, hyperedge, plugin) have no user-facing examples.

### Goal
Create 10 practical, real-world examples that:
1. Serve as integration tests (all must compile and run via `cargo run --example`)
2. Demonstrate real-world use cases users can adapt
3. Exercise ALL CypherLite features progressively
4. Cover AI/LLM, enterprise, IoT, DevOps, and social domains

---

## 2. Requirements (EARS Format)

### R-EX-001 [Ubiquitous]
Each example file MUST reside in `crates/cypherlite-query/examples/` and compile/run via `cargo run -p cypherlite-query --example {name} --all-features`.

### R-EX-002 [Ubiquitous]
Each example MUST have a header comment block containing: purpose description, required feature flags, and execution command.

### R-EX-003 [Ubiquitous]
Each example MUST use `tempfile::tempdir()` for database storage and clean up automatically.

### R-EX-004 [Ubiquitous]
Each example MUST print section headers and query results to stdout for educational value.

### R-EX-005 [Event-Driven]
WHEN `cargo run -p cypherlite-query --example {name} --all-features` is executed THEN it MUST exit with code 0.

### R-EX-006 [Ubiquitous]
All examples MUST be in English (comments and output).

---

## 3. Example Specifications

### Example 1: AI Agent Conversation Memory
**File**: `agent_memory.rs`
**Category**: AI/LLM | **Difficulty**: Beginner | **Feature Flags**: (default)

**Scenario**: LLM agent stores conversation sessions with User, Session, Message nodes. Demonstrates:
- CREATE nodes (User, Session, Message) with timestamps
- CREATE relationships (:PARTICIPATED_IN, :CONTAINS, :FOLLOWS)
- MATCH with WHERE filtering (by user, by session)
- ORDER BY + LIMIT (recent N messages)
- WITH + count() aggregation (messages per session)

**Acceptance Criteria**:
- Creates 2 users, 3 sessions, 10+ messages
- Queries recent messages for a specific user
- Queries session summary with message counts

---

### Example 2: Simple Ontology (Taxonomy)
**File**: `simple_ontology.rs`
**Category**: Education/Knowledge | **Difficulty**: Beginner | **Feature Flags**: (default)

**Scenario**: Animal taxonomy (Kingdom > Phylum > Class > Order > Family > Genus > Species) modeled as a graph. Demonstrates:
- MERGE with ON CREATE SET (upsert taxonomy nodes)
- Variable-length paths (`-[:IS_A*1..5]->`) for ancestor/descendant queries
- OPTIONAL MATCH for optional properties
- "Is X a descendant of Y?" pattern

**Acceptance Criteria**:
- Creates taxonomy tree with 3+ levels
- Traverses hierarchy with variable-length paths
- Uses MERGE for idempotent node creation

---

### Example 3: GraphRAG Knowledge Graph
**File**: `graphrag.rs`
**Category**: AI/LLM | **Difficulty**: Intermediate | **Feature Flags**: (default)

**Scenario**: Document entities (Person, Organization, Concept) extracted for RAG pipeline. Demonstrates:
- CREATE INDEX for fast entity lookup
- Multi-hop traversal (`[*1..3]`) for relationship discovery
- WITH + count() for entity importance ranking
- Parameterized queries (simulate LLM-generated queries)

**Acceptance Criteria**:
- Creates 10+ entities with 15+ relationships
- Performs multi-hop "find related concepts" query
- Ranks entities by connection count
- Uses parameterized queries

---

### Example 4: Temporal Audit Trail
**File**: `temporal_audit.rs`
**Category**: Business/Enterprise | **Difficulty**: Intermediate | **Feature Flags**: `temporal-core`

**Scenario**: Employee salary/title change tracking with point-in-time queries. Demonstrates:
- Property updates with SET (salary changes over time)
- AT TIME queries (salary at specific past time)
- BETWEEN TIME queries (all changes in date range)
- Automatic _created_at/_updated_at timestamps

**Acceptance Criteria**:
- Creates employees, updates properties multiple times
- Queries past state with AT TIME
- Queries change history with BETWEEN TIME

---

### Example 5: Social Network Analysis
**File**: `social_network.rs`
**Category**: Social/Network | **Difficulty**: Intermediate | **Feature Flags**: (default)

**Scenario**: Social platform with User, Post nodes and follow/like relationships. Demonstrates:
- Friend-of-friend recommendation (2-hop path)
- UNWIND for tag expansion
- Aggregation + ORDER BY for influence ranking
- OPTIONAL MATCH for posts without likes

**Acceptance Criteria**:
- Creates 6+ users with follow/like relationships
- Generates friend-of-friend recommendations
- Ranks users by follower count
- Expands tag arrays with UNWIND

---

### Example 6: IoT Sensor Network with Temporal Edges
**File**: `iot_sensor.rs`
**Category**: IoT/Sensor | **Difficulty**: Intermediate | **Feature Flags**: `temporal-core`, `temporal-edge`

**Scenario**: Sensor-Gateway-Zone network where sensor connections change over time. Demonstrates:
- Temporal edges with _valid_from/_valid_to
- AT TIME query on edges (which gateway was sensor connected to at time T?)
- Sensor reading storage and time-range queries
- BETWEEN TIME for historical connection data

**Acceptance Criteria**:
- Creates sensors, gateways, zones with temporal connections
- Queries sensor-gateway mapping at different time points
- Demonstrates edge validity windows

---

### Example 7: DevOps Dependency Graph with Subgraph Snapshots
**File**: `devops_dependency.rs`
**Category**: DevOps/Infra | **Difficulty**: Advanced | **Feature Flags**: `subgraph`

**Scenario**: Microservice dependency graph with deployment snapshots. Demonstrates:
- CREATE SNAPSHOT for deployment state capture
- MATCH (sg)-[:CONTAINS]->(n) for snapshot member queries
- Variable-length paths for transitive dependency analysis
- Snapshot comparison (before/after deployment)

**Acceptance Criteria**:
- Creates service dependency graph (5+ services)
- Takes deployment snapshot
- Modifies graph (add/remove service)
- Compares snapshot members with current state

---

### Example 8: E-Commerce Recommendation Engine
**File**: `ecommerce_recommendation.rs`
**Category**: Business/Enterprise | **Difficulty**: Advanced | **Feature Flags**: `temporal-core`

**Scenario**: Collaborative filtering for product recommendations. Demonstrates:
- 2-hop pattern: Customer->Product<-Customer->Product (co-purchase)
- WITH + count() + ORDER BY + LIMIT for recommendation ranking
- CREATE INDEX for fast customer/product lookup
- Temporal query for "trending in last N days"
- MERGE for customer profile upsert

**Acceptance Criteria**:
- Creates customers, products, orders
- Generates "customers who bought X also bought Y" recommendations
- Excludes already-purchased items
- Uses temporal filtering for recency

---

### Example 9: Multi-Party Event Modeling with Hyperedges
**File**: `meeting_scheduler.rs`
**Category**: Business/Education | **Difficulty**: Advanced | **Feature Flags**: `hypergraph`

**Scenario**: Meeting events connecting multiple participants and rooms via hyperedges. Demonstrates:
- CREATE HYPEREDGE for N:M participant-room relationships
- MATCH HYPEREDGE for querying meetings
- Temporal ref (FROM (participant AT TIME T)) for attendance tracking
- "Find all meetings for participant X" query

**Acceptance Criteria**:
- Creates participants, rooms
- Creates meetings as hyperedges with multiple participants
- Queries meetings by participant
- Demonstrates temporal ref on hyperedge members

---

### Example 10: Full-Stack Agent Knowledge Base with Plugins
**File**: `agent_knowledge_base.rs`
**Category**: AI/LLM (Full-Stack) | **Difficulty**: Advanced | **Feature Flags**: `all-features`

**Scenario**: Comprehensive AI agent knowledge management using all CypherLite features. Demonstrates:
- ScalarFunction plugin (text normalization)
- Trigger plugin (audit log on CREATE/UPDATE)
- Serializer plugin (JSON export)
- Temporal versioning (knowledge evolution)
- Subgraph snapshots (topic grouping)
- Hyperedges (multi-source citations)
- All basic Cypher operations

**Acceptance Criteria**:
- Registers 3 plugins (scalar, trigger, serializer)
- Creates knowledge graph with topics and citations
- Uses temporal queries for knowledge versioning
- Takes topic snapshot
- Creates citation hyperedge
- Exports data via serializer
- All operations succeed with zero errors

---

## 4. Traceability

| Example | Requirements | Feature Flags | Verification |
|---------|-------------|---------------|-------------|
| 1. agent_memory | R-EX-001~006 | (default) | `cargo run --example agent_memory` |
| 2. simple_ontology | R-EX-001~006 | (default) | `cargo run --example simple_ontology` |
| 3. graphrag | R-EX-001~006 | (default) | `cargo run --example graphrag` |
| 4. temporal_audit | R-EX-001~006 | temporal-core | `cargo run --example temporal_audit` |
| 5. social_network | R-EX-001~006 | (default) | `cargo run --example social_network` |
| 6. iot_sensor | R-EX-001~006 | temporal-edge | `cargo run --example iot_sensor` |
| 7. devops_dependency | R-EX-001~006 | subgraph | `cargo run --example devops_dependency` |
| 8. ecommerce_recommendation | R-EX-001~006 | temporal-core | `cargo run --example ecommerce_recommendation` |
| 9. meeting_scheduler | R-EX-001~006 | hypergraph | `cargo run --example meeting_scheduler` |
| 10. agent_knowledge_base | R-EX-001~006 | all-features | `cargo run --example agent_knowledge_base` |

## 5. Definition of Done

- [ ] 10 example files in `crates/cypherlite-query/examples/`
- [ ] All 10 pass `cargo run -p cypherlite-query --example {name} --all-features` with exit 0
- [ ] `cargo test --workspace --all-features` still passes (no regression)
- [ ] README.md examples section updated with all 10 examples
- [ ] CHANGELOG.md updated with v1.3.0 entry
