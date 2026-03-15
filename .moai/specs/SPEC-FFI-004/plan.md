# SPEC-FFI-004: Implementation Plan

| Field     | Value                                        |
| --------- | -------------------------------------------- |
| SPEC      | SPEC-FFI-004                                 |
| Title     | Node.js Bindings for CypherLite via napi-rs  |
| Mode      | TDD (RED-GREEN-REFACTOR)                     |
| Crate     | cypherlite-node                              |

---

## Milestones

### M1: Crate Setup and Build Infrastructure (Primary Goal)

**Objective**: Establish `cypherlite-node` crate with napi-rs + `@napi-rs/cli` build pipeline.

**Tasks**:
1. Create `crates/cypherlite-node/Cargo.toml`
   - `[lib]` with `crate-type = ["cdylib"]`, `name = "cypherlite_node"`
   - `napi` dependency with `napi6` feature
   - `napi-derive` dependency
   - Feature flag re-export from `cypherlite-query`
2. Create `crates/cypherlite-node/package.json`
   - `name`: `cypherlite`
   - `napi` configuration with `name: "cypherlite"` and platform triples
   - `devDependencies`: `@napi-rs/cli`, `vitest`, `typescript`
   - `scripts`: `build`, `test`, `prepublishOnly`
3. Create `crates/cypherlite-node/src/lib.rs`
   - `#[napi]` annotated module functions
   - `version()` and `features()` module-level functions
4. Create `crates/cypherlite-node/index.js`
   - napi-rs platform detection and binary loading boilerplate
5. Create `crates/cypherlite-node/tsconfig.json`
   - TypeScript configuration for test files
6. Add `crates/cypherlite-node` to workspace `Cargo.toml` members
7. Verify `napi build --release` produces `.node` addon and `index.d.ts`
8. Verify `const cypherlite = require('./index')` loads successfully

**Requirements**: REQ-FFI-NODE-001 ~ REQ-FFI-NODE-007, REQ-FFI-NODE-090 ~ REQ-FFI-NODE-091

**Acceptance**: `napi build` succeeds, `cypherlite.version()` returns `"1.1.0"`, `cypherlite.features()` returns feature string.

---

### M2: Database Lifecycle (Primary Goal)

**Objective**: Implement `Database` napi-rs class with open/close lifecycle.

**Tasks**:
1. Create `src/database.rs`
   - `#[napi] struct Database` wrapping `Mutex<Option<CypherLite>>`
   - `close(&self)` method: takes Mutex, drops inner `CypherLite`, sets to `None`
   - Closed-state check on all methods (throws `CypherLiteError`)
2. Create module-level `open(path, config?)` function
   - Accept `path: String` and optional `DatabaseConfig` napi object
   - Construct `DatabaseConfig` from JavaScript options
   - Call `CypherLite::open(config)` and wrap result in `Database`
3. Implement thread safety: `Mutex<Option<CypherLite>>` for Rust-side exclusivity
4. RED: Write vitest tests for open/close lifecycle
5. GREEN: Make tests pass
6. REFACTOR: Clean up error conversion

**Requirements**: REQ-FFI-NODE-010 ~ REQ-FFI-NODE-014, REQ-FFI-NODE-080

**Acceptance**: Database open/close works, closed state detected and throws error, double-close is no-op.

---

### M3: Query Execution and Error Handling (Primary Goal)

**Objective**: Implement `execute()` and `execute(query, params)` with error mapping.

**Tasks**:
1. Create `src/error.rs`
   - Define `CypherLiteError` JavaScript error class via napi-rs
   - Implement `From<cypherlite_core::CypherLiteError>` for napi `Error` conversion
   - Set `code` property on Error for error categorization
2. Create `src/value.rs`
   - `rust_value_to_js(env, &Value) -> napi::Result<JsUnknown>` conversion function
   - `js_to_rust_value(val: JsUnknown) -> napi::Result<Value>` conversion function
   - Handle all base type conversions (null, boolean, number, string, Buffer, Array, Date, BigInt)
3. Implement `Database.execute(query: String) -> Result` in `src/database.rs`
   - Acquire Mutex, execute query
   - Wrap `QueryResult` into JavaScript `Result` object
4. Implement `Database.execute(query: String, params: Option<Object>) -> Result`
   - Convert JavaScript object values to Rust `HashMap<String, Value>`
   - Call `execute_with_params()`
5. RED: Write vitest tests for queries, parameterized queries, error cases
6. GREEN: Make tests pass
7. REFACTOR: Optimize value conversion

**Requirements**: REQ-FFI-NODE-020 ~ REQ-FFI-NODE-022, REQ-FFI-NODE-070 ~ REQ-FFI-NODE-073

**Acceptance**: Queries execute and return results, params convert correctly, errors throw correct CypherLiteError with code.

---

### M4: Transaction Support (Secondary Goal)

**Objective**: Implement `Transaction` class with commit/rollback.

**Tasks**:
1. Create `src/transaction.rs`
   - Design for Rust lifetime management: `Transaction<'_>` cannot be stored in napi-rs class directly
   - Strategy: Store `Arc<Mutex<Option<CypherLite>>>` in Transaction, use explicit begin/commit/rollback at Rust level
   - `#[napi] struct Transaction` wrapping transaction state
   - `execute(query)` and `execute(query, params)` methods
   - `commit()` and `rollback()` methods (invalidate handle)
   - Closed-state check on all methods
2. Implement `Database.begin() -> Transaction`
   - Return `Transaction` object bound to this database
3. RED: Write vitest tests for transaction lifecycle
4. GREEN: Make tests pass
5. REFACTOR: Ensure clean error propagation

**Requirements**: REQ-FFI-NODE-030 ~ REQ-FFI-NODE-035

**Acceptance**: Transactions commit/rollback correctly, double-use throws error.

**Technical Note**: Rust `Transaction<'_>` has a lifetime tied to `&mut CypherLite`. napi-rs structs require `'static` lifetime. The implementation must use the same strategy as SPEC-FFI-003 (Python):
- Option A (Recommended): Hold `Arc<Mutex<CypherLite>>` in Transaction and use explicit transaction state management at the storage engine level.
- Option B: Use `unsafe` to erase the lifetime (not recommended).

---

### M5: Result and Row Access (Secondary Goal)

**Objective**: Implement `Result` class with iteration and `Row` as plain JavaScript objects.

**Tasks**:
1. Create `src/result.rs`
   - `#[napi] struct CypherLiteResult` storing columns (`Vec<String>`) and rows (pre-converted JS data)
   - Strategy: Eagerly convert all `QueryResult` data to JavaScript values at construction time
   - `columns` property -> `string[]`
   - `length` property -> row count
   - `get(index: number)` -> `Row` object (throws `RangeError` for out of bounds)
   - `toArray()` -> `Row[]` array
   - `[Symbol.iterator]` implementation via napi-rs custom iterator
   - `toString()` -> `[CypherLiteResult columns=[...] rows=N]`
2. Implement Row construction
   - Row is a plain JavaScript object with:
     - String keys for column names
     - Numeric keys for positional access
     - `length` property (non-enumerable)
   - `toObject()` method returns plain object with column-name keys only
   - Row construction uses `napi::Env::create_object()` with named + indexed properties
3. RED: Write vitest tests for all access patterns
4. GREEN: Make tests pass
5. REFACTOR: Optimize iteration, verify property enumeration

**Requirements**: REQ-FFI-NODE-040 ~ REQ-FFI-NODE-054

**Acceptance**: Full iteration, indexing, property access, toArray, toString all work correctly.

---

### M6: Value Type Mapping (Secondary Goal)

**Objective**: Complete bidirectional Value conversion including feature-gated types.

**Tasks**:
1. Implement ID types as BigInt in `src/value.rs`
   - `Value::Node(id)` -> `BigInt(id)`
   - `Value::Edge(id)` -> `BigInt(id)`
2. Implement `Date` conversion
   - `Value::DateTime(ms)` -> `new Date(ms)`
   - `Date` -> `Value::DateTime(date.getTime())`
3. Implement `Buffer` conversion
   - `Value::Bytes(b)` -> `Buffer.from(b)`
   - `Buffer` -> `Value::Bytes(vec)`
4. Implement `BigInt` parameter conversion
   - `BigInt` -> `Value::Int64(n)` (with range check, throw `RangeError` if > i64::MAX)
5. Implement feature-gated types (`#[cfg(feature = "subgraph")]`):
   - `Value::Subgraph(id)` -> `BigInt(id)`
6. Implement feature-gated types (`#[cfg(feature = "hypergraph")]`):
   - `Value::Hyperedge(id)` -> `BigInt(id)`
   - `Value::TemporalNode(id, ts)` -> `{ nodeId: BigInt(id), timestamp: BigInt(ts) }`
7. Implement integer/float detection for `number` -> Rust conversion
   - `Number.isInteger(n)` -> `Value::Int64`
   - Otherwise -> `Value::Float64`
8. RED: Write vitest roundtrip tests for all value types
9. GREEN: Make tests pass
10. REFACTOR: DRY conversion code

**Requirements**: REQ-FFI-NODE-060 ~ REQ-FFI-NODE-064

**Acceptance**: All value types round-trip correctly, feature-gated types compile conditionally, unsupported types throw TypeError.

---

### M7: TypeScript Definitions, Tests, Documentation (Final Goal)

**Objective**: Complete TypeScript definitions, comprehensive vitest suite, and documentation.

**Tasks**:
1. Verify auto-generated `index.d.ts` completeness
   - Type annotations for all classes: `Database`, `Transaction`, `CypherLiteResult`
   - Type annotations for module functions: `open()`, `version()`, `features()`
   - Type annotations for `CypherLiteError` class with `code` property
   - `DatabaseConfig` interface
   - `Row` type definition
   - Overloaded `execute` signatures for optional params
2. Manually supplement `index.d.ts` if napi-rs auto-generation misses complex types
3. Validate types with `tsc --noEmit` against test files
4. Write comprehensive vitest suite:
   - `__test__/database.spec.ts`: lifecycle, closed state, config options
   - `__test__/query.spec.ts`: simple query, parameterized query, CRUD operations
   - `__test__/transaction.spec.ts`: commit, rollback, conflict, closed state
   - `__test__/result.spec.ts`: columns, iteration, indexing, row access, toArray
   - `__test__/values.spec.ts`: all type conversions, roundtrip, edge cases
   - `__test__/errors.spec.ts`: error types, code property, instanceof checks
   - `__test__/setup.ts`: shared test setup (temp directory, sample data)
5. Create `crates/cypherlite-node/README.md`
6. Update CI to include Node.js test job (`napi build` + `vitest`)

**Requirements**: REQ-FFI-NODE-100 ~ REQ-FFI-NODE-111, REQ-FFI-NODE-TEST-*, REQ-FFI-NODE-DOC-*, REQ-FFI-NODE-NFR-*

**Acceptance**: tsc passes with no errors, all vitest tests pass, README includes examples, CI green.

---

## File Impact Analysis

### New Files

| File                                                | Purpose                          |
| --------------------------------------------------- | -------------------------------- |
| `crates/cypherlite-node/Cargo.toml`                 | Crate manifest (napi-rs)         |
| `crates/cypherlite-node/package.json`               | npm package config               |
| `crates/cypherlite-node/tsconfig.json`              | TypeScript config                |
| `crates/cypherlite-node/index.js`                   | Platform detection + loader      |
| `crates/cypherlite-node/src/lib.rs`                 | napi-rs module root              |
| `crates/cypherlite-node/src/database.rs`            | Database class (open/close)      |
| `crates/cypherlite-node/src/transaction.rs`         | Transaction class (commit/rollback) |
| `crates/cypherlite-node/src/result.rs`              | Result class (iter/len/get)      |
| `crates/cypherlite-node/src/value.rs`               | Value conversion (Rust <-> JS)   |
| `crates/cypherlite-node/src/error.rs`               | Error class mapping              |
| `crates/cypherlite-node/__test__/database.spec.ts`  | Database lifecycle tests         |
| `crates/cypherlite-node/__test__/query.spec.ts`     | Query execution tests            |
| `crates/cypherlite-node/__test__/transaction.spec.ts` | Transaction tests              |
| `crates/cypherlite-node/__test__/result.spec.ts`    | Result/Row access tests          |
| `crates/cypherlite-node/__test__/values.spec.ts`    | Value conversion tests           |
| `crates/cypherlite-node/__test__/errors.spec.ts`    | Error handling tests             |
| `crates/cypherlite-node/__test__/setup.ts`          | Shared test setup                |
| `crates/cypherlite-node/README.md`                  | Package documentation            |

### Modified Files

| File                          | Change                                               |
| ----------------------------- | ---------------------------------------------------- |
| `Cargo.toml` (workspace root) | Add `"crates/cypherlite-node"` to `members`         |
| `.github/workflows/ci.yml`    | Add Node.js binding test job                         |

### No-Change Files

- `crates/cypherlite-ffi/` (C ABI layer, not used by Node.js bindings)
- `crates/cypherlite-python/` (Python bindings, independent)
- `crates/cypherlite-core/` (consumed as dependency, no changes needed)
- `crates/cypherlite-storage/` (consumed as dependency, no changes needed)
- `crates/cypherlite-query/` (consumed as dependency, no changes needed)
- `bindings/go/cypherlite/` (Go bindings, independent)

---

## Risk Analysis

### R1: Rust Lifetime in napi-rs (High)

**Risk**: Rust `Transaction<'_>` holds a mutable borrow of `CypherLite`. napi-rs `#[napi]` structs require `'static` lifetime. Storing a borrowed `Transaction` inside a napi-rs class is not directly possible.

**Mitigation**: Use `Arc<Mutex<Option<CypherLite>>>` shared between `Database` and `Transaction`. Transaction manages explicit begin/commit/rollback state via the Mutex. This is the same proven pattern used in SPEC-FFI-003 (Python/PyO3) and `cypherlite-ffi` (C ABI).

### R2: Synchronous API and Event Loop Blocking (Medium)

**Risk**: Synchronous calls block the Node.js event loop. Long-running queries could make the application unresponsive.

**Mitigation**: CypherLite is an embedded database with sub-millisecond to low-millisecond query times. The sync API is intentional and matches `better-sqlite3`'s proven approach. For v1.0, document that long-running queries should be moved to Worker Threads if event loop responsiveness is critical. Async API is deferred to v2.0.

### R3: BigInt for IDs (Medium)

**Risk**: Using `BigInt` for NodeID/EdgeID values may surprise users who expect `number`. BigInt has different semantics (no `===` with number, no JSON serialization by default).

**Mitigation**: Document the BigInt choice clearly. Provide `Number(bigint)` conversion guidance for cases where IDs are within safe integer range. BigInt is necessary for 64-bit ID fidelity; `number` only supports 53-bit integers.

### R4: Row as Plain Object with Dual Access (Low)

**Risk**: Supporting both column-name properties and numeric-index properties on Row objects adds complexity and may cause confusion with `Object.keys()`.

**Mitigation**: Numeric keys are non-enumerable so they don't appear in `Object.keys()`. The `length` property is also non-enumerable. `toObject()` provides a clean column-name-only object when needed.

### R5: napi-rs Cross-Compilation (Low)

**Risk**: Building native addons for multiple platforms requires cross-compilation infrastructure.

**Mitigation**: napi-rs provides first-class CI support via `@napi-rs/cli` with GitHub Actions templates for cross-platform builds. The `optionalDependencies` pattern for platform-specific packages is well-established in the napi-rs ecosystem.

### R6: Value Conversion Performance (Low)

**Risk**: Eager conversion of all `QueryResult` rows to JavaScript objects at result construction may be slow for large result sets.

**Mitigation**: For v1.0, eager conversion is acceptable and simpler. Future optimization could implement lazy row conversion, but this adds complexity. Document the eager approach and defer lazy conversion to v2.0.

---

## Architecture Design Direction

```
JavaScript (user code)
       |
       v
index.js                    # Platform detection + native loader
       |
       v
cypherlite.*.node            # napi-rs native addon (cdylib)
       |
       v
cypherlite-node (Rust)       # napi-rs wrapper crate
       |
       v
cypherlite-query             # CypherLite facade API
       |
       v
cypherlite-storage           # Storage engine
       |
       v
cypherlite-core              # Common types
```

**Key Design Decision**: Direct Rust wrapping via napi-rs (not via C ABI) provides:
- Zero overhead from C ABI layer
- Rich type conversion via napi-rs `#[napi]` macro
- Native JavaScript Error support
- Automatic memory management (napi-rs handles JavaScript GC integration)
- Simpler build pipeline (no separate C library build step)
- Auto-generated TypeScript definitions from Rust types

---

## Dependencies

### Rust Dependencies (crates/cypherlite-node/Cargo.toml)

| Crate              | Version  | Features                        | Purpose                |
| ------------------ | -------- | ------------------------------- | ---------------------- |
| `napi`             | `3`      | `napi6`                         | N-API Rust bindings    |
| `napi-derive`      | `3`      |                                 | Procedural macros      |
| `cypherlite-query` | `path`   | (all features propagated)       | CypherLite API         |
| `cypherlite-core`  | `path`   | (all features propagated)       | Common types/errors    |

### Rust Build Dependencies

| Crate              | Version  | Purpose                          |
| ------------------ | -------- | -------------------------------- |
| `napi-build`       | `2`      | Build script for napi-rs         |

### Node.js Dependencies (development only)

| Package            | Version  | Purpose                          |
| ------------------ | -------- | -------------------------------- |
| `@napi-rs/cli`     | `^3.0`   | Build tool                       |
| `vitest`           | `^3.0`   | Test framework                   |
| `typescript`       | `^5.7`   | Type checking                    |
