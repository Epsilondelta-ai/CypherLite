# SPEC-FFI-001: Implementation Plan

| Field     | Value                          |
| --------- | ------------------------------ |
| SPEC      | SPEC-FFI-001                   |
| Title     | C ABI FFI Bindings - Plan      |
| Version   | 1.0.0                          |
| Created   | 2026-03-14                     |

---

## 1. Milestone Overview

```
M1 ──> M2 ──> M3 ──> M4 ──> M5 ──> M6 ──> M7 ──> M8
 │      │      │      │      │      │      │      │
Setup  Life  Query   Tx   Result  Value  Header  Docs
       cycle               Access  Types  + C Test + Feature
```

**Dependency Graph:**
- M1: No dependencies (crate bootstrap)
- M2: Depends on M1 (needs crate structure)
- M3: Depends on M2 (needs db handle + error handling)
- M4: Depends on M2 (needs db handle + error handling)
- M5: Depends on M3 (needs query execution to produce results)
- M6: Depends on M5 (needs result access to return values)
- M7: Depends on M6 (needs all FFI functions for header + C test)
- M8: Depends on M7 (needs header for documentation)

Note: M3 and M4 are independent of each other and may be developed in parallel after M2.

---

## 2. Milestones

### M1: Crate Setup + cbindgen Configuration + CI Integration

**Priority**: Primary Goal

**Tasks:**
1. Create `crates/cypherlite-ffi/` directory structure
2. Create `Cargo.toml` with `crate-type = ["cdylib", "staticlib"]`
3. Add `cypherlite-ffi` to workspace `Cargo.toml` members
4. Configure feature flags mirroring `cypherlite-query` features
5. Create `cbindgen.toml` with C11 output, `CYL_` prefix configuration
6. Create `src/lib.rs` with module structure (skeleton)
7. Create `include/` directory for generated header
8. Add `cypherlite-ffi` to CI check/test/coverage jobs
9. Write initial test verifying crate compiles with all feature combinations

**File Impact:**
- NEW: `crates/cypherlite-ffi/Cargo.toml`
- NEW: `crates/cypherlite-ffi/src/lib.rs`
- NEW: `crates/cypherlite-ffi/cbindgen.toml`
- NEW: `crates/cypherlite-ffi/include/.gitkeep`
- MODIFY: `Cargo.toml` (workspace members)
- MODIFY: `.github/workflows/ci.yml` (add ffi crate to CI matrix)

**Requirements Covered:** REQ-FFI-C-001, REQ-FFI-C-002

---

### M2: Core Lifecycle + Error Handling

**Priority**: Primary Goal

**Tasks:**
1. Define `CylError` enum as `#[repr(i32)]` with all error code constants
2. Implement `CypherLiteError -> CylError` conversion function
3. Implement thread-local error message buffer using `std::cell::RefCell<Option<CString>>`
4. Implement `cyl_last_error_message()` function
5. Define opaque `CylDb` wrapper struct
6. Implement `cyl_db_open(path, error_out)` with UTF-8 validation
7. Implement `cyl_db_open_with_config(path, page_size, cache_capacity, error_out)`
8. Implement `cyl_db_close(db)` with null-pointer safety
9. Write tests: open/close lifecycle, null-pointer safety, error code mapping for each `CypherLiteError` variant, thread-local error messages

**File Impact:**
- NEW: `crates/cypherlite-ffi/src/error.rs`
- NEW: `crates/cypherlite-ffi/src/db.rs`
- MODIFY: `crates/cypherlite-ffi/src/lib.rs` (module declarations, re-exports)

**Requirements Covered:** REQ-FFI-C-003~008, REQ-FFI-C-031~035, REQ-FFI-C-036~037, REQ-FFI-C-038~040

---

### M3: Query Execution

**Priority**: Primary Goal

**Tasks:**
1. Implement `CylResult` opaque wrapper for `QueryResult`
2. Implement `cyl_db_execute(db, query, error_out)`
3. Implement `cyl_db_execute_with_params(db, query, keys, values, count, error_out)`
4. Implement `cyl_result_free(result)` with null safety
5. Handle UTF-8 validation on query strings and parameter keys
6. Write tests: simple query, parameterized query, invalid UTF-8, query parse errors, execution errors

**File Impact:**
- NEW: `crates/cypherlite-ffi/src/query.rs`
- MODIFY: `crates/cypherlite-ffi/src/lib.rs`

**Requirements Covered:** REQ-FFI-C-009~011, REQ-FFI-C-022

---

### M4: Transaction Support

**Priority**: Primary Goal

**Tasks:**
1. Design `CylTx` owning wrapper that safely holds a mutable reference to `CypherLite`
   - Strategy: `CylTx` takes ownership of `Box<CypherLite>` from `CylDb`, returning it on commit/rollback/free
   - Alternative: Use `Pin<Box<...>>` with unsafe lifetime extension — evaluate trade-offs
2. Implement `cyl_tx_begin(db, error_out)` — takes `CylDb` reference, returns `CylTx`
3. Implement `cyl_tx_execute(tx, query, error_out)`
4. Implement `cyl_tx_execute_with_params(tx, query, keys, values, count, error_out)`
5. Implement `cyl_tx_commit(tx, error_out)` — commits and frees
6. Implement `cyl_tx_rollback(tx)` — rollbacks and frees
7. Implement `cyl_tx_free(tx)` — auto-rollback if uncommitted, then free
8. Write tests: begin/commit, begin/rollback, begin/free (auto-rollback), transaction conflict, execute within transaction

**Design Decision — Transaction Ownership:**
The Rust `Transaction<'a>` borrows `&'a mut CypherLite`. In C FFI, we cannot express lifetimes. Two approaches:

- **Option A (Recommended):** `CylTx` stores a raw pointer to `CypherLite` obtained from `CylDb`, with `CylDb` flagged as "in-transaction" to prevent double-borrow. `cyl_tx_commit/rollback/free` clears the flag.
- **Option B:** Move `CypherLite` out of `CylDb` into `CylTx`, making `CylDb` temporarily unusable. Restore on commit/rollback.

Recommendation: Option A (simpler, matches user expectations that db handle remains valid during transaction).

**File Impact:**
- NEW: `crates/cypherlite-ffi/src/transaction.rs`
- MODIFY: `crates/cypherlite-ffi/src/db.rs` (add in-transaction flag)
- MODIFY: `crates/cypherlite-ffi/src/lib.rs`

**Requirements Covered:** REQ-FFI-C-012~017

---

### M5: Result Access

**Priority**: Secondary Goal

**Tasks:**
1. Implement `cyl_result_column_count(result)` -> `uint32_t`
2. Implement `cyl_result_column_name(result, index)` -> `*const c_char`
3. Implement `cyl_result_row_count(result)` -> `uint64_t`
4. Implement `cyl_result_row(result, index)` -> `*const CylRow`
5. Ensure `CylRow` borrows from `CylResult` (no separate free needed)
6. Write tests: column enumeration, row iteration, out-of-bounds access, empty results

**File Impact:**
- NEW: `crates/cypherlite-ffi/src/result.rs`
- MODIFY: `crates/cypherlite-ffi/src/lib.rs`

**Requirements Covered:** REQ-FFI-C-018~021

---

### M6: Value Type System

**Priority**: Secondary Goal

**Tasks:**
1. Define `CylValueTag` constants (`CYL_VALUE_NULL` through `CYL_VALUE_DATETIME`)
2. Define `CylValue` C-compatible struct with `#[repr(C)]` tag + union
3. Implement `Value -> CylValue` conversion for each variant
4. Implement `CylValue -> Value` conversion (for parameters)
5. Implement `cyl_row_get(row, column_index)` -> `CylValue`
6. Implement `cyl_row_get_by_name(row, column_name)` -> `CylValue`
7. Implement `cyl_value_free(value)` for heap-allocated variants
8. Implement parameter constructors: `cyl_param_null/bool/int64/float64/string/bytes`
9. Feature-gated: add Subgraph, Hyperedge, TemporalNode variants
10. Write tests: each value type round-trip (Rust -> C -> Rust), null handling, string/bytes ownership, list values, feature-gated variants

**File Impact:**
- NEW: `crates/cypherlite-ffi/src/value.rs`
- MODIFY: `crates/cypherlite-ffi/src/result.rs` (row_get functions)
- MODIFY: `crates/cypherlite-ffi/src/lib.rs`

**Requirements Covered:** REQ-FFI-C-023~030, REQ-FFI-C-025~029

---

### M7: C Header Generation + C Test Program

**Priority**: Secondary Goal

**Tasks:**
1. Finalize `cbindgen.toml` configuration:
   - Language: C
   - Style: Both (tag + typedef)
   - Include guards and `extern "C"` wrappers
   - Documentation passthrough
   - Feature flag conditional compilation (`#ifdef CYL_FEATURE_SUBGRAPH`)
2. Run `cbindgen` and verify generated `include/cypherlite.h`
3. Write `tests/ffi_test.c`:
   - Open/close database
   - Execute CREATE/MATCH queries
   - Iterate results and read values
   - Transaction begin/commit/rollback
   - Error handling
4. Create build script or Makefile for compiling C test
5. Add C test compilation + execution to CI
6. Validate header compiles cleanly with `gcc -std=c11 -Wall -Werror` and `clang -std=c11 -Wall -Werror`
7. Write Miri test configuration for Rust FFI tests
8. Add Valgrind/ASan CI step for C test program

**File Impact:**
- NEW: `crates/cypherlite-ffi/tests/ffi_test.c`
- NEW: `crates/cypherlite-ffi/tests/build_and_run.sh`
- MODIFY: `crates/cypherlite-ffi/cbindgen.toml` (finalize)
- MODIFY: `.github/workflows/ci.yml` (add C test job, Miri, Valgrind)
- GENERATE: `crates/cypherlite-ffi/include/cypherlite.h`

**Requirements Covered:** REQ-FFI-C-041~044, REQ-FFI-TEST-001~007

---

### M8: Feature Flags + Version API + Documentation

**Priority**: Final Goal

**Tasks:**
1. Implement `cyl_version()` -> `*const c_char` (returns crate version)
2. Implement `cyl_features()` -> `*const c_char` (returns comma-separated feature list)
3. Verify all feature-flag combinations compile and pass tests:
   - Default features only
   - `subgraph` enabled
   - `hypergraph` enabled
   - `full-temporal` enabled
   - `plugin` (should have no FFI impact in this SPEC)
4. Write crate `README.md` with build instructions and C usage example
5. Add doc comments to all `#[no_mangle] extern "C"` functions
6. Verify header generation with all feature combinations
7. Final coverage check (target: 85%+)

**File Impact:**
- NEW: `crates/cypherlite-ffi/README.md`
- MODIFY: `crates/cypherlite-ffi/src/lib.rs` (version/features functions)
- MODIFY: `crates/cypherlite-ffi/src/*.rs` (doc comments)

**Requirements Covered:** REQ-FFI-C-045, REQ-FFI-DOC-001~002, REQ-FFI-NFR-004~005

---

## 3. Risk Analysis

### R1: Transaction Lifetime Safety (High Risk)

**Description:** Rust's `Transaction<'a>` type borrows `&'a mut CypherLite`. Expressing this ownership relationship safely in C FFI without lifetimes is the primary technical challenge.

**Impact:** Incorrect implementation leads to use-after-free or data corruption.

**Mitigation:**
- Use the in-transaction flag approach (Option A from M4)
- Document that `CylDb` must not be used for direct queries while a `CylTx` is active
- Add runtime checks: if `cyl_db_execute` is called while in-transaction, return `CYL_ERR_TRANSACTION_CONFLICT`
- Extensive Miri testing for all transaction lifecycle paths

### R2: Memory Leaks in Value Conversions (Medium Risk)

**Description:** Converting `Value::String`, `Value::Bytes`, and `Value::List` to C-compatible forms requires heap allocation. If callers do not call the appropriate free functions, memory leaks occur.

**Impact:** Slow memory leak in long-running applications.

**Mitigation:**
- Clear ownership documentation in header comments
- Row values are borrowed (no free needed) vs. parameter values are owned (must free)
- C test program validates no leaks via Valgrind
- Consider providing a `cyl_result_get_string(result, row, col, buf, buf_len)` copy-into-buffer pattern as an alternative

### R3: ABI Stability (Medium Risk)

**Description:** Adding new `Value` variants or error codes in future versions could break ABI compatibility.

**Impact:** Downstream consumers may crash or misinterpret data after library upgrade.

**Mitigation:**
- Reserve tag value ranges: core 0-9, subgraph 10-19, hypergraph 20-29, future 30+
- Reserve error code ranges: core 0-99, subgraph 100-199, hypergraph 200-299
- Use `#[repr(C)]` consistently for all public types
- Document ABI stability policy in README

### R4: cbindgen Limitations (Low Risk)

**Description:** cbindgen may not handle all Rust patterns (e.g., complex generics, conditional compilation).

**Impact:** Manual header maintenance may be required.

**Mitigation:**
- Keep FFI function signatures simple (`extern "C"` with primitive types and raw pointers)
- Avoid generics in FFI-exposed types
- Test header generation in CI to catch regressions
- Maintain a manual header as fallback if cbindgen proves insufficient

### R5: Thread Safety Documentation Mismatch (Low Risk)

**Description:** The internal `RwLock` allows concurrent reads, but the C API documentation must clearly communicate the threading model.

**Impact:** Callers may incorrectly assume full thread safety and encounter undefined behavior.

**Mitigation:**
- Header doc comments explicitly state: "CylDb is thread-safe for concurrent reads. Writes require external synchronization."
- CylTx doc comments explicitly state: "CylTx must not be shared between threads."
- Thread safety integration tests with concurrent reads

---

## 4. Dependencies

| Dependency    | Version    | Purpose                              |
| ------------- | ---------- | ------------------------------------ |
| cbindgen      | latest     | C header generation from Rust source |
| libc          | 0.2        | C type definitions for FFI           |
| cypherlite-query | workspace | Core database functionality          |
| cypherlite-core  | workspace | Error types, identifiers, config     |

**Dev Dependencies:**
| Dependency    | Version    | Purpose                              |
| ------------- | ---------- | ------------------------------------ |
| tempfile      | 3          | Temporary database files for tests   |

**CI Dependencies:**
| Tool          | Purpose                                  |
| ------------- | ---------------------------------------- |
| gcc/clang     | C test program compilation               |
| Valgrind      | Memory leak detection (Linux CI only)    |
| Miri          | Rust-side undefined behavior detection   |

---

## 5. Estimated File Impact

### New Files (cypherlite-ffi crate)

| File                        | Purpose                          | Est. Lines |
| --------------------------- | -------------------------------- | ---------- |
| `Cargo.toml`                | Crate manifest                   | 40         |
| `cbindgen.toml`             | Header generation config         | 50         |
| `src/lib.rs`                | Module declarations, re-exports  | 80         |
| `src/error.rs`              | Error codes, thread-local msgs   | 200        |
| `src/db.rs`                 | Database lifecycle FFI           | 150        |
| `src/query.rs`              | Query execution FFI              | 150        |
| `src/transaction.rs`        | Transaction FFI                  | 200        |
| `src/result.rs`             | Result/Row access FFI            | 200        |
| `src/value.rs`              | Value type system FFI            | 350        |
| `include/cypherlite.h`      | Generated C header               | 400+       |
| `tests/ffi_test.c`          | C integration test               | 300        |
| `tests/build_and_run.sh`    | C test build script              | 30         |
| `README.md`                 | Usage documentation              | 150        |

**Total New:** ~2,300 lines

### Modified Files

| File                         | Change                                |
| ---------------------------- | ------------------------------------- |
| `Cargo.toml` (workspace)    | Add `cypherlite-ffi` to members       |
| `.github/workflows/ci.yml`  | Add FFI crate to test matrix, C test job, Miri |

---

## 6. Technical Approach

### Module Architecture

```
cypherlite-ffi/
  src/
    lib.rs          -- #[no_mangle] re-exports, crate-level docs
    error.rs        -- CylError enum, error_to_code(), thread-local message
    db.rs           -- CylDb, cyl_db_open/close/open_with_config
    query.rs        -- cyl_db_execute, cyl_db_execute_with_params
    transaction.rs  -- CylTx, cyl_tx_begin/execute/commit/rollback/free
    result.rs       -- CylResult, CylRow, column/row access
    value.rs        -- CylValue tagged union, CylValueTag, param constructors
  include/
    cypherlite.h    -- Generated by cbindgen
  tests/
    ffi_test.c      -- C integration test
    build_and_run.sh
  cbindgen.toml
  Cargo.toml
  README.md
```

### Pattern: Error Handling

Every FFI function follows this pattern:

```rust
#[no_mangle]
pub unsafe extern "C" fn cyl_db_execute(
    db: *mut CylDb,
    query: *const c_char,
    error_out: *mut CylError,
) -> *mut CylResult {
    // 1. Null-check all pointer args
    // 2. Convert C string to &str (validate UTF-8)
    // 3. Call Rust API
    // 4. On success: return allocated result
    // 5. On error: set *error_out, set thread-local message, return null
}
```

### Pattern: Null-Safe Free

```rust
#[no_mangle]
pub unsafe extern "C" fn cyl_db_close(db: *mut CylDb) {
    if !db.is_null() {
        drop(Box::from_raw(db));
    }
}
```

### TDD Cycle Per Milestone

Following `quality.yaml` (development_mode: tdd):

1. **RED**: Write failing test for each FFI function (Rust integration test calling `extern "C"`)
2. **GREEN**: Implement the FFI function with minimal code to pass
3. **REFACTOR**: Clean up, extract patterns, add doc comments
