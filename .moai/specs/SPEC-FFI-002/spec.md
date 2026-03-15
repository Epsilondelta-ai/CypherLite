# SPEC-FFI-002: Go Bindings via CGo

| Field     | Value                                              |
| --------- | -------------------------------------------------- |
| ID        | SPEC-FFI-002                                       |
| Title     | Go Bindings for CypherLite via CGo                 |
| Status    | Approved                                           |
| Version   | 1.0.0                                              |
| Created   | 2026-03-15                                         |
| Priority  | High                                               |
| Phase     | 12 / v0.12                                         |
| Package   | github.com/Epsilondelta-ai/cypherlite-go           |
| Depends   | SPEC-FFI-001 (cypherlite-ffi C ABI)                |

---

## 1. Environment

- **Go Version**: 1.21+ (minimum)
- **C ABI**: cypherlite-ffi crate (SPEC-FFI-001, 28 extern "C" functions, cbindgen-generated `cypherlite.h`)
- **Binding Mechanism**: CGo with `#cgo` directives linking to pre-built `libcypherlite_ffi` (cdylib or staticlib)
- **Target Platforms**: Linux (x86_64, aarch64), macOS (x86_64, aarch64), Windows (x86_64)
- **Feature Flags**: Rust feature flags (`subgraph`, `hypergraph`) mapped to Go build tags
- **CI**: GitHub Actions (go test, go vet, race detector, golangci-lint)
- **Package Location**: `bindings/go/cypherlite/` directory inside the CypherLite repository (Go module: `github.com/Epsilondelta-ai/cypherlite-go`)
- **Test Prerequisites**: Pre-built `libcypherlite_ffi.{so,dylib,a}` and `cypherlite.h` available at known paths

## 2. Assumptions

- **A1**: The C ABI layer (SPEC-FFI-001) is stable and complete. All 28 extern "C" functions in `include/cypherlite.h` are available.
- **A2**: Go callers use the Go module as a library (not as a standalone binary). The module is imported as `github.com/Epsilondelta-ai/cypherlite-go`.
- **A3**: The pre-built cypherlite-ffi library (cdylib or staticlib) must be installed on the system or specified via `CGO_LDFLAGS` / `CGO_CFLAGS` before `go build`.
- **A4**: Go goroutines may migrate between OS threads. Every CGo call that may set a thread-local error must be wrapped with `runtime.LockOSThread()` + `defer runtime.UnlockOSThread()`.
- **A5**: CylValue tagged union fields cannot be accessed directly from Go. The Go binding must use `unsafe.Pointer` arithmetic or C accessor helper functions.
- **A6**: String data borrowed from `CylResult` (column names, string values) must be copied to Go-managed memory before `CylResult` is freed.
- **A7**: Feature-gated value types (Subgraph, Hyperedge, TemporalNode) will be conditionally compiled using Go build tags (`//go:build subgraph`, `//go:build hypergraph`).
- **A8**: The Go package does not manage the Rust build process. Users must build `libcypherlite_ffi` separately before using the Go package.

## 3. Requirements

### 3.1 Go Module Setup

**REQ-FFI-GO-001** (Ubiquitous)
The Go package shall be located at `bindings/go/cypherlite/` with a `go.mod` declaring module path `github.com/Epsilondelta-ai/cypherlite-go` and minimum Go version 1.21.

**REQ-FFI-GO-002** (Ubiquitous)
The package shall contain a CGo bridge file (`cgo_bridge.go`) with `#cgo` directives that include the `cypherlite.h` header and link against `libcypherlite_ffi`.

**REQ-FFI-GO-003** (Ubiquitous)
The `#cgo` directives shall support customization of include and library paths via `CGO_CFLAGS` and `CGO_LDFLAGS` environment variables.

**REQ-FFI-GO-004** (Ubiquitous)
The package shall export all public API types and functions from a single `cypherlite` package namespace.

### 3.2 Database Lifecycle

**REQ-FFI-GO-010** (Event-Driven)
**When** a Go caller invokes `Open(path string)`, **then** the system shall call `cyl_db_open` via CGo, lock the OS thread for error retrieval, convert any error to a Go `error`, and return a `*DB` handle or an error.

**REQ-FFI-GO-011** (Event-Driven)
**When** a Go caller invokes `OpenWithConfig(path string, pageSize, cacheCapacity uint32)`, **then** the system shall call `cyl_db_open_with_config` with the specified configuration parameters and return a `*DB` handle or an error.

**REQ-FFI-GO-012** (Event-Driven)
**When** a Go caller invokes `db.Close()`, **then** the system shall call `cyl_db_close`, set the internal pointer to nil to prevent double-free, and return any error.

**REQ-FFI-GO-013** (Unwanted)
The system shall not cause a panic or segfault when `Close()` is called multiple times on the same `*DB` handle; subsequent calls shall be no-ops.

**REQ-FFI-GO-014** (Optional)
**Where** Go runtime finalizers are supported, the `*DB` type shall register a `runtime.SetFinalizer` that calls `Close()` as a safety net for leaked handles. The finalizer shall log a warning when triggered (indicating missing explicit `Close()`).

### 3.3 Query Execution

**REQ-FFI-GO-020** (Event-Driven)
**When** a Go caller invokes `db.Execute(query string)`, **then** the system shall convert the Go string to a C string, call `cyl_db_execute` via CGo (with OS thread locking), and return a `*Result` or an error.

**REQ-FFI-GO-021** (Event-Driven)
**When** a Go caller invokes `db.ExecuteWithParams(query string, params map[string]interface{})`, **then** the system shall:
1. Convert each Go key to a C string
2. Convert each Go value to a `CylValue` using the type mapping (see REQ-FFI-GO-060)
3. Call `cyl_db_execute_with_params` with the parallel key/value arrays
4. Free all temporary C allocations after the call returns
5. Return a `*Result` or an error

**REQ-FFI-GO-022** (Unwanted)
The system shall not accept unsupported parameter types (e.g., structs, slices of structs); it shall return an `ErrUnsupportedParamType` error.

### 3.4 Transaction Support

**REQ-FFI-GO-030** (Event-Driven)
**When** a Go caller invokes `db.Begin()`, **then** the system shall call `cyl_tx_begin` and return a `*Tx` handle or an error (e.g., `ErrTransactionConflict` if another transaction is active).

**REQ-FFI-GO-031** (Event-Driven)
**When** a Go caller invokes `tx.Execute(query string)`, **then** the system shall call `cyl_tx_execute` within the transaction context and return a `*Result` or an error.

**REQ-FFI-GO-032** (Event-Driven)
**When** a Go caller invokes `tx.ExecuteWithParams(query string, params map[string]interface{})`, **then** the system shall call `cyl_tx_execute_with_params` with converted parameters and return a `*Result` or an error.

**REQ-FFI-GO-033** (Event-Driven)
**When** a Go caller invokes `tx.Commit()`, **then** the system shall call `cyl_tx_commit`, invalidate the `*Tx` handle, and return any error.

**REQ-FFI-GO-034** (Event-Driven)
**When** a Go caller invokes `tx.Rollback()`, **then** the system shall call `cyl_tx_rollback` and invalidate the `*Tx` handle.

**REQ-FFI-GO-035** (Unwanted)
The system shall not cause a panic when `Commit()` or `Rollback()` is called on an already-consumed `*Tx`; it shall return `ErrTxClosed`.

**REQ-FFI-GO-036** (Optional)
**Where** Go runtime finalizers are supported, the `*Tx` type shall register a finalizer that calls `cyl_tx_free` (auto-rollback) as a safety net for leaked transactions.

### 3.5 Result Access

**REQ-FFI-GO-040** (Event-Driven)
**When** a Go caller invokes `result.Columns()`, **then** the system shall call `cyl_result_column_count` and `cyl_result_column_name` for each index, copying column names into a Go `[]string` slice.

**REQ-FFI-GO-041** (Event-Driven)
**When** a Go caller invokes `result.RowCount()`, **then** the system shall call `cyl_result_row_count` and return the count as `int`.

**REQ-FFI-GO-042** (Event-Driven)
**When** a Go caller invokes `result.Row(index int)`, **then** the system shall return a `*Row` value providing access to columns at the given row index.

**REQ-FFI-GO-043** (Event-Driven)
**When** a Go caller invokes `result.Close()`, **then** the system shall call `cyl_result_free`, set the internal pointer to nil, and prevent double-free.

**REQ-FFI-GO-044** (Ubiquitous)
All string and byte data from `CylValue` payloads shall be copied to Go-managed memory at the time of access. The Go `*Result` type shall ensure that no Go-visible pointers reference C-owned memory after `result.Close()`.

### 3.6 Row Value Access

**REQ-FFI-GO-050** (Event-Driven)
**When** a Go caller invokes `row.Get(colIndex int)`, **then** the system shall call `cyl_result_get` and convert the returned `CylValue` to a Go `interface{}` value.

**REQ-FFI-GO-051** (Event-Driven)
**When** a Go caller invokes `row.GetByName(colName string)`, **then** the system shall call `cyl_result_get_by_name` and convert the returned `CylValue` to a Go `interface{}` value.

### 3.7 Value Type Mapping

**REQ-FFI-GO-060** (Ubiquitous)
The system shall map C `CylValue` tags to Go types as follows:

| CylValueTag       | Go Type    | Notes                          |
| ------------------ | ---------- | ------------------------------ |
| CYL_VALUE_NULL     | nil        |                                |
| CYL_VALUE_BOOL     | bool       |                                |
| CYL_VALUE_INT64    | int64      |                                |
| CYL_VALUE_FLOAT64  | float64    |                                |
| CYL_VALUE_STRING   | string     | Copied from C to Go memory     |
| CYL_VALUE_BYTES    | []byte     | Copied from C to Go memory     |
| CYL_VALUE_LIST     | []interface{} | Recursive conversion        |
| CYL_VALUE_NODE     | NodeID     | type NodeID uint64             |
| CYL_VALUE_EDGE     | EdgeID     | type EdgeID uint64             |
| CYL_VALUE_DATETIME | DateTime   | type DateTime int64 (ms epoch) |

**REQ-FFI-GO-061** (State-Driven)
**While** the `subgraph` build tag is active, the system shall additionally support:

| CylValueTag          | Go Type      |
| --------------------- | ------------ |
| CYL_VALUE_SUBGRAPH    | SubgraphID   |

**REQ-FFI-GO-062** (State-Driven)
**While** the `hypergraph` build tag is active, the system shall additionally support:

| CylValueTag              | Go Type          |
| ------------------------- | ---------------- |
| CYL_VALUE_HYPEREDGE       | HyperEdgeID      |
| CYL_VALUE_TEMPORAL_NODE   | TemporalNodeRef  |

**REQ-FFI-GO-063** (Ubiquitous)
The Go-to-C parameter conversion for `ExecuteWithParams` shall support the following Go types:

| Go Type    | CylValue Constructor     |
| ---------- | ------------------------ |
| nil        | cyl_param_null()         |
| bool       | cyl_param_bool()         |
| int, int64 | cyl_param_int64()        |
| float64    | cyl_param_float64()      |
| string     | cyl_param_string()       |
| []byte     | cyl_param_bytes()        |

### 3.8 Error Handling

**REQ-FFI-GO-070** (Ubiquitous)
The system shall define a Go `Error` type implementing the `error` interface that includes:
- `Code` field: the `CylError` integer code
- `Message` field: the thread-local error message retrieved via `cyl_last_error_message()`

**REQ-FFI-GO-071** (Event-Driven)
**When** any CGo call returns a non-zero `CylError`, **then** the system shall:
1. Call `cyl_last_error_message()` to retrieve the detailed message (while OS thread is still locked)
2. Copy the message to a Go string
3. Return an `*Error` wrapping both the code and message

**REQ-FFI-GO-072** (Ubiquitous)
The system shall define typed sentinel errors for common conditions:
- `ErrTransactionConflict` (CYL_ERR_TRANSACTION_CONFLICT)
- `ErrNodeNotFound` (CYL_ERR_NODE_NOT_FOUND)
- `ErrEdgeNotFound` (CYL_ERR_EDGE_NOT_FOUND)
- `ErrParse` (CYL_ERR_PARSE)
- `ErrExecution` (CYL_ERR_EXECUTION)

These shall be detectable via `errors.Is()`.

**REQ-FFI-GO-073** (Ubiquitous)
The `Error` type shall implement `Unwrap() error` to support `errors.Is()` and `errors.As()` chains with sentinel errors.

### 3.9 Thread Safety

**REQ-FFI-GO-080** (Ubiquitous)
Every CGo function call that may set a thread-local error shall be preceded by `runtime.LockOSThread()` and followed by `defer runtime.UnlockOSThread()` to ensure the goroutine does not migrate to another OS thread between the function call and `cyl_last_error_message()` retrieval.

**REQ-FFI-GO-081** (Ubiquitous)
The `*DB` type shall be safe for concurrent use from multiple goroutines (matching the thread safety of the underlying `CylDb` with its internal Mutex).

**REQ-FFI-GO-082** (Ubiquitous)
The `*Tx` type shall document that it is NOT safe for concurrent use. A `*Tx` must be used on a single goroutine at a time.

**REQ-FFI-GO-083** (Ubiquitous)
The `*Result` type shall document that it is NOT safe for concurrent use. Concurrent value access from a single `*Result` requires external synchronization.

### 3.10 Build Configuration

**REQ-FFI-GO-090** (Ubiquitous)
The package shall support static linking via `#cgo LDFLAGS: -lcypherlite_ffi -lm -ldl -lpthread` (or platform-appropriate flags).

**REQ-FFI-GO-091** (Ubiquitous)
The package shall support dynamic linking via `#cgo LDFLAGS: -lcypherlite_ffi` with shared library in `LD_LIBRARY_PATH` / `DYLD_LIBRARY_PATH`.

**REQ-FFI-GO-092** (State-Driven)
**While** the `subgraph` Go build tag is active, the CGo bridge shall define `CYL_FEATURE_SUBGRAPH` before including the header, enabling subgraph-specific types and functions.

**REQ-FFI-GO-093** (State-Driven)
**While** the `hypergraph` Go build tag is active, the CGo bridge shall define `CYL_FEATURE_HYPERGRAPH` before including the header, enabling hypergraph-specific types and functions.

**REQ-FFI-GO-094** (Ubiquitous)
The package shall provide a `cypherlite_static.go` file with `//go:build cypherlite_static` tag for static linking configuration, and a default file for dynamic linking.

### 3.11 Library Info Functions

**REQ-FFI-GO-100** (Event-Driven)
**When** a Go caller invokes `Version()`, **then** the system shall call `cyl_version()` and return the version string as a Go `string`.

**REQ-FFI-GO-101** (Event-Driven)
**When** a Go caller invokes `Features()`, **then** the system shall call `cyl_features()` and return the comma-separated feature flags as a Go `string`.

### 3.12 Testing Requirements

**REQ-FFI-GO-TEST-001** (Ubiquitous)
The package shall include Go test files exercising the complete API lifecycle: Open -> Execute -> Read Results -> Close.

**REQ-FFI-GO-TEST-002** (Ubiquitous)
Tests shall use `go test -race` to detect data races in concurrent access patterns.

**REQ-FFI-GO-TEST-003** (Ubiquitous)
Tests shall cover all error conditions: null pointer safety, transaction conflicts, invalid queries, UTF-8 validation.

**REQ-FFI-GO-TEST-004** (Ubiquitous)
Tests shall verify that all CylValue type conversions round-trip correctly (Go -> C -> Go).

**REQ-FFI-GO-TEST-005** (Ubiquitous)
Tests shall verify resource cleanup: no leaked `CylDb`, `CylTx`, or `CylResult` handles after test completion.

**REQ-FFI-GO-TEST-006** (Event-Driven)
**When** the `subgraph` or `hypergraph` build tags are active, tests shall verify the corresponding feature-gated value types.

---

## 4. Non-Functional Requirements

**REQ-FFI-GO-NFR-001** (Performance)
CGo crossing overhead shall be documented but not optimized beyond CGo's inherent cost (~100ns per call). The package shall minimize unnecessary CGo crossings (e.g., batch column name retrieval).

**REQ-FFI-GO-NFR-002** (Memory)
The Go binding shall not hold references to C-owned memory beyond the lifetime of the owning C object. All string/byte data shall be copied to Go-managed memory at access time.

**REQ-FFI-GO-NFR-003** (Documentation)
All exported types and functions shall have GoDoc comments explaining parameters, return values, ownership semantics, and thread safety.

**REQ-FFI-GO-NFR-004** (Compatibility)
The package shall compile and pass tests on Linux (x86_64), macOS (x86_64, aarch64), and Windows (x86_64) with Go 1.21+.

**REQ-FFI-GO-NFR-005** (Idiomatic Go)
The API shall follow Go conventions: method receivers on pointer types, `error` return as last value, `Close()` methods for resources, `interface{}` for dynamic values.

---

## 5. Out of Scope

- Rust build automation from Go (users build `libcypherlite_ffi` separately)
- Python bindings (future SPEC-FFI-003 via PyO3, bypasses C ABI)
- Node.js bindings (future SPEC-FFI-004 via napi-rs, bypasses C ABI)
- Plugin system exposure (ScalarFunction, IndexPlugin, Serializer, Trigger)
- Async/callback-based Go API (synchronous CGo only)
- Connection pooling or ORM-like abstractions
- Automatic Rust cross-compilation from Go build

---

## 6. Traceability

| Requirement           | Plan Milestone | Acceptance Criteria         |
| --------------------- | -------------- | --------------------------- |
| REQ-FFI-GO-001~004    | M1             | AC-MODULE-*                 |
| REQ-FFI-GO-010~014    | M2             | AC-LIFECYCLE-*              |
| REQ-FFI-GO-020~022    | M3             | AC-QUERY-*                  |
| REQ-FFI-GO-030~036    | M4             | AC-TX-*                     |
| REQ-FFI-GO-040~044    | M5             | AC-RESULT-*                 |
| REQ-FFI-GO-050~051    | M5             | AC-ROW-*                    |
| REQ-FFI-GO-060~063    | M6             | AC-VALUE-*                  |
| REQ-FFI-GO-070~073    | M2             | AC-ERROR-*                  |
| REQ-FFI-GO-080~083    | M2             | AC-THREAD-*                 |
| REQ-FFI-GO-090~094    | M1             | AC-BUILD-*                  |
| REQ-FFI-GO-100~101    | M1             | AC-INFO-*                   |
| REQ-FFI-GO-TEST-*     | M7             | AC-TEST-*                   |
| REQ-FFI-GO-NFR-*      | M7             | AC-NFR-*                    |
