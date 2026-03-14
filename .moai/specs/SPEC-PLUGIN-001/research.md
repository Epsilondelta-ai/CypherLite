---
id: SPEC-PLUGIN-001
type: research
created: "2026-03-13"
author: epsilondelta
---

# SPEC-PLUGIN-001 Research: Plugin System

## 1. Architecture Overview

### 1.1 Crate Structure
- **cypherlite-core** (v0.9.0): Types, traits, config, error definitions
  - Modules: config, error, traits, types
  - Key traits: `TransactionView`, `LabelRegistry`
  - Key types: `NodeId`, `EdgeId`, `PropertyValue`, `DatabaseConfig`, `CypherLiteError`

- **cypherlite-storage** (v0.9.0): Storage engine, indexes, transactions, WAL
  - Modules: btree, catalog, page, transaction, wal, index, version, subgraph, hyperedge
  - Key struct: `StorageEngine` (page_manager, buffer_pool, wal_writer, wal_reader, tx_manager)
  - Key struct: `IndexManager` (HashMap<String, (IndexDefinition, PropertyIndex)>)

- **cypherlite-query** (v0.9.0): Lexer, parser, planner, semantic analysis, executor, API
  - Modules: api, executor, lexer, parser, planner, semantic
  - Key struct: `CypherLite` (public API facade)
  - Key struct: `QueryResult`, `Row`, `Value`

### 1.2 Feature Flag Chain
```
temporal-core -> temporal-edge -> subgraph -> hypergraph -> full-temporal
```
All 3 crates share this chain. Plugin system should add a `plugin` feature flag.

## 2. Existing Extension Points (Traits)

### 2.1 TransactionView (cypherlite-core/src/traits.rs)
```rust
pub trait TransactionView {
    fn snapshot_frame(&self) -> u64;
}
```
- Object-safe: Yes (`Box<dyn TransactionView>` used in tests)
- Callers: executor, planner
- Plugin relevance: LOW (internal transaction mechanism)

### 2.2 LabelRegistry (cypherlite-core/src/traits.rs)
```rust
pub trait LabelRegistry {
    fn get_or_create_label(&mut self, name: &str) -> u32;
    fn label_id(&self, name: &str) -> Option<u32>;
    fn label_name(&self, id: u32) -> Option<&str>;
    fn get_or_create_rel_type(&mut self, name: &str) -> u32;
    fn rel_type_id(&self, name: &str) -> Option<u32>;
    fn rel_type_name(&self, id: u32) -> Option<&str>;
    fn get_or_create_prop_key(&mut self, name: &str) -> u32;
    fn prop_key_id(&self, name: &str) -> Option<u32>;
    fn prop_key_name(&self, id: u32) -> Option<&str>;
}
```
- Object-safe: Yes (`&mut dyn LabelRegistry` used in planner, semantic)
- Callers: Planner::new(), SemanticAnalyzer::new()
- Plugin relevance: MODERATE (custom registries possible but low demand)

### 2.3 FromValue (cypherlite-query/src/api/mod.rs)
```rust
pub trait FromValue: Sized {
    fn from_value(value: &Value) -> Option<Self>;
}
```
- Implementations: i64, f64, String, bool, Vec<u8>
- Plugin relevance: HIGH (users can implement for custom types)

## 3. Plugin Type Feasibility Analysis

### 3.1 Index Plugin (BEST CANDIDATE)
- **Current state**: `IndexManager` uses `HashMap<String, (IndexDefinition, PropertyIndex)>` registry pattern
- **Extension approach**: Define `IndexPlugin` trait, allow custom index implementations (e.g., full-text, spatial, vector)
- **Coupling**: LOW - IndexManager already has a clean registry interface
- **Impact**: New trait in core, IndexManager modification in storage
- **Reference**: `index/mod.rs:170` IndexManager pattern

### 3.2 Query Function Plugin (GOOD)
- **Current state**: `eval.rs:63` uses `match func_name.as_str()` for function dispatch
  - Built-in functions: id, type, labels, properties, keys, size, toString, toInteger, toFloat, toBoolean, coalesce, head, last, tail, range, reverse, nodes, relationships, length, startNode, endNode, abs, ceil, floor, round, sign, sqrt, timestamp, date, datetime, duration, left, right, trim, ltrim, rtrim, replace, substring, toLower, toUpper, split, contains, startsWith, endsWith, exists, randomUUID
- **Extension approach**: Define `ScalarFunction` trait, register custom functions
- **Coupling**: MODERATE - requires refactoring the match dispatch to a registry lookup
- **Impact**: New trait in core/query, executor eval.rs refactoring

### 3.3 Serializer Plugin (GOOD)
- **Current state**: Uses bincode for internal serialization (serde-based)
- **Extension approach**: Define `Serializer` trait for custom import/export formats (JSON, CSV, GraphML)
- **Coupling**: LOW - serde already provides abstraction layer
- **Impact**: New trait in core, new module in query for format handling

### 3.4 Storage Backend Plugin (MODERATE)
- **Current state**: `StorageEngine` struct has tight coupling (page_manager, buffer_pool, wal_writer, wal_reader, tx_manager)
- **Extension approach**: Define `StorageBackend` trait abstracting I/O operations
- **Coupling**: HIGH - StorageEngine internals are deeply interconnected
- **Impact**: Major refactoring of storage crate, high risk
- **Recommendation**: DEFER to v2.0, too invasive for current architecture

### 3.5 Business Logic / Trigger Plugin (MODERATE)
- **Current state**: No hook infrastructure for pre/post mutation events
- **Extension approach**: Define `Trigger` trait with on_create/on_update/on_delete hooks
- **Coupling**: MODERATE - requires adding hook points in executor write operations
- **Impact**: executor modification, new trait in core

### 3.6 Event / Lifecycle Plugin (DIFFICULT)
- **Current state**: No event system, no observer pattern
- **Extension approach**: Full event bus with publish/subscribe
- **Coupling**: HIGH - requires threading event system through all layers
- **Impact**: Cross-cutting concern affecting all 3 crates
- **Recommendation**: DEFER to v2.0

## 4. Recommended Plugin Scope for v1.0

Based on feasibility and value analysis:

| Priority | Plugin Type | Feasibility | User Value | Risk |
|----------|------------|-------------|------------|------|
| P1 | Query Function | GOOD | HIGH | Low |
| P2 | Index | BEST | HIGH | Low |
| P3 | Serializer (Import/Export) | GOOD | HIGH | Low |
| P4 | Trigger (Business Logic) | MODERATE | MEDIUM | Medium |
| DEFER | Storage Backend | LOW | LOW | High |
| DEFER | Event/Lifecycle | DIFFICULT | MEDIUM | High |

### Rationale
- **Query Function first**: Most commonly requested extension point in graph databases. The match dispatch refactoring is contained in a single file (eval.rs).
- **Index second**: IndexManager already has a registry pattern. Full-text and vector search are high-demand features.
- **Serializer third**: Data import/export is essential for real-world usage. serde provides a clean foundation.
- **Trigger fourth**: Enables validation, auditing, and business rules but requires executor hooks.

## 5. Dynamic Dispatch Patterns in Codebase

Already established patterns:
- `Box<dyn TransactionView>` - object-safe trait objects
- `&mut dyn LabelRegistry` - trait references in planner/semantic
- `HashMap<String, (IndexDefinition, PropertyIndex)>` - name-based registry

These patterns provide a foundation for plugin registry design:
```rust
// Proposed pattern
pub trait Plugin: Send + Sync {
    fn name(&self) -> &str;
    fn version(&self) -> &str;
}

pub struct PluginRegistry<T: Plugin> {
    plugins: HashMap<String, Box<T>>,
}
```

## 6. Feature Flag Design

```toml
[features]
plugin = []  # Base plugin infrastructure (no dependency chain requirement)
plugin-full = ["plugin", "full-temporal"]  # All plugins + all features
```

Plugin feature should be independent of the temporal feature chain since plugins are an orthogonal concern.

## 7. Dependencies Consideration

Current dependencies that support plugin design:
- `serde` (already used) - for plugin config serialization
- `parking_lot` (already used) - for thread-safe plugin registry
- `dashmap` (already used) - for concurrent plugin access

No new external dependencies needed for basic plugin infrastructure.

## 8. Constraints

- C1: CypherLite is synchronous (no async/await) - plugins must be synchronous
- C2: Single-file database - plugins cannot modify the file format without header version bump
- C3: MSRV 1.84 - no features beyond Rust 1.84
- C4: Object safety required for trait-based plugins (no `Self: Sized` constraints)
- C5: Thread safety required (`Send + Sync`) since StorageEngine uses parking_lot/dashmap
