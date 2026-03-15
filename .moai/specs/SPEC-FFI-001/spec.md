# SPEC-FFI-001: C ABI FFI Bindings

| Field     | Value                                              |
| --------- | -------------------------------------------------- |
| ID        | SPEC-FFI-001                                       |
| Title     | C ABI FFI Bindings for CypherLite                  |
| Status    | Approved                                           |
| Version   | 1.0.0                                              |
| Created   | 2026-03-14                                         |
| Priority  | High                                               |
| Phase     | 12 / v0.12                                         |
| Crate     | cypherlite-ffi                                     |
| Depends   | cypherlite-core, cypherlite-storage, cypherlite-query |

---

## 1. Environment

- **Language**: Rust 1.84+ (MSRV), C11 consumers
- **Build**: Cargo workspace, cbindgen for header generation
- **CI**: GitHub Actions (6 parallel jobs: check, msrv, test, coverage 85%, security, bench-check)
- **Target Platforms**: Linux (x86_64, aarch64), macOS (x86_64, aarch64), Windows (x86_64)
- **Feature Flags**: temporal-core -> temporal-edge -> subgraph -> hypergraph -> full-temporal; plugin (independent)
- **Concurrency Model**: parking_lot RwLock, single-writer, MVCC snapshot isolation
- **Existing FFI**: None (greenfield)

## 2. Assumptions

- **A1**: The C ABI is the foundational layer that Go bindings (SPEC-FFI-002) will wrap via CGo.
- **A2**: Python (PyO3) and Node.js (napi-rs) bindings will bypass the C ABI and interact with Rust directly; they are out of scope.
- **A3**: Consumers of the C ABI are responsible for managing the lifetime of opaque pointers returned by the library.
- **A4**: The plugin system (4 traits + PluginRegistry) is excluded from this SPEC due to complexity; it will be addressed in a future SPEC.
- **A5**: cbindgen can generate a complete and correct C11 header from the `#[no_mangle] extern "C"` functions.
- **A6**: The `CypherLite` struct's internal use of `parking_lot::RwLock` makes the opaque pointer safe to share across threads for read operations, but write operations (execute, begin transaction) require external synchronization by the caller.
- **A7**: All feature-gated Value variants (Subgraph, Hyperedge, TemporalNode) will have corresponding feature-gated FFI functions.
- **A8**: The FFI crate will produce a C dynamic library (`cdylib`) and a C static library (`staticlib`).

## 3. Requirements

### 3.1 Crate Setup

**REQ-FFI-C-001** (Ubiquitous)
The `cypherlite-ffi` crate shall produce both `cdylib` and `staticlib` library outputs and be a member of the Cargo workspace.

**REQ-FFI-C-002** (Ubiquitous)
The `cypherlite-ffi` crate shall re-export all feature flags from `cypherlite-query` so that conditional compilation propagates correctly to FFI functions.

### 3.2 Opaque Pointer Types

**REQ-FFI-C-003** (Ubiquitous)
The system shall expose the following types as opaque pointers through the C ABI:
- `CylDb` (wraps `CypherLite`)
- `CylTx` (wraps `Transaction<'_>` via an owning wrapper that holds `&mut CypherLite`)
- `CylResult` (wraps `QueryResult`)
- `CylRow` (wraps borrowed `Row`)

**REQ-FFI-C-004** (Ubiquitous)
Each opaque pointer type shall be created exclusively through corresponding `cyl_*` constructor functions and freed exclusively through corresponding `cyl_*_free` functions.

### 3.3 Database Lifecycle

**REQ-FFI-C-005** (Event-Driven)
**When** a C caller invokes `cyl_db_open(path, error_out)`, **then** the system shall create a `CypherLite` instance with default `DatabaseConfig` (using the provided path) and return a valid `*mut CylDb` pointer, or set `*error_out` to a non-zero error code and return null on failure.

**REQ-FFI-C-006** (Event-Driven)
**When** a C caller invokes `cyl_db_open_with_config(path, page_size, cache_capacity, error_out)`, **then** the system shall create a `CypherLite` instance with the specified configuration parameters.

**REQ-FFI-C-007** (Event-Driven)
**When** a C caller invokes `cyl_db_close(db)`, **then** the system shall drop the `CypherLite` instance, flushing all pending writes, and the pointer shall become invalid.

**REQ-FFI-C-008** (Unwanted)
The system shall not crash or cause undefined behavior when `cyl_db_close` is called with a null pointer; it shall be a no-op.

### 3.4 Query Execution

**REQ-FFI-C-009** (Event-Driven)
**When** a C caller invokes `cyl_db_execute(db, query, error_out)` with a valid UTF-8 null-terminated query string, **then** the system shall execute the Cypher query and return a `*mut CylResult` pointer, or set `*error_out` and return null on failure.

**REQ-FFI-C-010** (Event-Driven)
**When** a C caller invokes `cyl_db_execute_with_params(db, query, param_keys, param_values, param_count, error_out)`, **then** the system shall execute the parameterized Cypher query with the provided key-value parameter pairs.

**REQ-FFI-C-011** (Unwanted)
The system shall not accept non-UTF-8 query strings; it shall return error code `CYL_ERR_INVALID_UTF8` when the input is not valid UTF-8.

### 3.5 Transaction Support

**REQ-FFI-C-012** (Event-Driven)
**When** a C caller invokes `cyl_tx_begin(db, error_out)`, **then** the system shall begin a write transaction and return a `*mut CylTx` pointer, or set `*error_out` to `CYL_ERR_TRANSACTION_CONFLICT` and return null if another write transaction is active.

**REQ-FFI-C-013** (Event-Driven)
**When** a C caller invokes `cyl_tx_execute(tx, query, error_out)`, **then** the system shall execute the query within the transaction context.

**REQ-FFI-C-014** (Event-Driven)
**When** a C caller invokes `cyl_tx_execute_with_params(tx, query, param_keys, param_values, param_count, error_out)`, **then** the system shall execute the parameterized query within the transaction context.

**REQ-FFI-C-015** (Event-Driven)
**When** a C caller invokes `cyl_tx_commit(tx, error_out)`, **then** the system shall commit all changes and free the transaction handle. The pointer becomes invalid after this call.

**REQ-FFI-C-016** (Event-Driven)
**When** a C caller invokes `cyl_tx_rollback(tx)`, **then** the system shall discard all changes and free the transaction handle.

**REQ-FFI-C-017** (Event-Driven)
**When** a C caller invokes `cyl_tx_free(tx)` on an uncommitted transaction, **then** the system shall automatically rollback before freeing resources (matching Rust Drop semantics).

### 3.6 Result Access

**REQ-FFI-C-018** (Event-Driven)
**When** a C caller invokes `cyl_result_column_count(result)`, **then** the system shall return the number of columns in the result set as `uint32_t`.

**REQ-FFI-C-019** (Event-Driven)
**When** a C caller invokes `cyl_result_column_name(result, index)`, **then** the system shall return a pointer to a null-terminated UTF-8 column name string, or null if index is out of bounds. The string is owned by the result and valid until `cyl_result_free`.

**REQ-FFI-C-020** (Event-Driven)
**When** a C caller invokes `cyl_result_row_count(result)`, **then** the system shall return the number of rows in the result set as `uint64_t`.

**REQ-FFI-C-021** (Event-Driven)
**When** a C caller invokes `cyl_result_row(result, index)`, **then** the system shall return a `*const CylRow` pointer to the row at the given index, or null if the index is out of bounds. The row is borrowed from the result and valid until `cyl_result_free`.

**REQ-FFI-C-022** (Event-Driven)
**When** a C caller invokes `cyl_result_free(result)`, **then** the system shall free the result and all associated memory. Null input shall be a no-op.

### 3.7 Row Value Access

**REQ-FFI-C-023** (Event-Driven)
**When** a C caller invokes `cyl_row_get(row, column_index)`, **then** the system shall return a `CylValue` tagged union representing the value at the given column index.

**REQ-FFI-C-024** (Event-Driven)
**When** a C caller invokes `cyl_row_get_by_name(row, column_name)`, **then** the system shall return a `CylValue` tagged union for the named column, or a value with tag `CYL_VALUE_NULL` if the column does not exist.

### 3.8 Value Type System (Tagged Union)

**REQ-FFI-C-025** (Ubiquitous)
The system shall expose the `Value` enum as a C tagged union `CylValue` with the following structure:
- A `uint8_t tag` field identifying the variant
- A `union` payload containing variant-specific data

**REQ-FFI-C-026** (Ubiquitous)
The tag byte shall use the following mapping for core variants:

| Tag Constant          | Value | Payload                               |
| --------------------- | ----- | ------------------------------------- |
| `CYL_VALUE_NULL`      | 0     | (none)                                |
| `CYL_VALUE_BOOL`      | 1     | `bool` (uint8_t, 0 or 1)             |
| `CYL_VALUE_INT64`     | 2     | `int64_t`                             |
| `CYL_VALUE_FLOAT64`   | 3     | `double`                              |
| `CYL_VALUE_STRING`    | 4     | `const char*` (null-terminated UTF-8) |
| `CYL_VALUE_BYTES`     | 5     | `const uint8_t* data` + `size_t len`  |
| `CYL_VALUE_LIST`      | 6     | `const CylValue* items` + `size_t len`|
| `CYL_VALUE_NODE`      | 7     | `uint64_t` (NodeId)                   |
| `CYL_VALUE_EDGE`      | 8     | `uint64_t` (EdgeId)                   |
| `CYL_VALUE_DATETIME`  | 9     | `int64_t` (ms since Unix epoch)       |

**REQ-FFI-C-027** (State-Driven)
**While** the `subgraph` feature is enabled, the system shall additionally support:

| Tag Constant            | Value | Payload              |
| ----------------------- | ----- | -------------------- |
| `CYL_VALUE_SUBGRAPH`    | 10    | `uint64_t` (SubgraphId) |

**REQ-FFI-C-028** (State-Driven)
**While** the `hypergraph` feature is enabled, the system shall additionally support:

| Tag Constant             | Value | Payload                              |
| ------------------------ | ----- | ------------------------------------ |
| `CYL_VALUE_HYPEREDGE`    | 11    | `uint64_t` (HyperEdgeId)            |
| `CYL_VALUE_TEMPORAL_NODE`| 12    | `uint64_t node_id` + `int64_t time` |

**REQ-FFI-C-029** (Event-Driven)
**When** a `CylValue` contains heap-allocated data (String, Bytes, List), the system shall provide `cyl_value_free(value)` to release the memory. Values returned from `cyl_row_get*` are borrowed (no free needed); values returned from standalone conversion functions are owned.

### 3.9 Parameter Value Construction

**REQ-FFI-C-030** (Ubiquitous)
The system shall provide constructor functions for building parameter values from C types:
- `cyl_param_null()` -> `CylValue`
- `cyl_param_bool(uint8_t)` -> `CylValue`
- `cyl_param_int64(int64_t)` -> `CylValue`
- `cyl_param_float64(double)` -> `CylValue`
- `cyl_param_string(const char*)` -> `CylValue` (copies the string)
- `cyl_param_bytes(const uint8_t*, size_t)` -> `CylValue` (copies the bytes)

### 3.10 Error Handling

**REQ-FFI-C-031** (Ubiquitous)
The system shall define a `CylError` enum (represented as `int32_t`) with the following error codes:

| Error Constant                  | Value | Maps to                       |
| ------------------------------- | ----- | ----------------------------- |
| `CYL_OK`                        | 0     | Success                       |
| `CYL_ERR_IO`                    | 1     | `IoError`                     |
| `CYL_ERR_CORRUPTED_PAGE`        | 2     | `CorruptedPage`               |
| `CYL_ERR_TRANSACTION_CONFLICT`  | 3     | `TransactionConflict`         |
| `CYL_ERR_OUT_OF_SPACE`          | 4     | `OutOfSpace`                  |
| `CYL_ERR_INVALID_MAGIC`         | 5     | `InvalidMagicNumber`          |
| `CYL_ERR_UNSUPPORTED_VERSION`   | 6     | `UnsupportedVersion`          |
| `CYL_ERR_CHECKSUM`              | 7     | `ChecksumMismatch`            |
| `CYL_ERR_SERIALIZATION`         | 8     | `SerializationError`          |
| `CYL_ERR_NODE_NOT_FOUND`        | 9     | `NodeNotFound`                |
| `CYL_ERR_EDGE_NOT_FOUND`        | 10    | `EdgeNotFound`                |
| `CYL_ERR_PARSE`                 | 11    | `ParseError`                  |
| `CYL_ERR_SEMANTIC`              | 12    | `SemanticError`               |
| `CYL_ERR_EXECUTION`             | 13    | `ExecutionError`              |
| `CYL_ERR_UNSUPPORTED_SYNTAX`    | 14    | `UnsupportedSyntax`           |
| `CYL_ERR_CONSTRAINT_VIOLATION`  | 15    | `ConstraintViolation`         |
| `CYL_ERR_INVALID_DATETIME`      | 16    | `InvalidDateTimeFormat`       |
| `CYL_ERR_SYSTEM_PROPERTY`       | 17    | `SystemPropertyReadOnly`      |
| `CYL_ERR_FEATURE_INCOMPATIBLE`  | 18    | `FeatureIncompatible`         |
| `CYL_ERR_NULL_POINTER`          | 19    | (FFI-specific, null argument) |
| `CYL_ERR_INVALID_UTF8`          | 20    | (FFI-specific, bad string)    |

**REQ-FFI-C-032** (State-Driven)
**While** the `subgraph` feature is enabled, additional error codes shall be available:

| Error Constant                  | Value | Maps to                       |
| ------------------------------- | ----- | ----------------------------- |
| `CYL_ERR_SUBGRAPH_NOT_FOUND`    | 100   | `SubgraphNotFound`            |
| `CYL_ERR_FEATURE_REQUIRES_SUBGRAPH` | 101 | `FeatureRequiresSubgraph`  |

**REQ-FFI-C-033** (State-Driven)
**While** the `hypergraph` feature is enabled:

| Error Constant                  | Value | Maps to                       |
| ------------------------------- | ----- | ----------------------------- |
| `CYL_ERR_HYPEREDGE_NOT_FOUND`   | 200   | `HyperEdgeNotFound`           |

**REQ-FFI-C-034** (Event-Driven)
**When** any FFI function fails, the system shall store a detailed, null-terminated UTF-8 error message in a thread-local buffer. The function `cyl_last_error_message()` shall return a pointer to this message, valid until the next FFI call on the same thread.

**REQ-FFI-C-035** (Event-Driven)
**When** `cyl_last_error_message()` is called and no error has occurred, the system shall return a null pointer.

### 3.11 String Handling

**REQ-FFI-C-036** (Ubiquitous)
All string parameters passed from C to Rust shall be `const char*` null-terminated UTF-8. The FFI layer shall validate UTF-8 before conversion and return `CYL_ERR_INVALID_UTF8` on failure.

**REQ-FFI-C-037** (Ubiquitous)
All strings returned from Rust to C shall be null-terminated UTF-8. Strings owned by opaque objects (column names, error messages) are borrowed and must not be freed by the caller. Strings returned as standalone values must be freed via `cyl_string_free`.

### 3.12 Thread Safety

**REQ-FFI-C-038** (Ubiquitous)
The `CylDb` opaque pointer shall be safe to share between threads (`Send + Sync`). Concurrent calls to read-only functions (e.g., query execution returning results) are safe due to the internal `RwLock`. Concurrent write operations require external synchronization by the caller.

**REQ-FFI-C-039** (Ubiquitous)
The `CylTx` opaque pointer shall not be safe to share between threads. It must be used only on the thread that created it. This shall be documented in the C header.

**REQ-FFI-C-040** (Ubiquitous)
The thread-local error message buffer (`cyl_last_error_message`) shall be safe in multi-threaded environments, with each thread maintaining its own error state.

### 3.13 C Header Generation

**REQ-FFI-C-041** (Ubiquitous)
The system shall use `cbindgen` to generate a C11-compatible header file `cypherlite.h` from the Rust source.

**REQ-FFI-C-042** (Ubiquitous)
The generated header shall include:
- All opaque type declarations (`typedef struct CylDb CylDb;`)
- The `CylValue` tagged union definition
- All `CylError` enum constants
- All `CYL_VALUE_*` tag constants
- All function declarations with doc comments
- `#ifdef __cplusplus extern "C"` guards

**REQ-FFI-C-043** (Ubiquitous)
The `cbindgen.toml` configuration shall be checked into the repository and the header generation shall be reproducible via `cbindgen --config cbindgen.toml --crate cypherlite-ffi --output include/cypherlite.h`.

### 3.14 Feature Flag Support

**REQ-FFI-C-044** (State-Driven)
**While** a feature flag is disabled at compile time, the corresponding FFI functions and type variants shall not be present in the compiled library or generated header.

**REQ-FFI-C-045** (Ubiquitous)
The system shall provide `cyl_version()` returning a null-terminated version string and `cyl_features()` returning a comma-separated list of enabled feature flags.

---

## 4. Non-Functional Requirements

**REQ-FFI-NFR-001** (Performance)
FFI function call overhead shall not exceed 1 microsecond per call (excluding the actual database operation).

**REQ-FFI-NFR-002** (Memory)
The FFI layer shall not perform hidden heap allocations beyond those documented in the API (opaque pointer creation, string copies for parameters, value copies for returns).

**REQ-FFI-NFR-003** (Safety)
Every `unsafe` block in the FFI crate shall have a `// SAFETY:` comment documenting the invariants that make the operation sound.

**REQ-FFI-NFR-004** (Compatibility)
The generated C header shall compile without warnings under C11 mode with gcc, clang, and MSVC.

**REQ-FFI-NFR-005** (ABI Stability)
Function signatures and error codes defined in this SPEC are considered a public contract. Changes require a new major version.

---

## 5. Testing Requirements

**REQ-FFI-TEST-001** (Ubiquitous)
The crate shall include Rust integration tests that call through the FFI boundary using the `extern "C"` functions, verifying the complete lifecycle: open -> execute -> read results -> close.

**REQ-FFI-TEST-002** (Ubiquitous)
The crate shall include a C test program (`tests/ffi_test.c`) that compiles against the generated header and links against the built library, exercising the core API surface.

**REQ-FFI-TEST-003** (Ubiquitous)
Memory safety shall be validated using Miri (`cargo +nightly miri test`) for Rust-side tests and Valgrind/AddressSanitizer for the C test program in CI.

**REQ-FFI-TEST-004** (Event-Driven)
**When** a `cyl_*_free` function is called with a null pointer, it shall be a no-op (no crash, no error).

**REQ-FFI-TEST-005** (Event-Driven)
**When** a `cyl_*_free` function is called twice on the same pointer, the behavior is undefined. The documentation shall state this explicitly (consistent with C `free()` semantics).

**REQ-FFI-TEST-006** (Ubiquitous)
Error propagation shall be tested for each `CylError` variant to verify correct error code and message.

**REQ-FFI-TEST-007** (Ubiquitous)
All FFI functions shall be tested with null pointer inputs to verify graceful error handling (no segfaults).

---

## 6. Documentation Requirements

**REQ-FFI-DOC-001** (Ubiquitous)
The generated C header shall include doc comments on every function describing parameters, return values, ownership semantics, and thread safety.

**REQ-FFI-DOC-002** (Ubiquitous)
The crate shall include a `README.md` with:
- Build instructions for the shared/static library
- A minimal C usage example (open, query, iterate results, close)
- Platform-specific linking instructions (Linux, macOS, Windows)

---

## 7. Out of Scope

- Go bindings (future SPEC-FFI-002)
- Python bindings via PyO3 (future SPEC-FFI-003)
- Node.js bindings via napi-rs (future SPEC-FFI-004)
- Plugin system FFI exposure (ScalarFunction, IndexPlugin, Serializer, Trigger traits)
- StorageEngine direct FFI access (consumers use CypherLite facade only)
- Async/callback-based API (synchronous C ABI only)

---

## 8. Traceability

| Requirement      | Plan Milestone | Acceptance Criteria       |
| ---------------- | -------------- | ------------------------- |
| REQ-FFI-C-001~002 | M1           | AC-SETUP-*                |
| REQ-FFI-C-003~008 | M2           | AC-LIFECYCLE-*            |
| REQ-FFI-C-009~011 | M3           | AC-QUERY-*                |
| REQ-FFI-C-012~017 | M4           | AC-TX-*                   |
| REQ-FFI-C-018~024 | M5           | AC-RESULT-*               |
| REQ-FFI-C-025~030 | M6           | AC-VALUE-*                |
| REQ-FFI-C-031~040 | M2 (error), M2 (thread) | AC-ERROR-*, AC-THREAD-* |
| REQ-FFI-C-041~045 | M7           | AC-HEADER-*, AC-FEATURE-* |
| REQ-FFI-TEST-*   | M7            | AC-SAFETY-*               |
| REQ-FFI-DOC-*    | M8            | AC-DOC-*                  |
