# SPEC-FFI-002: Acceptance Criteria

| Field     | Value                              |
| --------- | ---------------------------------- |
| SPEC      | SPEC-FFI-002                       |
| Title     | Go Bindings via CGo - Acceptance   |
| Version   | 1.0.0                              |
| Created   | 2026-03-15                         |

---

## AC-MODULE: Go Module Setup

### AC-MODULE-001: Go Module Initialization
```gherkin
Given the CypherLite repository with cypherlite-ffi crate built
When I run "go mod tidy" in bindings/go/cypherlite/
Then go.mod exists with module "github.com/Epsilondelta-ai/cypherlite-go"
And the minimum Go version is 1.21
And no external dependencies are declared
```

### AC-MODULE-002: CGo Bridge Compilation
```gherkin
Given the Go module is initialized
And libcypherlite_ffi is built (cargo build --release -p cypherlite-ffi)
And cypherlite.h exists in crates/cypherlite-ffi/include/
When I run "go build ./bindings/go/cypherlite/"
Then the package compiles without errors
And CGo resolves all symbols from libcypherlite_ffi
```

### AC-MODULE-003: Package Namespace
```gherkin
Given the Go package compiles
When I import "github.com/Epsilondelta-ai/cypherlite-go"
Then all public types (DB, Tx, Result, Row, NodeID, EdgeID, DateTime, Error) are accessible
And all public functions (Open, OpenWithConfig, Version, Features) are accessible
```

---

## AC-BUILD: Build Configuration

### AC-BUILD-001: Dynamic Linking (Default)
```gherkin
Given libcypherlite_ffi.so (or .dylib) is in LD_LIBRARY_PATH
When I run "go build ./bindings/go/cypherlite/" without build tags
Then the package compiles using dynamic linking
And the resulting binary requires libcypherlite_ffi at runtime
```

### AC-BUILD-002: Static Linking
```gherkin
Given libcypherlite_ffi.a is available
When I run "go build -tags cypherlite_static ./bindings/go/cypherlite/"
Then the package compiles using static linking
And the resulting binary does not require libcypherlite_ffi at runtime
```

### AC-BUILD-003: Subgraph Build Tag
```gherkin
Given libcypherlite_ffi is built with --features subgraph
When I run "go build -tags subgraph ./bindings/go/cypherlite/"
Then the package compiles with SubgraphID type available
And CYL_FEATURE_SUBGRAPH is defined before header inclusion
```

### AC-BUILD-004: Hypergraph Build Tag
```gherkin
Given libcypherlite_ffi is built with --features hypergraph
When I run "go build -tags hypergraph ./bindings/go/cypherlite/"
Then the package compiles with HyperEdgeID and TemporalNodeRef types available
And CYL_FEATURE_HYPERGRAPH is defined before header inclusion
```

---

## AC-INFO: Library Info Functions

### AC-INFO-001: Version String
```gherkin
Given the Go package is compiled and linked
When I call cypherlite.Version()
Then it returns a non-empty string matching the Rust crate version
And the returned string is a valid Go string (not a C pointer)
```

### AC-INFO-002: Features String
```gherkin
Given the Go package is compiled and linked
When I call cypherlite.Features()
Then it returns a comma-separated list of enabled feature flags
And the returned string is a valid Go string (not a C pointer)
```

---

## AC-LIFECYCLE: Database Lifecycle

### AC-LIFECYCLE-001: Open and Close
```gherkin
Given a valid filesystem path for a new database
When I call cypherlite.Open(path)
Then a *DB handle is returned without error
And the database file is created at the given path
When I call db.Close()
Then no error is returned
And the database file is flushed and closed
```

### AC-LIFECYCLE-002: Open with Config
```gherkin
Given a valid filesystem path
When I call cypherlite.OpenWithConfig(path, 8192, 1024)
Then a *DB handle is returned without error
And the database uses the specified page size and cache capacity
```

### AC-LIFECYCLE-003: Open Invalid Path
```gherkin
Given an invalid filesystem path (e.g., "/nonexistent/dir/db.cyl")
When I call cypherlite.Open(path)
Then an error is returned (not nil)
And the error implements the error interface
And the error contains the CYL_ERR_IO code
And the error message describes the I/O failure
```

### AC-LIFECYCLE-004: Double Close Safety
```gherkin
Given a *DB handle that has been opened
When I call db.Close() twice
Then the first call succeeds without error
And the second call is a no-op (no panic, no error or nil error)
```

### AC-LIFECYCLE-005: Finalizer Safety Net
```gherkin
Given a *DB handle is created but Close() is not called
When the Go garbage collector runs and finalizes the DB
Then cyl_db_close is called automatically
And no panic or segfault occurs
```

---

## AC-ERROR: Error Handling

### AC-ERROR-001: Error Interface
```gherkin
Given a CGo call returns CYL_ERR_PARSE
When the Go binding converts the error
Then the returned error implements the error interface
And error.Error() contains both the error code name and the detailed message
```

### AC-ERROR-002: Sentinel Error Matching
```gherkin
Given a transaction conflict error occurs
When I check errors.Is(err, cypherlite.ErrTransactionConflict)
Then the result is true
And errors.Is(err, cypherlite.ErrNodeNotFound) is false
```

### AC-ERROR-003: Error Unwrap Chain
```gherkin
Given an error returned from db.Execute()
When I call errors.As(err, &cylErr) with *cypherlite.Error target
Then cylErr.Code contains the CylError integer code
And cylErr.Message contains the detailed error message from cyl_last_error_message()
```

### AC-ERROR-004: Thread-Local Error Safety
```gherkin
Given two goroutines executing queries concurrently
When goroutine A triggers CYL_ERR_PARSE and goroutine B triggers CYL_ERR_EXECUTION
Then goroutine A receives the parse error message (not execution error)
And goroutine B receives the execution error message (not parse error)
And neither goroutine receives the other's error message
```

---

## AC-THREAD: Thread Safety

### AC-THREAD-001: LockOSThread Pattern
```gherkin
Given any CGo function call that may set a thread-local error
When the Go binding executes that call
Then runtime.LockOSThread() is called before the CGo call
And cyl_last_error_message() is called before runtime.UnlockOSThread()
And runtime.UnlockOSThread() is deferred after the error retrieval
```

### AC-THREAD-002: Concurrent DB Access
```gherkin
Given a *DB handle shared between 10 goroutines
When each goroutine executes 100 read queries concurrently
Then all queries complete without data races
And "go test -race" reports no race conditions
```

### AC-THREAD-003: Tx Single-Goroutine Enforcement
```gherkin
Given a *Tx handle
When the documentation for Tx is inspected
Then it clearly states that *Tx is NOT safe for concurrent use
And examples show single-goroutine usage pattern
```

---

## AC-QUERY: Query Execution

### AC-QUERY-001: Simple Query
```gherkin
Given an open database
When I call db.Execute("CREATE (n:Person {name: 'Alice'}) RETURN n")
Then a *Result is returned without error
And result.RowCount() returns 1
When I call db.Execute("MATCH (n:Person) RETURN n.name")
Then result.RowCount() returns 1
And row.Get(0) returns "Alice" as a Go string
```

### AC-QUERY-002: Parameterized Query
```gherkin
Given an open database
When I call db.ExecuteWithParams(
    "CREATE (n:Person {name: $name, age: $age}) RETURN n",
    map[string]interface{}{"name": "Bob", "age": int64(30)})
Then a *Result is returned without error
When I call db.Execute("MATCH (n:Person {name: 'Bob'}) RETURN n.age")
Then row.Get(0) returns int64(30)
```

### AC-QUERY-003: Unsupported Parameter Type
```gherkin
Given an open database
When I call db.ExecuteWithParams("MATCH (n) RETURN n",
    map[string]interface{}{"bad": struct{}{}})
Then an error is returned
And errors.Is(err, cypherlite.ErrUnsupportedParamType) is true
```

### AC-QUERY-004: Invalid Query
```gherkin
Given an open database
When I call db.Execute("INVALID CYPHER SYNTAX")
Then an error is returned
And errors.Is(err, cypherlite.ErrParse) is true
And err.Error() contains a descriptive parse error message
```

---

## AC-TX: Transaction Support

### AC-TX-001: Commit Lifecycle
```gherkin
Given an open database
When I call db.Begin()
Then a *Tx is returned without error
When I call tx.Execute("CREATE (n:TxTest {val: 1})")
Then no error is returned
When I call tx.Commit()
Then no error is returned
When I call db.Execute("MATCH (n:TxTest) RETURN n.val")
Then result.RowCount() returns 1
And row.Get(0) returns int64(1)
```

### AC-TX-002: Rollback Lifecycle
```gherkin
Given an open database with no TxTest nodes
When I call db.Begin()
And tx.Execute("CREATE (n:TxTest {val: 2})")
And tx.Rollback()
Then no error is returned
When I call db.Execute("MATCH (n:TxTest) RETURN n")
Then result.RowCount() returns 0
```

### AC-TX-003: Transaction Conflict
```gherkin
Given an open database with an active transaction (tx1)
When I call db.Begin() to start tx2
Then an error is returned
And errors.Is(err, cypherlite.ErrTransactionConflict) is true
```

### AC-TX-004: Execute After Commit
```gherkin
Given a *Tx that has been committed
When I call tx.Execute("MATCH (n) RETURN n")
Then an error is returned
And errors.Is(err, cypherlite.ErrTxClosed) is true
```

### AC-TX-005: Double Commit Safety
```gherkin
Given a *Tx that has been committed
When I call tx.Commit() again
Then ErrTxClosed is returned (no panic, no segfault)
```

### AC-TX-006: Parameterized Query in Transaction
```gherkin
Given an active transaction
When I call tx.ExecuteWithParams(
    "CREATE (n:TxParam {name: $name}) RETURN n",
    map[string]interface{}{"name": "Charlie"})
Then a *Result is returned without error
When I call tx.Commit()
Then the node persists in the database
```

---

## AC-RESULT: Result Access

### AC-RESULT-001: Column Enumeration
```gherkin
Given a query "MATCH (n:Person) RETURN n.name, n.age"
When I call result.Columns()
Then a []string with ["n.name", "n.age"] is returned
And result.Columns() returns the same slice on subsequent calls (cached)
```

### AC-RESULT-002: Row Count
```gherkin
Given a query that returns 5 rows
When I call result.RowCount()
Then 5 is returned
```

### AC-RESULT-003: Row Access
```gherkin
Given a result with 3 rows
When I call result.Row(0), result.Row(1), result.Row(2)
Then each returns a *Row with the correct row data
When I call result.Row(3) (out of bounds)
Then nil is returned (or appropriate Go zero value)
```

### AC-RESULT-004: Result Close
```gherkin
Given a *Result from a query
When I call result.Close()
Then the C-side CylResult is freed
And subsequent calls to result methods return zero values (no crash)
```

---

## AC-ROW: Row Value Access

### AC-ROW-001: Get by Column Index
```gherkin
Given a result row with columns [name(string), age(int64), active(bool)]
When I call row.Get(0)
Then a Go string is returned
When I call row.Get(1)
Then a Go int64 is returned
When I call row.Get(2)
Then a Go bool is returned
```

### AC-ROW-002: Get by Column Name
```gherkin
Given a result row from "MATCH (n) RETURN n.name AS name"
When I call row.GetByName("name")
Then the correct Go string value is returned
When I call row.GetByName("nonexistent")
Then nil is returned
```

---

## AC-VALUE: Value Type Mapping

### AC-VALUE-001: Null Value
```gherkin
Given a query returning a null property
When I read the value via row.Get()
Then nil (Go nil interface) is returned
```

### AC-VALUE-002: Boolean Value
```gherkin
Given a node with boolean property {active: true}
When I query and read the property via row.Get()
Then a Go bool with value true is returned
```

### AC-VALUE-003: Integer Value
```gherkin
Given a node with integer property {count: 42}
When I query and read the property via row.Get()
Then a Go int64 with value 42 is returned
```

### AC-VALUE-004: Float Value
```gherkin
Given a node with float property {score: 3.14}
When I query and read the property via row.Get()
Then a Go float64 with value 3.14 is returned
```

### AC-VALUE-005: String Value
```gherkin
Given a node with string property {name: 'CypherLite'}
When I query and read the property via row.Get()
Then a Go string with value "CypherLite" is returned
And the string is Go-managed memory (not a pointer into C memory)
```

### AC-VALUE-006: String with Unicode
```gherkin
Given a node with string property {name: 'Unicode Text'}
When I query and read the property via row.Get()
Then the full Unicode string is correctly returned
```

### AC-VALUE-007: NodeID Value
```gherkin
Given a query "CREATE (n:Test) RETURN id(n)"
When I read the value via row.Get()
Then a cypherlite.NodeID (uint64) is returned
```

### AC-VALUE-008: EdgeID Value
```gherkin
Given a query returning an edge ID
When I read the value via row.Get()
Then a cypherlite.EdgeID (uint64) is returned
```

### AC-VALUE-009: DateTime Value
```gherkin
Given a node with datetime property
When I read the value via row.Get()
Then a cypherlite.DateTime (int64, milliseconds since epoch) is returned
```

### AC-VALUE-010: Parameter Type Round-Trip
```gherkin
Given each supported Go parameter type (nil, bool, int64, float64, string, []byte)
When I create a node with that parameter via ExecuteWithParams
And query back the property value
Then the returned Go value matches the original input
```

---

## AC-TEST: Testing Requirements

### AC-TEST-001: Race Detector Clean
```gherkin
Given the complete Go test suite
When I run "go test -race ./bindings/go/cypherlite/..."
Then all tests pass
And no data race conditions are reported
```

### AC-TEST-002: Full Lifecycle Integration
```gherkin
Given a clean test environment
When the integration test runs:
  1. Open database
  2. CREATE 10 nodes with properties
  3. CREATE 10 edges between nodes
  4. MATCH all nodes and verify count
  5. MATCH with property filter and verify values
  6. Transaction: BEGIN -> CREATE -> COMMIT -> verify
  7. Transaction: BEGIN -> CREATE -> ROLLBACK -> verify absence
  8. Close database
Then all assertions pass
And no resources are leaked
```

### AC-TEST-003: Error Recovery
```gherkin
Given an open database
When I execute an invalid query (triggers error)
And then execute a valid query
Then the valid query succeeds
And the previous error does not affect the new query
```

### AC-TEST-004: Resource Cleanup Verification
```gherkin
Given the test suite completes
When I inspect for leaked C resources
Then no CylDb, CylTx, or CylResult handles remain un-freed
And runtime finalizers did not need to fire (all Close() called explicitly)
```

---

## AC-NFR: Non-Functional Requirements

### AC-NFR-001: GoDoc Coverage
```gherkin
Given all exported types and functions in the package
When I run "go doc ./bindings/go/cypherlite/"
Then every exported type has a doc comment
And every exported function has a doc comment
And thread safety is documented for DB, Tx, and Result types
```

### AC-NFR-002: Platform Compatibility
```gherkin
Given the Go package and pre-built libcypherlite_ffi
When I run "go test" on Linux x86_64
Then all tests pass
When I run "go test" on macOS aarch64
Then all tests pass
```

### AC-NFR-003: No External Dependencies
```gherkin
Given the go.mod file
When I inspect the require section
Then only the Go standard library is used
And no third-party modules are required
```

### AC-NFR-004: Memory Safety
```gherkin
Given the complete test suite
When all tests complete
Then no Go-visible pointers reference freed C memory
And all C strings have been copied to Go-managed memory before C resource cleanup
And "go test -race" reports zero data races
```

---

## Definition of Done

- [ ] All AC-MODULE criteria pass
- [ ] All AC-BUILD criteria pass
- [ ] All AC-INFO criteria pass
- [ ] All AC-LIFECYCLE criteria pass
- [ ] All AC-ERROR criteria pass
- [ ] All AC-THREAD criteria pass
- [ ] All AC-QUERY criteria pass
- [ ] All AC-TX criteria pass
- [ ] All AC-RESULT criteria pass
- [ ] All AC-ROW criteria pass
- [ ] All AC-VALUE criteria pass
- [ ] All AC-TEST criteria pass
- [ ] All AC-NFR criteria pass
- [ ] `go test -race` passes with zero races
- [ ] `go vet` reports zero issues
- [ ] README.md with build instructions and examples
- [ ] CI job passes on Linux and macOS
