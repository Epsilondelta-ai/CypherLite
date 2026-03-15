# SPEC-FFI-004: Acceptance Criteria

| Field     | Value                                        |
| --------- | -------------------------------------------- |
| SPEC      | SPEC-FFI-004                                 |
| Title     | Node.js Bindings for CypherLite via napi-rs  |
| Format    | Given-When-Then (Gherkin)                    |

---

## AC-SETUP: Crate and Package Setup

### AC-SETUP-001: Cargo Workspace Integration

```gherkin
Given the CypherLite workspace Cargo.toml
When "crates/cypherlite-node" is listed in members
Then `cargo check -p cypherlite-node` shall compile without errors
And the crate output type shall be cdylib
```

### AC-SETUP-002: napi-rs Configuration

```gherkin
Given the cypherlite-node Cargo.toml
When napi dependency is declared with feature "napi6"
And napi-derive is declared as a dependency
Then the built .node addon shall be compatible with Node.js 14 through 22
```

### AC-SETUP-003: napi Build

```gherkin
Given a package.json with @napi-rs/cli configured
When `napi build --release` is executed
Then a .node native addon shall be produced
And an index.d.ts TypeScript definition file shall be generated
And `const cypherlite = require('./index')` shall succeed
And `cypherlite.version()` shall return a non-empty string
And `cypherlite.features()` shall return a string
```

### AC-SETUP-004: Feature Flag Propagation

```gherkin
Given the cypherlite-node crate with feature flags from cypherlite-query
When built with `--features subgraph`
Then `cypherlite.features()` shall include "subgraph" in the returned string
```

### AC-SETUP-005: Platform Detection

```gherkin
Given a successful napi build
When index.js is loaded via require()
Then it shall detect the current OS and architecture
And load the correct platform-specific .node binary
And throw a clear error if no binary is available for the current platform
```

---

## AC-LIFECYCLE: Database Lifecycle

### AC-LIFECYCLE-001: Open with Default Config

```gherkin
Given a valid file system path
When cypherlite.open(path) is called
Then a Database object shall be returned
And the .cyl file shall be created at the specified path
```

### AC-LIFECYCLE-002: Open with Custom Config

```gherkin
Given a valid file system path
When cypherlite.open(path, { pageSize: 8192, cacheCapacity: 2048 }) is called
Then a Database object shall be returned with the specified configuration
```

### AC-LIFECYCLE-003: Close Database

```gherkin
Given an open Database object
When db.close() is called
Then all pending writes shall be flushed
And subsequent operations shall throw CypherLiteError with "Database is closed"
```

### AC-LIFECYCLE-004: Double Close is No-op

```gherkin
Given an open Database object
When db.close() is called twice
Then no exception shall be thrown
And no crash or segfault shall occur
```

### AC-LIFECYCLE-005: Operations on Closed Database

```gherkin
Given a closed Database object
When db.execute("MATCH (n) RETURN n") is called
Then CypherLiteError shall be thrown with message containing "closed"
When db.begin() is called
Then CypherLiteError shall be thrown with message containing "closed"
```

---

## AC-QUERY: Query Execution

### AC-QUERY-001: Simple Query

```gherkin
Given an open Database
When db.execute("CREATE (n:Person {name: 'Alice'}) RETURN n.name") is called
Then a Result object shall be returned
And result.columns shall deep-equal ["n.name"]
And result.length shall equal 1
And result.get(0)["n.name"] shall equal "Alice"
```

### AC-QUERY-002: Parameterized Query

```gherkin
Given an open Database with a Person node named "Alice"
When db.execute("MATCH (n:Person {name: $name}) RETURN n.name", { name: "Alice" }) is called
Then a Result object shall be returned
And result.get(0)["n.name"] shall equal "Alice"
```

### AC-QUERY-003: Multiple Rows

```gherkin
Given an open Database with Person nodes "Alice" and "Bob"
When db.execute("MATCH (n:Person) RETURN n.name ORDER BY n.name") is called
Then result.length shall equal 2
And result.get(0)["n.name"] shall equal "Alice"
And result.get(1)["n.name"] shall equal "Bob"
```

### AC-QUERY-004: Empty Result

```gherkin
Given an open Database with no data
When db.execute("MATCH (n:Person) RETURN n") is called
Then a Result object shall be returned
And result.length shall equal 0
And result.toArray() shall deep-equal []
```

### AC-QUERY-005: Parse Error

```gherkin
Given an open Database
When db.execute("INVALID QUERY SYNTAX") is called
Then CypherLiteError shall be thrown
And error.code shall equal "ParseError"
And error.message shall describe the parse failure
```

### AC-QUERY-006: Unsupported Parameter Type

```gherkin
Given an open Database
When db.execute("MATCH (n) RETURN n", { key: { nested: "object" } }) is called
Then TypeError shall be thrown (not CypherLiteError)
And the error message shall list the unsupported type
```

---

## AC-TX: Transaction Support

### AC-TX-001: Commit Transaction

```gherkin
Given an open Database
When a transaction is started and committed:
  const tx = db.begin()
  tx.execute("CREATE (n:Person {name: 'Alice'})")
  tx.commit()
Then the data shall be persisted
And db.execute("MATCH (n:Person) RETURN n.name").get(0)["n.name"] shall equal "Alice"
```

### AC-TX-002: Rollback Transaction

```gherkin
Given an open Database
When a transaction is started and rolled back:
  const tx = db.begin()
  tx.execute("CREATE (n:Person {name: 'Alice'})")
  tx.rollback()
Then the data shall NOT be persisted
And db.execute("MATCH (n:Person) RETURN n").length shall equal 0
```

### AC-TX-003: Transaction Execute with Params

```gherkin
Given an open Database and active transaction
When tx.execute("CREATE (n:Person {name: $name})", { name: "Alice" }) is called
Then the parameterized query shall execute within the transaction context
```

### AC-TX-004: Operations on Closed Transaction

```gherkin
Given a committed transaction
When tx.execute("MATCH (n) RETURN n") is called
Then CypherLiteError shall be thrown with message containing "closed"
When tx.commit() is called
Then CypherLiteError shall be thrown
When tx.rollback() is called
Then CypherLiteError shall be thrown
```

### AC-TX-005: Try-Finally Pattern

```gherkin
Given an open Database
When a transaction is used with try-finally:
  const tx = db.begin()
  try {
      tx.execute("CREATE (n:Person {name: 'Alice'})")
      throw new Error("test error")
  } catch (e) {
      tx.rollback()
  }
Then the data shall NOT be persisted
And the error shall have been caught
```

---

## AC-RESULT: Result Access

### AC-RESULT-001: Column Names

```gherkin
Given a Result from "MATCH (n:Person) RETURN n.name, n.age"
When result.columns is accessed
Then it shall deep-equal ["n.name", "n.age"]
```

### AC-RESULT-002: Row Count via length

```gherkin
Given a Result with 3 rows
When result.length is accessed
Then it shall equal 3
```

### AC-RESULT-003: Index Access via get()

```gherkin
Given a Result with 3 rows
When result.get(0) is called
Then a Row object shall be returned for the first row
When result.get(2) is called
Then a Row object shall be returned for the last row
When result.get(3) is called
Then RangeError shall be thrown
```

### AC-RESULT-004: Iteration via for...of

```gherkin
Given a Result with N rows
When iterating with `for (const row of result)`
Then exactly N Row objects shall be yielded
And each Row shall provide access to column values
```

### AC-RESULT-005: toArray Method

```gherkin
Given a Result with 3 rows
When result.toArray() is called
Then an Array of 3 Row objects shall be returned
And each element shall be a valid Row with column values
```

### AC-RESULT-006: toString Method

```gherkin
Given a Result with columns ["name", "age"] and 5 rows
When result.toString() is called
Then it shall return '[CypherLiteResult columns=["name","age"] rows=5]'
```

---

## AC-ROW: Row Value Access

### AC-ROW-001: Access by Column Name

```gherkin
Given a Row from a query returning n.name = "Alice"
When row["n.name"] is accessed
Then it shall return "Alice"
```

### AC-ROW-002: Access by Numeric Index

```gherkin
Given a Row from a query with columns ["n.name", "n.age"]
When row[0] is accessed
Then it shall return the value of the first column
When row[1] is accessed
Then it shall return the value of the second column
```

### AC-ROW-003: Missing Column Name

```gherkin
Given a Row without a column "nonexistent"
When row["nonexistent"] is accessed
Then it shall return undefined (JavaScript convention)
```

### AC-ROW-004: Out of Bounds Index

```gherkin
Given a Row with 2 columns
When row[5] is accessed
Then it shall return undefined (JavaScript convention)
```

### AC-ROW-005: Object.keys() Returns Column Names

```gherkin
Given a Row from a query with columns ["n.name", "n.age"]
When Object.keys(row) is called
Then it shall return ["n.name", "n.age"] (string keys only, no numeric indices)
```

### AC-ROW-006: toObject Method

```gherkin
Given a Row with name="Alice" and age=30
When row.toObject() is called
Then it shall return { "n.name": "Alice", "n.age": 30 }
And the returned object shall NOT have numeric keys or length property
```

### AC-ROW-007: Length Property

```gherkin
Given a Row with 3 columns
When row.length is accessed
Then it shall return 3
And "length" shall NOT appear in Object.keys(row)
```

---

## AC-VALUE: Value Type Mapping

### AC-VALUE-001: Null Value

```gherkin
Given a query that returns a null value
When the value is accessed from a Row
Then it shall be JavaScript null
```

### AC-VALUE-002: Boolean Value

```gherkin
Given a query "RETURN true AS val"
When result.get(0)["val"] is accessed
Then it shall be JavaScript true (typeof === "boolean")
```

### AC-VALUE-003: Integer Value

```gherkin
Given a query "RETURN 42 AS val"
When result.get(0)["val"] is accessed
Then it shall be JavaScript 42 (typeof === "number")
```

### AC-VALUE-004: Float Value

```gherkin
Given a query "RETURN 3.14 AS val"
When result.get(0)["val"] is accessed
Then it shall be JavaScript 3.14 (typeof === "number")
```

### AC-VALUE-005: String Value

```gherkin
Given a query "RETURN 'hello' AS val"
When result.get(0)["val"] is accessed
Then it shall be JavaScript "hello" (typeof === "string")
```

### AC-VALUE-006: List Value

```gherkin
Given a query "RETURN [1, 2, 3] AS val"
When result.get(0)["val"] is accessed
Then it shall be JavaScript [1, 2, 3] (Array.isArray === true)
And each element shall be the correctly converted JavaScript type
```

### AC-VALUE-007: NodeID Value as BigInt

```gherkin
Given a query that returns a Node ID
When the value is accessed from a Row
Then it shall be a BigInt (typeof === "bigint")
And Number(value) shall return the underlying ID if within safe integer range
```

### AC-VALUE-008: EdgeID Value as BigInt

```gherkin
Given a query that returns an Edge ID
When the value is accessed from a Row
Then it shall be a BigInt (typeof === "bigint")
```

### AC-VALUE-009: DateTime Value

```gherkin
Given a query that returns a DateTime value
When the value is accessed from a Row
Then it shall be an instance of Date
And value.getTime() shall return the milliseconds since epoch
```

### AC-VALUE-010: Buffer Value

```gherkin
Given a query that returns a Bytes value
When the value is accessed from a Row
Then it shall be an instance of Buffer
And Buffer.isBuffer(value) shall return true
```

### AC-VALUE-011: Parameter Roundtrip

```gherkin
Given the following JavaScript parameter values:
  null, true, 42, 3.14, "hello", Buffer.from([0, 1, 2]), [1, "two", null]
When each is passed as a query parameter and returned
Then the returned value shall equal the original JavaScript value
```

### AC-VALUE-012: BigInt Parameter

```gherkin
Given a BigInt value 42n
When passed as a query parameter
Then it shall be converted to Int64 in Rust
And the returned value shall equal 42 (number) or 42n (BigInt) depending on context
```

### AC-VALUE-013: Undefined Parameter

```gherkin
Given an undefined value in params
When passed as a query parameter
Then it shall be converted to Value::Null
And the returned value shall be null
```

### AC-VALUE-014: Subgraph Value (Feature-Gated)

```gherkin
Given a build with --features subgraph
And a query that returns a Subgraph ID
When the value is accessed from a Row
Then it shall be a BigInt
```

### AC-VALUE-015: HyperEdge Value (Feature-Gated)

```gherkin
Given a build with --features hypergraph
And a query that returns a HyperEdge ID
When the value is accessed from a Row
Then it shall be a BigInt
```

### AC-VALUE-016: TemporalNode Value (Feature-Gated)

```gherkin
Given a build with --features hypergraph
And a query that returns a TemporalNode
When the value is accessed from a Row
Then it shall be an object { nodeId: BigInt, timestamp: BigInt }
```

---

## AC-ERROR: Error Handling

### AC-ERROR-001: CypherLiteError extends Error

```gherkin
Given the cypherlite module
When CypherLiteError is thrown
Then error instanceof Error shall be true
And error instanceof CypherLiteError shall be true
And error.name shall equal "CypherLiteError"
```

### AC-ERROR-002: Error Code Property

```gherkin
Given a CypherLiteError thrown by a parse failure
When inspecting the error
Then error.message shall contain the parse error description
And error.code shall equal "ParseError"
And error.stack shall include the call stack
```

### AC-ERROR-003: Error Code Mapping

```gherkin
Given various Rust error types
When they are converted to JavaScript
Then IoError shall have code "IoError"
And ParseError shall have code "ParseError"
And ExecutionError shall have code "ExecutionError"
And TransactionConflict shall have code "TransactionConflict"
```

### AC-ERROR-004: TypeError for Invalid Arguments

```gherkin
Given an open Database
When db.execute(42) is called (non-string query)
Then TypeError shall be thrown (not CypherLiteError)
```

### AC-ERROR-005: Error in try-catch

```gherkin
Given an open Database
When an error-causing query is executed inside try-catch:
  try {
      db.execute("INVALID SYNTAX")
  } catch (e) {
      // inspect e
  }
Then e shall be an instance of CypherLiteError
And e.code shall be defined
And e.message shall be non-empty
```

---

## AC-THREAD: Thread Safety

### AC-THREAD-001: Main Thread Usage

```gherkin
Given an open Database
When database operations are performed on the main thread
Then all operations shall complete without errors
And results shall be correct
```

### AC-THREAD-002: Sync API Does Not Return Promises

```gherkin
Given an open Database
When db.execute("MATCH (n) RETURN n") is called
Then the return value shall NOT be a Promise
And typeof result shall NOT be "object" with .then method
And the result shall be immediately available
```

---

## AC-INFO: Library Info

### AC-INFO-001: Version

```gherkin
When cypherlite.version() is called
Then it shall return a string matching the pattern "X.Y.Z" (e.g., "1.1.0")
```

### AC-INFO-002: Features

```gherkin
When cypherlite.features() is called
Then it shall return a string (possibly empty if no features enabled)
And when built with --features temporal-core, the string shall contain "temporal-core"
```

---

## AC-TYPEDEF: TypeScript Definitions

### AC-TYPEDEF-001: tsc Validation

```gherkin
Given the index.d.ts TypeScript definition file
When `tsc --noEmit __test__/database.spec.ts` is executed
Then it shall pass with no type errors
```

### AC-TYPEDEF-002: Type Completeness

```gherkin
Given the TypeScript definition file
When inspecting its contents
Then DatabaseConfig interface shall be defined with optional pageSize and cacheCapacity
And Database class shall have execute, begin, close methods with correct signatures
And Transaction class shall have execute, commit, rollback methods
And CypherLiteResult class shall have columns, length, get, toArray, [Symbol.iterator]
And CypherLiteError shall have code property typed as string
And open, version, features function signatures shall be exported
```

---

## AC-PACKAGE: npm Package

### AC-PACKAGE-001: npm Install from Build

```gherkin
Given a native addon built by `napi build --release`
When `node -e "const c = require('./index'); console.log(c.version())"` is executed
Then it shall print a valid version string
```

### AC-PACKAGE-002: Platform-Specific Packages

```gherkin
Given the package.json napi configuration
When `napi prepublish` is executed
Then platform-specific npm packages shall be generated:
  @cypherlite/linux-x64-gnu
  @cypherlite/darwin-arm64
  (etc.)
And each shall contain the correct .node binary
```

---

## AC-TEST: Test Coverage

### AC-TEST-001: Rust Unit Tests

```gherkin
Given the cypherlite-node crate
When `cargo test -p cypherlite-node` is executed
Then all Rust unit tests shall pass
```

### AC-TEST-002: vitest Integration Tests

```gherkin
Given a napi build
When `npx vitest run` is executed
Then all JavaScript/TypeScript integration tests shall pass
```

### AC-TEST-003: Complete Lifecycle Test

```gherkin
Given a clean temporary directory
When the complete lifecycle is executed:
  const cypherlite = require('./index')
  const db = cypherlite.open(path.join(tmpDir, 'test.cyl'))
  db.execute("CREATE (n:Person {name: 'Alice', age: 30})")
  const result = db.execute("MATCH (n:Person) RETURN n.name, n.age")
  assert(result.columns.length === 2)
  assert(result.length === 1)
  for (const row of result) {
      assert(row['n.name'] === 'Alice')
      assert(row['n.age'] === 30)
  }
  db.close()
Then all assertions shall pass
```

---

## AC-NFR: Non-Functional Requirements

### AC-NFR-001: Multi-Platform Build

```gherkin
Given the cypherlite-node crate
When built on Linux x86_64, macOS aarch64, and Windows x86_64
Then `napi build --release` shall succeed on all platforms
And vitest tests shall pass on all platforms
```

### AC-NFR-002: Node.js Version Compatibility

```gherkin
Given a .node addon built with napi6
When loaded on Node.js 14, 18, 20, and 22
Then require() and basic operations shall succeed on all versions
```

### AC-NFR-003: Idiomatic JavaScript API

```gherkin
Given the cypherlite JavaScript API
When reviewed against Node.js conventions
Then sync API shall be used for embedded DB operations (no unnecessary Promises)
And Symbol.iterator shall be used for result traversal
And property access shall be used for row values
And Error subclass shall be used for error handling
And Buffer shall be used for binary data
And BigInt shall be used for 64-bit IDs
```

---

## vitest Test Scenarios Summary

| Test File                    | Scenarios                                              | Count |
| ---------------------------- | ------------------------------------------------------ | ----- |
| `database.spec.ts`           | open, close, config, double close, closed state        | ~8    |
| `query.spec.ts`              | simple, parameterized, CRUD, empty result, errors      | ~12   |
| `transaction.spec.ts`        | commit, rollback, try-finally, conflict, closed         | ~8    |
| `result.spec.ts`             | columns, length, get, for...of, toArray, toString      | ~10   |
| `values.spec.ts`             | all type conversions, roundtrip, edge cases, BigInt     | ~16   |
| `errors.spec.ts`             | CypherLiteError, code, instanceof, TypeError            | ~8    |
| **Total**                    |                                                        | **~62** |
