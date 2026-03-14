# SPEC-PERF-001 Performance Optimization Research

## Executive Summary

CypherLite v1.0.0 (1,309 tests) targets 6 key performance metrics. Current benchmark coverage is partial — missing concurrency stress, memory profiling, and streaming benchmarks. Several optimization opportunities exist in hot paths without major architectural changes.

---

## 1. Current Benchmark Coverage

### Existing Benchmarks

**Storage** (`crates/cypherlite-storage/benches/storage_bench.rs`, 194 lines):
- 8 criterion tests: node_write_100, node_read_1000, wal_write_commit_10, checkpoint_10_frames, node_read_uncached_500, crash_recovery_1000_frames, edge_traversal_100n_500e

**Query** (`crates/cypherlite-query/benches/query_bench.rs`, 362 lines):
- 15 criterion tests: lexer (2), parser (2), execution (5), var-length paths (2), temporal (3), merge (1)

**Feature-Specific**:
- `hypergraph.rs` (150 lines): 5 benchmarks
- `subgraph.rs` (183 lines): 5 benchmarks
- `temporal_edge.rs` (110 lines): 2 benchmarks
- `inline_filter.rs` (82 lines): 3 benchmarks

### Critical Gaps

1. **Concurrency stress**: No multi-threaded read/write benchmarks
2. **Memory profiling**: No RSS/heap measurements at 1M node scale
3. **Streaming queries**: No iterator-based result benchmarks
4. **Batch operations**: No bulk insert/delete benchmarks
5. **Serialization overhead**: No bincode cost measurement on large property arrays
6. **Lock contention**: No write-lock acquisition time under concurrent read load

---

## 2. Storage Engine Optimization Opportunities

### 2.1 Buffer Pool LRU: O(n) Touch Cost (CRITICAL)

**Location**: `buffer_pool.rs:195-196`
```rust
self.lru_order.retain(|p| *p != page_id);  // O(n) scan!
self.lru_order.push_back(page_id);
```
- Called on every `get()`, `get_mut()`, `insert()`
- 256-page cache = O(256) per cache hit
- **Fix**: Doubly-linked-list LRU or epoch-based recency tracking

### 2.2 FSM Page Allocation: Linear Scan

**Location**: `page_manager.rs:106-144`
- Linear scan of FSM bytes for free bit per allocation
- No free-space hints, starts from byte 0 each time
- **Fix**: In-memory FSM bitmap + next_free_page hint

### 2.3 BTreeMap Clone on Serialization

**Location**: `btree/mod.rs` - In-memory BTreeMap serialized to pages
- Modifying one property requires full map serialization
- **Fix**: Selective field updates, delta serialization

### 2.4 WAL Write Path

**Location**: `wal/writer.rs:91-118`
- Synchronous fsync on SyncMode::Full
- No compression, no batching heuristic
- **Fix**: Optional group commit, LZ4 compression for WAL frames

### 2.5 Unused Dependencies

- `crossbeam = "0.8"`: ~200KB compiled, NO usage found in codebase
- `dashmap = "6"`: ~80KB compiled, NO usage found in codebase
- **Fix**: Remove from Cargo.toml (-280KB binary size)

---

## 3. Query Engine Optimization Opportunities

### 3.1 Expression Evaluator: String Allocations in Hot Loop

**Location**: `eval.rs:24`
```rust
let temporal_key = format!("__temporal_props__{}", var_name);
```
- Allocates String on every property access in temporal queries
- **Fix**: Pre-allocate or use Cow<str>

### 3.2 No Short-Circuit for AND/OR

**Location**: `eval.rs:33-36`
- Always evaluates both sides of AND/OR
- **Fix**: Early exit on first false (AND) or first true (OR)

### 3.3 Record Cloning in Expand Operator

**Location**: `expand.rs:57,82,88`
```rust
let mut new_record = record.clone();  // Full HashMap clone per edge
```
- 100 nodes x 5 edges = 500 full HashMap clones
- **Fix**: Cow<Record> or Arc<Record> with copy-on-write

### 3.4 VarLengthExpand: Exponential Memory

**Location**: `var_length_expand.rs` DFS algorithm
- All results collected in Vec<Record>
- max_hops=10 in dense graph = 1M+ records in memory
- **Fix**: Streaming iterator model

### 3.5 Optimizer: No Cost Model

**Location**: `optimize.rs` (1499 lines)
- No row count statistics per operator
- No cost comparison for plan alternatives
- **Fix**: Implement basic statistics collection and cost model

---

## 4. Memory & Allocation Patterns

### Clone Frequency
- 176+ `.clone()` sites across codebase (grep count)
- Hot spots: eval.rs (8 sites), expand.rs (3 sites), var_length_expand.rs (multiple)

### Value Enum Size
```rust
pub enum Value { Node(NodeId), Edge(EdgeId), String(String), List(Vec<Value>), ... }
```
- Each variant adds 8-byte discriminant
- Largest variant determines enum size
- **Fix**: Box large variants, or use `#[repr(u32)]` discriminant

### Serialization
- `bincode::serialize()` allocates Vec for every property read
- No zero-copy deserialization
- **Fix**: Cow<[u8]> or reference-based formats

---

## 5. Concurrency Patterns

- Single writer via `Arc<Mutex<()>>` (parking_lot) - by design
- Snapshot reads via `AtomicU64` frame number - O(1), no contention
- MVCC uses unsafe transmute to extend MutexGuard lifetime (documented @MX:WARN)
- Missing: lock contention metrics, concurrent read throughput measurement

---

## 6. Recommended Priority Order

### Tier 1: Quick Wins (High Impact, Low Effort)

| # | Optimization | Location | Impact | Effort |
|---|-------------|----------|--------|--------|
| 1 | Fix LRU O(n) touch | buffer_pool.rs:195 | Cache hit -50% latency | 2-4h |
| 2 | Remove unused deps | Cargo.toml | Binary -280KB | 1h |
| 3 | Add concurrency benchmarks | new bench file | Measure contention | 2-3h |

### Tier 2: Medium Effort Wins

| # | Optimization | Location | Impact | Effort |
|---|-------------|----------|--------|--------|
| 4 | eval() string allocation | eval.rs:24 | Temporal queries -10% | 3-4h |
| 5 | AND/OR short-circuit | eval.rs:33-36 | WHERE clauses -20% | 4-5h |
| 6 | Record Cow/Arc | expand.rs | Large fanout -30% memory | 6-8h |

### Tier 3: Architectural Changes

| # | Optimization | Location | Impact | Effort |
|---|-------------|----------|--------|--------|
| 7 | Streaming query results | all operators | Memory -90% for large results | 20-30h |
| 8 | Cost-based optimizer | optimize.rs | Complex queries +50% | 15-20h |
| 9 | FSM allocation hints | page_manager.rs | Write throughput +20% | 8-10h |

---

## 7. v1.0 Performance Target Feasibility

| Target | Feasibility | Key Optimizations |
|--------|------------|-------------------|
| Simple match (p99) < 10ms | HIGH | LRU fix, eval optimization |
| 2-hop pattern (p99) < 50ms | HIGH | Expand record sharing, short-circuit |
| Binary size < 50MB | HIGH | Remove unused deps, feature-gate |
| Memory (1M nodes) < 500MB | MEDIUM | Streaming results, Value enum sizing |
| Sequential write > 1,000/s | HIGH | FSM hints, batch allocations |
| Concurrent read (4T) > 50k/s | HIGH | Already lock-free reads via MVCC |
