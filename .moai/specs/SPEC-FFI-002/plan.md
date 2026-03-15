# SPEC-FFI-002: Implementation Plan

| Field     | Value                          |
| --------- | ------------------------------ |
| SPEC      | SPEC-FFI-002                   |
| Title     | Go Bindings via CGo - Plan     |
| Version   | 1.0.0                          |
| Created   | 2026-03-15                     |

---

## 1. Milestone Overview

```
M1 ──> M2 ──> M3 ──> M4 ──> M5 ──> M6 ──> M7
 |      |      |      |      |      |      |
Module  Life   Query   Tx   Result  Value  Test
+CGo   cycle+          +             Type  +Doc
Bridge Error          Params         Map   +CI
```

**Dependency Graph:**
- M1: No dependencies (Go module bootstrap + CGo bridge)
- M2: Depends on M1 (needs CGo bridge for DB lifecycle + error handling)
- M3: Depends on M2 (needs DB handle for query execution)
- M4: Depends on M2 (needs DB handle for transaction begin)
- M5: Depends on M3 (needs query results to access)
- M6: Depends on M5 (needs result access for value conversion)
- M7: Depends on M6 (needs full API for integration tests + CI)

Note: M3 and M4 are independent of each other and may be developed in parallel after M2.

---

## 2. Milestones

### M1: Go Module Setup + CGo Bridge + Build Configuration

**Priority**: Primary Goal

**Tasks:**
1. Create `bindings/go/cypherlite/` directory structure
2. Initialize `go.mod` with module path `github.com/Epsilondelta-ai/cypherlite-go` and Go 1.21 minimum
3. Create `cgo_bridge.go` with `#cgo` directives:
   - `#cgo CFLAGS: -I${SRCDIR}/../../../crates/cypherlite-ffi/include`
   - `#cgo LDFLAGS: -L${SRCDIR}/../../../target/release -lcypherlite_ffi`
   - Platform-specific linker flags (pthread, dl, m for Linux; framework for macOS)
   - `#include "cypherlite.h"` after C preamble
4. Create `cypherlite_static.go` with `//go:build cypherlite_static` for static linking flags
5. Create `cypherlite_dynamic.go` with `//go:build !cypherlite_static` for dynamic linking (default)
6. Create `version.go` with `Version()` and `Features()` functions wrapping `cyl_version`/`cyl_features`
7. Create `types.go` with Go type definitions: `NodeID`, `EdgeID`, `DateTime`, `Error`
8. Create feature-gated files: `types_subgraph.go` (`//go:build subgraph`), `types_hypergraph.go` (`//go:build hypergraph`)
9. Write initial test: verify `Version()` returns non-empty string, `Features()` returns valid string

**File Impact:**
- NEW: `bindings/go/cypherlite/go.mod`
- NEW: `bindings/go/cypherlite/cgo_bridge.go`
- NEW: `bindings/go/cypherlite/cypherlite_static.go`
- NEW: `bindings/go/cypherlite/cypherlite_dynamic.go`
- NEW: `bindings/go/cypherlite/version.go`
- NEW: `bindings/go/cypherlite/types.go`
- NEW: `bindings/go/cypherlite/types_subgraph.go`
- NEW: `bindings/go/cypherlite/types_hypergraph.go`
- NEW: `bindings/go/cypherlite/version_test.go`

**Requirements Covered:** REQ-FFI-GO-001~004, REQ-FFI-GO-090~094, REQ-FFI-GO-100~101

---

### M2: Database Lifecycle + Error Handling + Thread Safety

**Priority**: Primary Goal

**Tasks:**
1. Create `errors.go`:
   - Define `Error` struct with `Code int32` and `Message string` fields
   - Implement `error` interface (`Error() string`)
   - Implement `Unwrap() error` for `errors.Is()` support
   - Define sentinel errors: `ErrTransactionConflict`, `ErrNodeNotFound`, `ErrEdgeNotFound`, `ErrParse`, `ErrExecution`, `ErrUnsupportedParamType`, `ErrTxClosed`
   - Create `errorFromCode(code C.CylError) *Error` helper that calls `C.cyl_last_error_message()` while thread is locked
2. Create `db.go`:
   - Define `DB` struct with `ptr *C.CylDb` and `mu sync.Mutex` for nil-safety on close
   - Implement `Open(path string) (*DB, error)`: LockOSThread, C string conversion, cyl_db_open, error check, SetFinalizer
   - Implement `OpenWithConfig(path string, pageSize, cacheCapacity uint32) (*DB, error)`
   - Implement `Close() error`: nil-check, cyl_db_close, set ptr to nil
   - Implement `isClosed() bool` helper
   - Register `runtime.SetFinalizer` on DB creation for safety net
3. Create `thread.go`:
   - Helper `lockThread()` and `unlockThread()` wrapping `runtime.LockOSThread()`
   - Pattern documentation for CGo + thread-local error retrieval
4. Write tests:
   - Open/Close lifecycle
   - Double-close safety (no panic)
   - Open with invalid path (error handling)
   - OpenWithConfig with custom settings
   - Error code to Go error conversion
   - Sentinel error matching via `errors.Is()`
   - Thread safety: concurrent Open from multiple goroutines

**File Impact:**
- NEW: `bindings/go/cypherlite/errors.go`
- NEW: `bindings/go/cypherlite/db.go`
- NEW: `bindings/go/cypherlite/thread.go`
- NEW: `bindings/go/cypherlite/errors_test.go`
- NEW: `bindings/go/cypherlite/db_test.go`

**Requirements Covered:** REQ-FFI-GO-010~014, REQ-FFI-GO-070~073, REQ-FFI-GO-080~083

---

### M3: Query Execution

**Priority**: Primary Goal

**Tasks:**
1. Create `query.go`:
   - Implement `db.Execute(query string) (*Result, error)`: C string conversion, LockOSThread, cyl_db_execute, result wrapping
   - Implement `db.ExecuteWithParams(query string, params map[string]interface{}) (*Result, error)`:
     - Convert Go map to parallel C arrays (keys: `**C.char`, values: `*C.CylValue`)
     - Call `cyl_db_execute_with_params`
     - Free temporary C allocations after call
   - Create `goValueToCylValue(v interface{}) (C.CylValue, error)` converter (Go -> C direction)
2. Write tests:
   - Simple CREATE + MATCH query
   - Parameterized query with various types
   - Invalid query (parse error)
   - Empty result set
   - Unsupported parameter type rejection

**File Impact:**
- NEW: `bindings/go/cypherlite/query.go`
- NEW: `bindings/go/cypherlite/query_test.go`

**Requirements Covered:** REQ-FFI-GO-020~022, REQ-FFI-GO-063

---

### M4: Transaction Support

**Priority**: Primary Goal

**Tasks:**
1. Create `tx.go`:
   - Define `Tx` struct with `ptr *C.CylTx`, `db *DB`, `closed bool`
   - Implement `db.Begin() (*Tx, error)`: LockOSThread, cyl_tx_begin, SetFinalizer
   - Implement `tx.Execute(query string) (*Result, error)`: closed-check, LockOSThread, cyl_tx_execute
   - Implement `tx.ExecuteWithParams(query string, params map[string]interface{}) (*Result, error)`
   - Implement `tx.Commit() error`: closed-check, LockOSThread, cyl_tx_commit, set closed=true
   - Implement `tx.Rollback() error`: closed-check, cyl_tx_rollback, set closed=true
   - Register `runtime.SetFinalizer` for auto-rollback safety net
2. Write tests:
   - Begin + Execute + Commit lifecycle
   - Begin + Execute + Rollback lifecycle
   - Transaction conflict detection (Begin while another Tx active)
   - Execute after Commit (ErrTxClosed)
   - Double Commit safety (ErrTxClosed, no panic)
   - Finalizer auto-rollback (verify no crash on GC)

**File Impact:**
- NEW: `bindings/go/cypherlite/tx.go`
- NEW: `bindings/go/cypherlite/tx_test.go`

**Requirements Covered:** REQ-FFI-GO-030~036

---

### M5: Result Access

**Priority**: Secondary Goal

**Tasks:**
1. Create `result.go`:
   - Define `Result` struct with `ptr *C.CylResult`, `columns []string` (cached), `rowCount int`
   - Implement `result.Columns() []string`: call cyl_result_column_count + cyl_result_column_name, cache and return
   - Implement `result.RowCount() int`: call cyl_result_row_count, return cached value
   - Implement `result.Close()`: cyl_result_free, set ptr to nil
   - Register `runtime.SetFinalizer` for safety
2. Create `row.go`:
   - Define `Row` struct with `result *Result`, `index int`
   - Implement `result.Row(index int) *Row`: bounds check, return Row
   - Implement `row.Get(colIndex int) interface{}`: call cyl_result_get, convert CylValue
   - Implement `row.GetByName(colName string) interface{}`: call cyl_result_get_by_name, convert CylValue
3. Write tests:
   - Column enumeration (names and count)
   - Row iteration
   - Get by index and by name
   - Out-of-bounds access (returns nil)
   - Empty result set
   - Double-close safety

**File Impact:**
- NEW: `bindings/go/cypherlite/result.go`
- NEW: `bindings/go/cypherlite/row.go`
- NEW: `bindings/go/cypherlite/result_test.go`

**Requirements Covered:** REQ-FFI-GO-040~044, REQ-FFI-GO-050~051

---

### M6: Value Type Conversion

**Priority**: Secondary Goal

**Tasks:**
1. Create `value.go`:
   - Implement `cylValueToGo(val C.CylValue) interface{}` converter:
     - Switch on `val.tag` for each CylValueTag constant
     - NULL -> nil
     - BOOL -> bool
     - INT64 -> int64
     - FLOAT64 -> float64
     - STRING -> string (C.GoString copy)
     - BYTES -> []byte (C.GoBytes copy)
     - LIST -> []interface{} (recursive)
     - NODE -> NodeID(uint64)
     - EDGE -> EdgeID(uint64)
     - DATETIME -> DateTime(int64)
   - Handle union access via unsafe.Pointer offset arithmetic or C helper macros
   - Document CylValue memory layout assumptions
2. Create feature-gated conversion files:
   - `value_subgraph.go` (`//go:build subgraph`): SUBGRAPH -> SubgraphID conversion
   - `value_hypergraph.go` (`//go:build hypergraph`): HYPEREDGE -> HyperEdgeID, TEMPORAL_NODE -> TemporalNodeRef
3. Write tests:
   - Round-trip for each value type (Create node with property -> Query -> Read value)
   - String with Unicode characters
   - Empty string and nil values
   - Large byte arrays
   - Nested lists (if supported)

**File Impact:**
- NEW: `bindings/go/cypherlite/value.go`
- NEW: `bindings/go/cypherlite/value_subgraph.go`
- NEW: `bindings/go/cypherlite/value_hypergraph.go`
- NEW: `bindings/go/cypherlite/value_test.go`

**Requirements Covered:** REQ-FFI-GO-060~062

---

### M7: Integration Tests + CI + Documentation

**Priority**: Final Goal

**Tasks:**
1. Create `integration_test.go`:
   - Full lifecycle test: Open -> CREATE nodes/edges -> MATCH query -> Read results -> Verify values -> Close
   - Transaction lifecycle: Begin -> CREATE -> Commit -> Verify persistence -> MATCH
   - Concurrent read test: multiple goroutines querying simultaneously
   - Error recovery test: invalid queries, recovery and continued operation
2. Create CI configuration:
   - GitHub Actions job `go-bindings`:
     - Build `libcypherlite_ffi.so` (cargo build --release -p cypherlite-ffi)
     - Set CGO_LDFLAGS and CGO_CFLAGS
     - Run `go test -race -v ./bindings/go/cypherlite/...`
     - Run `go vet ./bindings/go/cypherlite/...`
   - Add to existing `.github/workflows/ci.yml` as a new job depending on `test`
3. Create `README.md` in `bindings/go/cypherlite/`:
   - Build prerequisites (Rust toolchain, libcypherlite_ffi)
   - Installation instructions
   - Quick start example (Open, CREATE, MATCH, Close)
   - API reference summary
   - Platform-specific notes (Linux, macOS, Windows)
   - Static vs dynamic linking instructions
   - Build tag documentation (subgraph, hypergraph)
4. Create `example_test.go` with Example functions for GoDoc

**File Impact:**
- NEW: `bindings/go/cypherlite/integration_test.go`
- NEW: `bindings/go/cypherlite/example_test.go`
- NEW: `bindings/go/cypherlite/README.md`
- MODIFY: `.github/workflows/ci.yml` (add go-bindings job)

**Requirements Covered:** REQ-FFI-GO-TEST-001~006, REQ-FFI-GO-NFR-001~005

---

## 3. Risk Analysis

### R1: CGo Overhead and Thread Safety (High Risk)

**Description:** CGo calls have ~100ns overhead each and are not inlined by the Go compiler. Additionally, Go goroutines may migrate between OS threads, which conflicts with C thread-local error storage.

**Impact:** Performance degradation for high-frequency calls; incorrect error messages if thread migration occurs between error-producing call and error retrieval.

**Mitigation:**
- Wrap every error-producing CGo call with `runtime.LockOSThread()` / `runtime.UnlockOSThread()`
- Retrieve error message immediately after the C call, within the same locked block
- Document CGo overhead in README and benchmarks
- Batch column name retrieval to minimize CGo crossings in result access

### R2: CylValue Union Access from Go (High Risk)

**Description:** Go cannot directly access C union fields. The `CylValuePayload` union requires either unsafe pointer arithmetic or C accessor functions to extract typed values.

**Impact:** Incorrect offset calculations lead to memory corruption or wrong values.

**Mitigation:**
- Define C helper macros or inline functions in the CGo preamble to safely extract each union field
- Alternative: Use `unsafe.Pointer` with compile-time verified offsets
- Extensive round-trip tests for every value type
- Consider using `C.cyl_result_get` which returns `CylValue` by value -- verify layout compatibility

### R3: String Ownership and Lifetime (Medium Risk)

**Description:** C strings borrowed from `CylResult` (column names, string values) become invalid after `cyl_result_free`. If Go code retains references to these strings, use-after-free occurs.

**Impact:** Undefined behavior: crashes, corrupted data in Go strings.

**Mitigation:**
- Copy all C strings to Go strings (`C.GoString`) immediately at access time
- Never store raw `*C.char` pointers in Go structs
- Cache column names in Go memory on first `Columns()` call
- Document that `Row.Get()` always returns Go-owned data

### R4: Cross-Compilation Difficulty (Medium Risk)

**Description:** CGo complicates cross-compilation. Building for Linux from macOS requires a cross-compilation toolchain for both Go and the C library.

**Impact:** Users cannot easily build for target platforms different from their development machine.

**Mitigation:**
- Document platform requirements in README
- Provide pre-built binaries for major platforms (future)
- Support static linking to produce self-contained binaries
- Consider providing a Docker-based build environment

### R5: Go Finalizer Reliability (Low Risk)

**Description:** Go finalizers are not guaranteed to run. If the program exits before GC runs, finalizers may not fire, leading to unreleased C resources.

**Impact:** Resource leak on program exit (OS reclaims resources anyway, but intermediate resource exhaustion is possible).

**Mitigation:**
- Finalizers are a safety net, not the primary cleanup mechanism
- Document that users MUST call `Close()` explicitly
- Log warnings from finalizers to alert developers of missed `Close()` calls
- Provide `defer db.Close()` pattern in all examples

### R6: CylValue Layout Compatibility (Low Risk)

**Description:** The C `CylValue` struct is returned by value from several functions. CGo must correctly handle the struct layout including the union payload.

**Impact:** Misaligned or incorrect struct interpretation leads to corrupted values.

**Mitigation:**
- Verify `C.sizeof_CylValue` matches expected size at init time
- Use `C.CylValue` type directly (CGo generates correct struct definition from header)
- Test on all target platforms (32-bit alignment differences are unlikely for 64-bit targets but should be verified)

---

## 4. Dependencies

### Go Dependencies

| Dependency    | Version    | Purpose                              |
| ------------- | ---------- | ------------------------------------ |
| (stdlib only) | Go 1.21+   | runtime, unsafe, sync, errors, fmt   |

No external Go dependencies required. The binding uses only the standard library.

### Build Dependencies

| Dependency              | Purpose                              |
| ----------------------- | ------------------------------------ |
| `libcypherlite_ffi.so`  | Pre-built shared library (Linux)     |
| `libcypherlite_ffi.dylib` | Pre-built shared library (macOS)   |
| `libcypherlite_ffi.a`   | Pre-built static library (optional)  |
| `cypherlite.h`          | Generated C header from SPEC-FFI-001 |
| CGo toolchain           | C compiler (gcc/clang) for CGo build |

### CI Dependencies

| Tool          | Purpose                                  |
| ------------- | ---------------------------------------- |
| Go 1.21+      | Go toolchain for test and build          |
| Rust toolchain | Build libcypherlite_ffi in CI            |
| gcc/clang     | CGo C compiler                           |

---

## 5. Estimated File Impact

### New Files (Go bindings)

| File                            | Purpose                          | Est. Lines |
| ------------------------------- | -------------------------------- | ---------- |
| `go.mod`                        | Go module manifest               | 5          |
| `cgo_bridge.go`                 | CGo directives + C includes      | 40         |
| `cypherlite_static.go`          | Static linking build tag          | 15         |
| `cypherlite_dynamic.go`         | Dynamic linking build tag         | 15         |
| `types.go`                      | Go type definitions               | 60         |
| `types_subgraph.go`             | Subgraph types (build tag)        | 15         |
| `types_hypergraph.go`           | Hypergraph types (build tag)      | 25         |
| `errors.go`                     | Error types + sentinels           | 120        |
| `thread.go`                     | Thread locking helpers            | 30         |
| `db.go`                         | DB lifecycle (Open/Close)         | 120        |
| `query.go`                      | Query execution + params          | 180        |
| `tx.go`                         | Transaction support               | 150        |
| `result.go`                     | Result access                     | 120        |
| `row.go`                        | Row value access                  | 80         |
| `value.go`                      | CylValue -> Go conversion         | 150        |
| `value_subgraph.go`             | Subgraph value conversion         | 25         |
| `value_hypergraph.go`           | Hypergraph value conversion       | 40         |
| `version.go`                    | Version/Features wrappers         | 30         |
| `version_test.go`               | Version tests                     | 40         |
| `errors_test.go`                | Error handling tests              | 80         |
| `db_test.go`                    | DB lifecycle tests                | 120        |
| `query_test.go`                 | Query execution tests             | 150        |
| `tx_test.go`                    | Transaction tests                 | 150        |
| `result_test.go`                | Result access tests               | 120        |
| `value_test.go`                 | Value conversion tests            | 150        |
| `integration_test.go`           | Full lifecycle integration tests  | 200        |
| `example_test.go`               | GoDoc examples                    | 80         |
| `README.md`                     | Usage documentation               | 200        |

**Total New:** ~2,510 lines

### Modified Files

| File                         | Change                                |
| ---------------------------- | ------------------------------------- |
| `.github/workflows/ci.yml`  | Add `go-bindings` job                 |

---

## 6. Technical Approach

### Package Architecture

```
bindings/go/cypherlite/
  go.mod                  -- module declaration
  cgo_bridge.go           -- #cgo directives, C header inclusion
  cypherlite_static.go    -- //go:build cypherlite_static
  cypherlite_dynamic.go   -- //go:build !cypherlite_static (default)
  types.go                -- NodeID, EdgeID, DateTime, Value
  types_subgraph.go       -- //go:build subgraph
  types_hypergraph.go     -- //go:build hypergraph
  errors.go               -- Error type, sentinel errors
  thread.go               -- runtime.LockOSThread helpers
  db.go                   -- DB struct, Open, OpenWithConfig, Close
  query.go                -- Execute, ExecuteWithParams
  tx.go                   -- Tx struct, Begin, Commit, Rollback
  result.go               -- Result struct, Columns, RowCount, Close
  row.go                  -- Row struct, Get, GetByName
  value.go                -- cylValueToGo, goValueToCylValue
  value_subgraph.go       -- //go:build subgraph
  value_hypergraph.go     -- //go:build hypergraph
  version.go              -- Version(), Features()
  README.md               -- documentation
  *_test.go               -- test files
```

### Pattern: CGo Call with Thread-Local Error Retrieval

```go
func (db *DB) Execute(query string) (*Result, error) {
    db.mu.Lock()
    defer db.mu.Unlock()
    if db.ptr == nil {
        return nil, ErrClosed
    }

    cQuery := C.CString(query)
    defer C.free(unsafe.Pointer(cQuery))

    var errCode C.CylError

    runtime.LockOSThread()
    defer runtime.UnlockOSThread()

    result := C.cyl_db_execute(db.ptr, cQuery, &errCode)
    if errCode != C.CYL_OK {
        return nil, errorFromCode(errCode)
    }

    return newResult(result), nil
}
```

### Pattern: CylValue Union Access via C Helper

```c
// In CGo preamble:
static inline int64_t cyl_value_int64(CylValue v) {
    return v.payload.int64;
}
static inline const char* cyl_value_string(CylValue v) {
    return v.payload.string;
}
// ... one accessor per variant
```

### TDD Cycle Per Milestone

Following `quality.yaml` (development_mode: tdd):

1. **RED**: Write failing Go test for each exported function
2. **GREEN**: Implement the Go function with minimal CGo wrapping
3. **REFACTOR**: Extract common patterns (thread locking, error conversion, string conversion)
