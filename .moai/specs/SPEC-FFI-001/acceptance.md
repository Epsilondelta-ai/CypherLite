# SPEC-FFI-001: Acceptance Criteria

| Field     | Value                              |
| --------- | ---------------------------------- |
| SPEC      | SPEC-FFI-001                       |
| Title     | C ABI FFI Bindings - Acceptance    |
| Version   | 1.0.0                             |
| Created   | 2026-03-14                         |

---

## AC-SETUP: Crate Setup (M1)

### AC-SETUP-001: Workspace Integration

```gherkin
Given the CypherLite workspace
When cypherlite-ffi is added to workspace members
Then `cargo check -p cypherlite-ffi` succeeds
And `cargo check -p cypherlite-ffi --all-features` succeeds
And the crate produces both cdylib and staticlib outputs
```

**Traces:** REQ-FFI-C-001

### AC-SETUP-002: Feature Flag Propagation

```gherkin
Given cypherlite-ffi with feature "subgraph" enabled
When cargo check is run
Then all subgraph-gated FFI functions are compiled
And the subgraph-gated Value variants are available

Given cypherlite-ffi with default features only
When cargo check is run
Then subgraph-gated and hypergraph-gated FFI functions are not compiled
```

**Traces:** REQ-FFI-C-002

---

## AC-LIFECYCLE: Database Lifecycle (M2)

### AC-LIFECYCLE-001: Open and Close

```gherkin
Given a valid file system path
When cyl_db_open(path, &error) is called
Then a non-null CylDb pointer is returned
And error equals CYL_OK

When cyl_db_close(db) is called with the returned pointer
Then the database file is properly flushed
And the pointer becomes invalid (no crash on close)
```

**Traces:** REQ-FFI-C-005, REQ-FFI-C-007

### AC-LIFECYCLE-002: Open with Configuration

```gherkin
Given a valid file system path
When cyl_db_open_with_config(path, 4096, 1024, &error) is called
Then a non-null CylDb pointer is returned
And the database uses the specified page_size and cache_capacity
```

**Traces:** REQ-FFI-C-006

### AC-LIFECYCLE-003: Open Failure

```gherkin
Given an invalid file system path (e.g., directory does not exist)
When cyl_db_open(invalid_path, &error) is called
Then a null pointer is returned
And error equals CYL_ERR_IO
And cyl_last_error_message() returns a non-null descriptive string
```

**Traces:** REQ-FFI-C-005, REQ-FFI-C-034

### AC-LIFECYCLE-004: Null Pointer Safety on Close

```gherkin
Given a null CylDb pointer
When cyl_db_close(null) is called
Then no crash occurs (no-op)
```

**Traces:** REQ-FFI-C-008

### AC-LIFECYCLE-005: Reopen After Close

```gherkin
Given a database that was opened, had data written, and was closed
When cyl_db_open(same_path, &error) is called again
Then the previously written data is accessible via queries
```

**Traces:** REQ-FFI-C-005, REQ-FFI-C-007

---

## AC-QUERY: Query Execution (M3)

### AC-QUERY-001: Simple Query Execution

```gherkin
Given an open CylDb handle
When cyl_db_execute(db, "CREATE (n:Person {name: 'Alice'}) RETURN n", &error) is called
Then a non-null CylResult pointer is returned
And error equals CYL_OK
And cyl_result_row_count(result) returns 1
```

**Traces:** REQ-FFI-C-009

### AC-QUERY-002: Parameterized Query

```gherkin
Given an open CylDb handle
And a node has been created
When cyl_db_execute_with_params(db, "MATCH (n:Person {name: $name}) RETURN n.name",
     keys=["name"], values=[cyl_param_string("Alice")], count=1, &error) is called
Then a non-null CylResult pointer is returned
And the result contains the matching row with value "Alice"
```

**Traces:** REQ-FFI-C-010

### AC-QUERY-003: Invalid UTF-8 Query

```gherkin
Given an open CylDb handle
When cyl_db_execute(db, <invalid-utf8-bytes>, &error) is called
Then a null pointer is returned
And error equals CYL_ERR_INVALID_UTF8
And cyl_last_error_message() describes the UTF-8 validation failure
```

**Traces:** REQ-FFI-C-011

### AC-QUERY-004: Parse Error

```gherkin
Given an open CylDb handle
When cyl_db_execute(db, "NOT VALID CYPHER !!!", &error) is called
Then a null pointer is returned
And error equals CYL_ERR_PARSE
And cyl_last_error_message() contains line/column information
```

**Traces:** REQ-FFI-C-009, REQ-FFI-C-031

### AC-QUERY-005: Result Free Null Safety

```gherkin
Given a null CylResult pointer
When cyl_result_free(null) is called
Then no crash occurs (no-op)
```

**Traces:** REQ-FFI-C-022, REQ-FFI-TEST-004

---

## AC-TX: Transaction Support (M4)

### AC-TX-001: Begin and Commit

```gherkin
Given an open CylDb handle
When cyl_tx_begin(db, &error) is called
Then a non-null CylTx pointer is returned
And error equals CYL_OK

When cyl_tx_execute(tx, "CREATE (n:Person {name: 'Bob'})", &error) is called
Then error equals CYL_OK

When cyl_tx_commit(tx, &error) is called
Then error equals CYL_OK
And the created node persists in the database

When cyl_db_execute(db, "MATCH (n:Person {name: 'Bob'}) RETURN n", &error) is called
Then the result contains 1 row
```

**Traces:** REQ-FFI-C-012, REQ-FFI-C-013, REQ-FFI-C-015

### AC-TX-002: Begin and Rollback

```gherkin
Given an open CylDb handle
When cyl_tx_begin(db, &error) is called
And cyl_tx_execute(tx, "CREATE (n:Person {name: 'Charlie'})", &error) is called
And cyl_tx_rollback(tx) is called
Then the created node does NOT persist

When cyl_db_execute(db, "MATCH (n:Person {name: 'Charlie'}) RETURN n", &error) is called
Then the result contains 0 rows
```

**Traces:** REQ-FFI-C-013, REQ-FFI-C-016

### AC-TX-003: Auto-Rollback on Free

```gherkin
Given an open CylDb handle
When cyl_tx_begin(db, &error) is called
And cyl_tx_execute(tx, "CREATE (n:Person {name: 'Dave'})", &error) is called
And cyl_tx_free(tx) is called without committing
Then the created node does NOT persist (automatic rollback)
```

**Traces:** REQ-FFI-C-017

### AC-TX-004: Transaction Conflict

```gherkin
Given an open CylDb handle with an active transaction
When cyl_tx_begin(db, &error) is called for a second transaction
Then a null pointer is returned
And error equals CYL_ERR_TRANSACTION_CONFLICT
```

**Traces:** REQ-FFI-C-012

### AC-TX-005: Parameterized Query in Transaction

```gherkin
Given an active CylTx handle
When cyl_tx_execute_with_params(tx, "CREATE (n:Person {name: $name})",
     keys=["name"], values=[cyl_param_string("Eve")], count=1, &error) is called
And cyl_tx_commit(tx, &error) is called
Then the node exists in the database
```

**Traces:** REQ-FFI-C-014, REQ-FFI-C-015

### AC-TX-006: Null Pointer Safety on Transaction Free

```gherkin
Given a null CylTx pointer
When cyl_tx_free(null) is called
Then no crash occurs (no-op)
```

**Traces:** REQ-FFI-TEST-004

---

## AC-RESULT: Result Access (M5)

### AC-RESULT-001: Column Enumeration

```gherkin
Given a CylResult from "MATCH (n:Person) RETURN n.name, n.age"
When cyl_result_column_count(result) is called
Then it returns 2

When cyl_result_column_name(result, 0) is called
Then it returns "n.name"

When cyl_result_column_name(result, 1) is called
Then it returns "n.age"
```

**Traces:** REQ-FFI-C-018, REQ-FFI-C-019

### AC-RESULT-002: Column Name Out of Bounds

```gherkin
Given a CylResult with 2 columns
When cyl_result_column_name(result, 5) is called
Then null is returned (no crash)
```

**Traces:** REQ-FFI-C-019

### AC-RESULT-003: Row Iteration

```gherkin
Given a CylResult with 3 rows
When cyl_result_row_count(result) is called
Then it returns 3

When cyl_result_row(result, 0) through cyl_result_row(result, 2) are called
Then each returns a non-null CylRow pointer

When cyl_result_row(result, 3) is called
Then null is returned (out of bounds)
```

**Traces:** REQ-FFI-C-020, REQ-FFI-C-021

### AC-RESULT-004: Empty Result Set

```gherkin
Given a CylResult from a query that matches nothing
When cyl_result_row_count(result) is called
Then it returns 0

When cyl_result_column_count(result) is called
Then it returns the number of projected columns (may be > 0)
```

**Traces:** REQ-FFI-C-018, REQ-FFI-C-020

---

## AC-VALUE: Value Type System (M6)

### AC-VALUE-001: Null Value

```gherkin
Given a row with a NULL value at column 0
When cyl_row_get(row, 0) is called
Then the returned CylValue has tag CYL_VALUE_NULL (0)
```

**Traces:** REQ-FFI-C-023, REQ-FFI-C-026

### AC-VALUE-002: Bool Value

```gherkin
Given a row with a boolean true value at column 0
When cyl_row_get(row, 0) is called
Then the returned CylValue has tag CYL_VALUE_BOOL (1)
And the payload bool_val equals 1
```

**Traces:** REQ-FFI-C-023, REQ-FFI-C-026

### AC-VALUE-003: Int64 Value

```gherkin
Given a row with integer value 42 at column 0
When cyl_row_get(row, 0) is called
Then the returned CylValue has tag CYL_VALUE_INT64 (2)
And the payload int64_val equals 42
```

**Traces:** REQ-FFI-C-023, REQ-FFI-C-026

### AC-VALUE-004: Float64 Value

```gherkin
Given a row with float value 3.14 at column 0
When cyl_row_get(row, 0) is called
Then the returned CylValue has tag CYL_VALUE_FLOAT64 (3)
And the payload float64_val equals 3.14
```

**Traces:** REQ-FFI-C-023, REQ-FFI-C-026

### AC-VALUE-005: String Value

```gherkin
Given a row with string value "hello" at column "name"
When cyl_row_get_by_name(row, "name") is called
Then the returned CylValue has tag CYL_VALUE_STRING (4)
And the payload string_val points to a null-terminated "hello" string
```

**Traces:** REQ-FFI-C-024, REQ-FFI-C-026

### AC-VALUE-006: Bytes Value

```gherkin
Given a row with byte array [0xDE, 0xAD, 0xBE, 0xEF] at column 0
When cyl_row_get(row, 0) is called
Then the returned CylValue has tag CYL_VALUE_BYTES (5)
And the payload bytes_val.data points to the 4 bytes
And the payload bytes_val.len equals 4
```

**Traces:** REQ-FFI-C-023, REQ-FFI-C-026

### AC-VALUE-007: List Value

```gherkin
Given a row with a list [1, 2, 3] at column 0
When cyl_row_get(row, 0) is called
Then the returned CylValue has tag CYL_VALUE_LIST (6)
And the payload list_val.items contains 3 CylValue elements
And each element has tag CYL_VALUE_INT64 with values 1, 2, 3
```

**Traces:** REQ-FFI-C-023, REQ-FFI-C-026

### AC-VALUE-008: Node and Edge IDs

```gherkin
Given a row with a Node value (id=42) at column 0
When cyl_row_get(row, 0) is called
Then the returned CylValue has tag CYL_VALUE_NODE (7)
And the payload uint64_val equals 42

Given a row with an Edge value (id=99) at column 1
When cyl_row_get(row, 1) is called
Then the returned CylValue has tag CYL_VALUE_EDGE (8)
And the payload uint64_val equals 99
```

**Traces:** REQ-FFI-C-023, REQ-FFI-C-026

### AC-VALUE-009: DateTime Value

```gherkin
Given a row with a DateTime value (epoch ms = 1700000000000) at column 0
When cyl_row_get(row, 0) is called
Then the returned CylValue has tag CYL_VALUE_DATETIME (9)
And the payload int64_val equals 1700000000000
```

**Traces:** REQ-FFI-C-023, REQ-FFI-C-026

### AC-VALUE-010: Get By Name — Missing Column

```gherkin
Given a row without a column named "nonexistent"
When cyl_row_get_by_name(row, "nonexistent") is called
Then the returned CylValue has tag CYL_VALUE_NULL
```

**Traces:** REQ-FFI-C-024

### AC-VALUE-011: Parameter Constructors

```gherkin
Given no precondition
When cyl_param_null() is called
Then the returned CylValue has tag CYL_VALUE_NULL

When cyl_param_bool(1) is called
Then the returned CylValue has tag CYL_VALUE_BOOL with payload 1

When cyl_param_int64(-100) is called
Then the returned CylValue has tag CYL_VALUE_INT64 with payload -100

When cyl_param_float64(2.718) is called
Then the returned CylValue has tag CYL_VALUE_FLOAT64 with payload 2.718

When cyl_param_string("test") is called
Then the returned CylValue has tag CYL_VALUE_STRING
And the string is a copy (caller owns it, must free)

When cyl_param_bytes([0x01, 0x02], 2) is called
Then the returned CylValue has tag CYL_VALUE_BYTES with len 2
And the bytes are a copy (caller owns it, must free)
```

**Traces:** REQ-FFI-C-030

### AC-VALUE-012: Feature-Gated Subgraph Value

```gherkin
Given the "subgraph" feature is enabled
And a row with a Subgraph value (id=7) at column 0
When cyl_row_get(row, 0) is called
Then the returned CylValue has tag CYL_VALUE_SUBGRAPH (10)
And the payload uint64_val equals 7
```

**Traces:** REQ-FFI-C-027

### AC-VALUE-013: Feature-Gated Hyperedge Value

```gherkin
Given the "hypergraph" feature is enabled
And a row with a Hyperedge value (id=15) at column 0
When cyl_row_get(row, 0) is called
Then the returned CylValue has tag CYL_VALUE_HYPEREDGE (11)
And the payload uint64_val equals 15
```

**Traces:** REQ-FFI-C-028

---

## AC-ERROR: Error Handling (M2)

### AC-ERROR-001: Error Code Mapping

```gherkin
Given each CypherLiteError variant
When the error is converted to CylError via the FFI layer
Then the correct CylError integer code is produced:
  | CypherLiteError variant    | Expected CylError code           |
  | IoError                    | CYL_ERR_IO (1)                   |
  | CorruptedPage              | CYL_ERR_CORRUPTED_PAGE (2)       |
  | TransactionConflict        | CYL_ERR_TRANSACTION_CONFLICT (3) |
  | OutOfSpace                 | CYL_ERR_OUT_OF_SPACE (4)         |
  | InvalidMagicNumber         | CYL_ERR_INVALID_MAGIC (5)        |
  | UnsupportedVersion         | CYL_ERR_UNSUPPORTED_VERSION (6)  |
  | ChecksumMismatch           | CYL_ERR_CHECKSUM (7)             |
  | SerializationError         | CYL_ERR_SERIALIZATION (8)        |
  | NodeNotFound               | CYL_ERR_NODE_NOT_FOUND (9)       |
  | EdgeNotFound               | CYL_ERR_EDGE_NOT_FOUND (10)      |
  | ParseError                 | CYL_ERR_PARSE (11)               |
  | SemanticError              | CYL_ERR_SEMANTIC (12)            |
  | ExecutionError             | CYL_ERR_EXECUTION (13)           |
  | UnsupportedSyntax          | CYL_ERR_UNSUPPORTED_SYNTAX (14)  |
  | ConstraintViolation        | CYL_ERR_CONSTRAINT_VIOLATION (15)|
  | InvalidDateTimeFormat      | CYL_ERR_INVALID_DATETIME (16)    |
  | SystemPropertyReadOnly     | CYL_ERR_SYSTEM_PROPERTY (17)     |
  | FeatureIncompatible        | CYL_ERR_FEATURE_INCOMPATIBLE (18)|
```

**Traces:** REQ-FFI-C-031

### AC-ERROR-002: Thread-Local Error Message

```gherkin
Given a failed FFI call that produced CYL_ERR_PARSE
When cyl_last_error_message() is called on the same thread
Then a non-null pointer to a descriptive null-terminated UTF-8 string is returned
And the string contains relevant error details

When cyl_last_error_message() is called on a different thread (that had no error)
Then null is returned (thread-local isolation)
```

**Traces:** REQ-FFI-C-034, REQ-FFI-C-035, REQ-FFI-C-040

### AC-ERROR-003: No Error State

```gherkin
Given a successful FFI call
When cyl_last_error_message() is called
Then null is returned
```

**Traces:** REQ-FFI-C-035

### AC-ERROR-004: Feature-Gated Error Codes

```gherkin
Given the "subgraph" feature is enabled
When a SubgraphNotFound error occurs
Then error equals CYL_ERR_SUBGRAPH_NOT_FOUND (100)

Given the "hypergraph" feature is enabled
When a HyperEdgeNotFound error occurs
Then error equals CYL_ERR_HYPEREDGE_NOT_FOUND (200)
```

**Traces:** REQ-FFI-C-032, REQ-FFI-C-033

---

## AC-THREAD: Thread Safety (M2)

### AC-THREAD-001: Concurrent Read Access

```gherkin
Given an open CylDb handle with existing data
When 4 threads concurrently call cyl_db_execute with read-only MATCH queries
Then all threads receive correct results
And no data corruption or crash occurs
```

**Traces:** REQ-FFI-C-038

### AC-THREAD-002: Transaction Not Shareable

```gherkin
Given a CylTx handle created on thread A
When documentation is reviewed
Then the header clearly states CylTx must not be shared between threads
```

**Traces:** REQ-FFI-C-039

---

## AC-HEADER: C Header Generation (M7)

### AC-HEADER-001: Header Compilation (gcc)

```gherkin
Given the generated include/cypherlite.h
When compiled with `gcc -std=c11 -Wall -Werror -c -x c -`
Then compilation succeeds with zero warnings
```

**Traces:** REQ-FFI-C-041, REQ-FFI-NFR-004

### AC-HEADER-002: Header Compilation (clang)

```gherkin
Given the generated include/cypherlite.h
When compiled with `clang -std=c11 -Wall -Werror -c -x c -`
Then compilation succeeds with zero warnings
```

**Traces:** REQ-FFI-C-041, REQ-FFI-NFR-004

### AC-HEADER-003: Header Content

```gherkin
Given the generated include/cypherlite.h
Then it contains:
  - Opaque type declarations (typedef struct CylDb CylDb; etc.)
  - CylValue tagged union definition with #[repr(C)] layout
  - CylError enum constants
  - CYL_VALUE_* tag constants
  - All function declarations
  - #ifdef __cplusplus extern "C" guards
  - Include guards (#ifndef CYPHERLITE_H / #define CYPHERLITE_H)
```

**Traces:** REQ-FFI-C-042

### AC-HEADER-004: Reproducible Generation

```gherkin
Given the cbindgen.toml in the repository
When `cbindgen --config cbindgen.toml --crate cypherlite-ffi --output include/cypherlite.h` is run twice
Then both outputs are identical (byte-for-byte)
```

**Traces:** REQ-FFI-C-043

---

## AC-FEATURE: Feature Flag Support (M8)

### AC-FEATURE-001: Conditional Compilation

```gherkin
Given cypherlite-ffi compiled without "subgraph" feature
When the compiled library symbols are inspected
Then no subgraph-related functions exist in the binary

Given cypherlite-ffi compiled with "subgraph" feature
When the compiled library symbols are inspected
Then subgraph-related functions are present
```

**Traces:** REQ-FFI-C-044

### AC-FEATURE-002: Version and Features API

```gherkin
Given cypherlite-ffi compiled with features "temporal-core,subgraph"
When cyl_version() is called
Then it returns a null-terminated string matching the crate version (e.g., "0.12.0")

When cyl_features() is called
Then it returns a null-terminated comma-separated string containing "temporal-core,subgraph"
```

**Traces:** REQ-FFI-C-045

---

## AC-SAFETY: Memory Safety (M7)

### AC-SAFETY-001: No Memory Leaks (Valgrind)

```gherkin
Given the C test program (tests/ffi_test.c)
When run under Valgrind with --leak-check=full
Then zero memory leaks are reported
And zero invalid memory accesses are reported
```

**Traces:** REQ-FFI-TEST-003

### AC-SAFETY-002: No Undefined Behavior (Miri)

```gherkin
Given the Rust FFI integration tests
When run with `cargo +nightly miri test -p cypherlite-ffi`
Then zero undefined behavior is detected
```

**Traces:** REQ-FFI-TEST-003

### AC-SAFETY-003: Null Pointer Inputs

```gherkin
Given each FFI function that accepts pointer arguments
When called with null pointers for each argument individually
Then the function returns an appropriate error code (CYL_ERR_NULL_POINTER)
And no segfault or crash occurs
```

**Traces:** REQ-FFI-TEST-007

### AC-SAFETY-004: Double-Free Documentation

```gherkin
Given the generated C header
When the documentation for cyl_*_free functions is reviewed
Then each clearly states "Calling this function twice on the same pointer is undefined behavior"
```

**Traces:** REQ-FFI-TEST-005

---

## AC-C-TEST: C Test Program (M7)

### AC-C-TEST-001: Full Lifecycle in C

```gherkin
Given the compiled C test program
When executed
Then it successfully:
  1. Opens a database with cyl_db_open
  2. Creates nodes with cyl_db_execute
  3. Queries nodes and iterates results
  4. Reads string, int64, and float64 values from rows
  5. Begins a transaction with cyl_tx_begin
  6. Executes within the transaction
  7. Commits the transaction
  8. Verifies committed data persists
  9. Begins another transaction and rolls it back
  10. Verifies rolled-back data does not persist
  11. Tests error handling (invalid query, transaction conflict)
  12. Frees all resources
  13. Closes the database
And the exit code is 0
```

**Traces:** REQ-FFI-TEST-001, REQ-FFI-TEST-002

---

## AC-DOC: Documentation (M8)

### AC-DOC-001: Header Documentation

```gherkin
Given the generated include/cypherlite.h
Then every public function has a doc comment describing:
  - Purpose
  - Parameters (with ownership semantics: borrowed vs. owned)
  - Return value (with null conditions)
  - Thread safety
  - Error conditions
```

**Traces:** REQ-FFI-DOC-001

### AC-DOC-002: README Content

```gherkin
Given crates/cypherlite-ffi/README.md
Then it contains:
  - Build instructions for shared and static libraries
  - Platform-specific linking flags (Linux: -lcypherlite_ffi, macOS: -L... -lcypherlite_ffi, Windows: cypherlite_ffi.lib)
  - A minimal C usage example demonstrating open/query/iterate/close
  - Feature flag build options
```

**Traces:** REQ-FFI-DOC-002

---

## AC-NFR: Non-Functional Requirements

### AC-NFR-001: FFI Overhead

```gherkin
Given a benchmark measuring FFI call overhead
When cyl_db_execute is called with a trivial no-op query
Then the FFI layer overhead (excluding actual query execution) is < 1 microsecond
```

**Traces:** REQ-FFI-NFR-001

### AC-NFR-002: Safety Comments

```gherkin
Given the cypherlite-ffi source code
When all `unsafe` blocks are reviewed
Then each has a `// SAFETY:` comment explaining the invariants
```

**Traces:** REQ-FFI-NFR-003

### AC-NFR-003: Coverage Target

```gherkin
Given the complete cypherlite-ffi crate
When `cargo llvm-cov` is run
Then code coverage is >= 85%
```

**Traces:** quality.yaml test_coverage_target

---

## Definition of Done

- [ ] All AC-SETUP criteria pass
- [ ] All AC-LIFECYCLE criteria pass
- [ ] All AC-QUERY criteria pass
- [ ] All AC-TX criteria pass
- [ ] All AC-RESULT criteria pass
- [ ] All AC-VALUE criteria pass (including feature-gated variants)
- [ ] All AC-ERROR criteria pass (including feature-gated codes)
- [ ] All AC-THREAD criteria pass
- [ ] All AC-HEADER criteria pass (gcc + clang compilation)
- [ ] All AC-FEATURE criteria pass
- [ ] All AC-SAFETY criteria pass (Valgrind + Miri + null inputs)
- [ ] All AC-C-TEST criteria pass
- [ ] All AC-DOC criteria pass
- [ ] All AC-NFR criteria pass (overhead, safety comments, coverage)
- [ ] CI pipeline passes with cypherlite-ffi in all jobs
- [ ] `cbindgen` generates clean header
- [ ] `cargo clippy -p cypherlite-ffi -- -D warnings` passes
- [ ] `cargo fmt -p cypherlite-ffi -- --check` passes
