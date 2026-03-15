# SPEC-EXAMPLE-001 Research

## Codebase Analysis

### Current API Surface
- `CypherLite::open(config)` - Open/create database
- `execute(query)` / `execute_with_params(query, params)` - Query execution
- `begin()` / `commit()` / `rollback()` - Transactions
- Plugin registration: `register_scalar_function`, `register_index_plugin`, `register_serializer`
- `export_data(format, query)` / `import_data(format, bytes)` - Serializer plugin

### Supported Cypher Features
- Core: CREATE, MATCH, SET, DELETE, DETACH DELETE, MERGE (ON CREATE/ON MATCH SET)
- Control: WHERE, WITH (DISTINCT, WHERE, aggregation), UNWIND, ORDER BY, LIMIT
- Paths: Variable-length `[*N..M]` with cycle detection
- Temporal: AT TIME, BETWEEN TIME (feature: temporal-core)
- Temporal Edge: _valid_from/_valid_to (feature: temporal-edge)
- Subgraph: CREATE SNAPSHOT, MATCH (sg)-[:CONTAINS]->(n) (feature: subgraph)
- Hyperedge: CREATE HYPEREDGE FROM...TO, MATCH HYPEREDGE (feature: hypergraph)
- Index: CREATE INDEX, DROP INDEX, automatic IndexScan planning
- Plugins: ScalarFunction, IndexPlugin, Serializer, Trigger (feature: plugin)

### Existing Examples (2 Rust + 3 FFI)
- basic_crud.rs: CREATE, MATCH, SET, DELETE, MERGE
- knowledge_graph.rs: Multi-hop paths, aggregation, INDEX
- python_quickstart.py, go_quickstart.go, node_quickstart.js: Basic CRUD patterns

### Test Coverage: ~1,490 tests, 24 test files
- Comprehensive unit/integration tests exist for all features
- Missing: real-world scenario integration tests (use-case driven)

## Gap Analysis
- No examples exercise temporal features (AT TIME, BETWEEN TIME)
- No examples exercise subgraph snapshots (CREATE SNAPSHOT)
- No examples exercise hyperedges (CREATE HYPEREDGE)
- No examples exercise plugins (ScalarFunction, Trigger, Serializer)
- No examples exercise temporal edge (_valid_from/_valid_to)
- No real-world scenario examples (AI memory, ontology, GraphRAG, social network, etc.)
