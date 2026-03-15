# SPEC-FFI-003: Acceptance Criteria

| Field     | Value                                   |
| --------- | --------------------------------------- |
| SPEC      | SPEC-FFI-003                            |
| Title     | Python Bindings for CypherLite via PyO3 |
| Format    | Given-When-Then (Gherkin)               |

---

## AC-SETUP: Crate and Package Setup

### AC-SETUP-001: Cargo Workspace Integration

```gherkin
Given the CypherLite workspace Cargo.toml
When "crates/cypherlite-python" is listed in members
Then `cargo check -p cypherlite-python` shall compile without errors
And the crate output type shall be cdylib
```

### AC-SETUP-002: PyO3 abi3 Configuration

```gherkin
Given the cypherlite-python Cargo.toml
When pyo3 dependency is declared with features ["abi3-py38", "extension-module"]
Then the built .so/.pyd shall be compatible with Python 3.8 through 3.13
And no per-version wheels shall be required
```

### AC-SETUP-003: maturin Build

```gherkin
Given a pyproject.toml with maturin build-backend
When `maturin develop` is executed in a Python 3.8+ virtualenv
Then `import cypherlite` shall succeed without errors
And `cypherlite.version()` shall return a non-empty string
And `cypherlite.features()` shall return a string
```

### AC-SETUP-004: Feature Flag Propagation

```gherkin
Given the cypherlite-python crate with feature flags from cypherlite-query
When built with `--features subgraph`
Then `cypherlite.features()` shall include "subgraph" in the returned string
And SubgraphID type shall be available
```

### AC-SETUP-005: Python Package Structure

```gherkin
Given a successful maturin develop build
When inspecting the installed package
Then cypherlite/__init__.py shall exist
And cypherlite/py.typed shall exist
And cypherlite/_cypherlite.pyi shall exist
And cypherlite/_cypherlite.*.so (or .pyd) shall exist
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
When cypherlite.open(path, page_size=8192, cache_capacity=2048) is called
Then a Database object shall be returned with the specified configuration
```

### AC-LIFECYCLE-003: Close Database

```gherkin
Given an open Database object
When db.close() is called
Then all pending writes shall be flushed
And subsequent operations shall raise CypherLiteError with "Database is closed"
```

### AC-LIFECYCLE-004: Double Close is No-op

```gherkin
Given an open Database object
When db.close() is called twice
Then no exception shall be raised
And no crash or segfault shall occur
```

### AC-LIFECYCLE-005: Context Manager Normal Exit

```gherkin
Given a valid file system path
When the database is used as a context manager:
  with cypherlite.open(path) as db:
      db.execute("CREATE (n:Test)")
Then the database shall be closed after the with block exits
And subsequent operations on db shall raise CypherLiteError
```

### AC-LIFECYCLE-006: Context Manager Exception Exit

```gherkin
Given a valid file system path
When an exception occurs inside the context manager:
  with cypherlite.open(path) as db:
      raise ValueError("test error")
Then the database shall be closed despite the exception
And the ValueError shall propagate to the caller
```

### AC-LIFECYCLE-007: Operations on Closed Database

```gherkin
Given a closed Database object
When db.execute("MATCH (n) RETURN n") is called
Then CypherLiteError shall be raised with message containing "closed"
When db.begin() is called
Then CypherLiteError shall be raised with message containing "closed"
```

---

## AC-QUERY: Query Execution

### AC-QUERY-001: Simple Query

```gherkin
Given an open Database
When db.execute("CREATE (n:Person {name: 'Alice'}) RETURN n.name") is called
Then a Result object shall be returned
And result.columns shall equal ["n.name"]
And len(result) shall equal 1
And result[0]["n.name"] shall equal "Alice"
```

### AC-QUERY-002: Parameterized Query

```gherkin
Given an open Database with a Person node named "Alice"
When db.execute("MATCH (n:Person {name: $name}) RETURN n.name", params={"name": "Alice"}) is called
Then a Result object shall be returned
And result[0]["n.name"] shall equal "Alice"
```

### AC-QUERY-003: Multiple Rows

```gherkin
Given an open Database with Person nodes "Alice" and "Bob"
When db.execute("MATCH (n:Person) RETURN n.name ORDER BY n.name") is called
Then len(result) shall equal 2
And result[0]["n.name"] shall equal "Alice"
And result[1]["n.name"] shall equal "Bob"
```

### AC-QUERY-004: Empty Result

```gherkin
Given an open Database with no data
When db.execute("MATCH (n:Person) RETURN n") is called
Then a Result object shall be returned
And len(result) shall equal 0
And list(result) shall equal []
```

### AC-QUERY-005: Parse Error

```gherkin
Given an open Database
When db.execute("INVALID QUERY SYNTAX") is called
Then ParseError shall be raised (subclass of CypherLiteError)
And the error message shall describe the parse failure
And error.code shall equal "ParseError"
```

### AC-QUERY-006: Unsupported Parameter Type

```gherkin
Given an open Database
When db.execute("MATCH (n) RETURN n", params={"key": {"nested": "dict"}}) is called
Then TypeError shall be raised (not CypherLiteError)
And the error message shall list the unsupported type
```

---

## AC-TX: Transaction Support

### AC-TX-001: Commit Transaction

```gherkin
Given an open Database
When a transaction is started and committed:
  tx = db.begin()
  tx.execute("CREATE (n:Person {name: 'Alice'})")
  tx.commit()
Then the data shall be persisted
And db.execute("MATCH (n:Person) RETURN n.name")[0]["n.name"] shall equal "Alice"
```

### AC-TX-002: Rollback Transaction

```gherkin
Given an open Database
When a transaction is started and rolled back:
  tx = db.begin()
  tx.execute("CREATE (n:Person {name: 'Alice'})")
  tx.rollback()
Then the data shall NOT be persisted
And db.execute("MATCH (n:Person) RETURN n") shall return 0 rows
```

### AC-TX-003: Context Manager Auto-Commit

```gherkin
Given an open Database
When a transaction context manager exits normally:
  with db.begin() as tx:
      tx.execute("CREATE (n:Person {name: 'Alice'})")
Then the transaction shall be committed automatically
And the data shall be persisted
```

### AC-TX-004: Context Manager Auto-Rollback

```gherkin
Given an open Database
When a transaction context manager exits with an exception:
  with db.begin() as tx:
      tx.execute("CREATE (n:Person {name: 'Alice'})")
      raise ValueError("test")
Then the transaction shall be rolled back automatically
And the data shall NOT be persisted
And the ValueError shall propagate
```

### AC-TX-005: Transaction Execute with Params

```gherkin
Given an open Database and active transaction
When tx.execute("CREATE (n:Person {name: $name})", params={"name": "Alice"}) is called
Then the parameterized query shall execute within the transaction context
```

### AC-TX-006: Operations on Closed Transaction

```gherkin
Given a committed transaction
When tx.execute("MATCH (n) RETURN n") is called
Then CypherLiteError shall be raised with message containing "closed"
When tx.commit() is called
Then CypherLiteError shall be raised
When tx.rollback() is called
Then CypherLiteError shall be raised
```

---

## AC-RESULT: Result Access

### AC-RESULT-001: Column Names

```gherkin
Given a Result from "MATCH (n:Person) RETURN n.name, n.age"
When result.columns is accessed
Then it shall equal ["n.name", "n.age"]
```

### AC-RESULT-002: Row Count via len()

```gherkin
Given a Result with 3 rows
When len(result) is called
Then it shall return 3
```

### AC-RESULT-003: Index Access

```gherkin
Given a Result with 3 rows
When result[0] is accessed
Then a Row object shall be returned for the first row
When result[2] is accessed
Then a Row object shall be returned for the last row
When result[3] is accessed
Then IndexError shall be raised
```

### AC-RESULT-004: Negative Index

```gherkin
Given a Result with 3 rows
When result[-1] is accessed
Then a Row object shall be returned for the last row (index 2)
When result[-3] is accessed
Then a Row object shall be returned for the first row (index 0)
```

### AC-RESULT-005: Iteration

```gherkin
Given a Result with N rows
When iterating with `for row in result`
Then exactly N Row objects shall be yielded
And each Row shall provide access to column values
```

### AC-RESULT-006: Repr

```gherkin
Given a Result with columns ["name", "age"] and 5 rows
When repr(result) is called
Then it shall return '<CypherLiteResult columns=["name", "age"] rows=5>'
```

---

## AC-ROW: Row Value Access

### AC-ROW-001: Access by Column Name

```gherkin
Given a Row from a query returning n.name = "Alice"
When row["n.name"] is accessed
Then it shall return "Alice"
```

### AC-ROW-002: Access by Column Index

```gherkin
Given a Row from a query with columns ["n.name", "n.age"]
When row[0] is accessed
Then it shall return the value of the first column
When row[1] is accessed
Then it shall return the value of the second column
```

### AC-ROW-003: Invalid Column Name

```gherkin
Given a Row without a column "nonexistent"
When row["nonexistent"] is accessed
Then KeyError shall be raised
```

### AC-ROW-004: Out of Bounds Index

```gherkin
Given a Row with 2 columns
When row[5] is accessed
Then IndexError shall be raised
```

### AC-ROW-005: Keys Method

```gherkin
Given a Row from a query with columns ["n.name", "n.age"]
When row.keys() is called
Then it shall return ["n.name", "n.age"]
```

### AC-ROW-006: Len Method

```gherkin
Given a Row with 3 columns
When len(row) is called
Then it shall return 3
```

### AC-ROW-007: Repr

```gherkin
Given a Row with name="Alice" and age=30
When repr(row) is called
Then it shall return a string like '<CypherLiteRow {n.name: Alice, n.age: 30}>'
```

---

## AC-VALUE: Value Type Mapping

### AC-VALUE-001: Null Value

```gherkin
Given a query that returns a null value
When the value is accessed from a Row
Then it shall be Python None
```

### AC-VALUE-002: Boolean Value

```gherkin
Given a query "RETURN true AS val"
When result[0]["val"] is accessed
Then it shall be Python True (type bool)
```

### AC-VALUE-003: Integer Value

```gherkin
Given a query "RETURN 42 AS val"
When result[0]["val"] is accessed
Then it shall be Python 42 (type int)
```

### AC-VALUE-004: Float Value

```gherkin
Given a query "RETURN 3.14 AS val"
When result[0]["val"] is accessed
Then it shall be Python 3.14 (type float)
```

### AC-VALUE-005: String Value

```gherkin
Given a query "RETURN 'hello' AS val"
When result[0]["val"] is accessed
Then it shall be Python "hello" (type str)
```

### AC-VALUE-006: List Value

```gherkin
Given a query "RETURN [1, 2, 3] AS val"
When result[0]["val"] is accessed
Then it shall be Python [1, 2, 3] (type list)
And each element shall be the correctly converted Python type
```

### AC-VALUE-007: NodeID Value

```gherkin
Given a query that returns a Node ID
When the value is accessed from a Row
Then it shall be an instance of cypherlite.NodeID
And int(value) shall return the underlying u64 ID
```

### AC-VALUE-008: EdgeID Value

```gherkin
Given a query that returns an Edge ID
When the value is accessed from a Row
Then it shall be an instance of cypherlite.EdgeID
And int(value) shall return the underlying u64 ID
```

### AC-VALUE-009: DateTime Value

```gherkin
Given a query that returns a DateTime value
When the value is accessed from a Row
Then it shall be an instance of datetime.datetime
And it shall be timezone-aware (UTC)
```

### AC-VALUE-010: Parameter Roundtrip

```gherkin
Given the following Python parameter values:
  None, True, 42, 3.14, "hello", b"\x00\x01", [1, "two", None]
When each is passed as a query parameter and returned
Then the returned value shall equal the original Python value
```

### AC-VALUE-011: Subgraph Value (Feature-Gated)

```gherkin
Given a build with --features subgraph
And a query that returns a Subgraph ID
When the value is accessed from a Row
Then it shall be an instance of cypherlite.SubgraphID
```

### AC-VALUE-012: HyperEdge Value (Feature-Gated)

```gherkin
Given a build with --features hypergraph
And a query that returns a HyperEdge ID
When the value is accessed from a Row
Then it shall be an instance of cypherlite.HyperEdgeID
```

---

## AC-ERROR: Error Handling

### AC-ERROR-001: Exception Hierarchy

```gherkin
Given the cypherlite module
When inspecting exception classes
Then CypherLiteError shall be a subclass of Exception
And ParseError shall be a subclass of CypherLiteError
And ExecutionError shall be a subclass of CypherLiteError
And TransactionConflictError shall be a subclass of CypherLiteError
And IoError shall be a subclass of CypherLiteError
```

### AC-ERROR-002: Error Attributes

```gherkin
Given a CypherLiteError raised by a parse failure
When inspecting the exception
Then error.message shall contain the parse error description
And error.code shall equal "ParseError"
And str(error) shall contain the error message
```

### AC-ERROR-003: isinstance Check

```gherkin
Given a ParseError raised by an invalid query
When checking isinstance(error, CypherLiteError)
Then it shall return True
When checking isinstance(error, ParseError)
Then it shall return True
When checking isinstance(error, ExecutionError)
Then it shall return False
```

### AC-ERROR-004: TypeError for Invalid Arguments

```gherkin
Given an open Database
When db.execute(42) is called (non-string query)
Then TypeError shall be raised (not CypherLiteError)
```

---

## AC-THREAD: Thread Safety

### AC-THREAD-001: Database Multi-Thread Access

```gherkin
Given an open Database
When multiple Python threads execute read queries concurrently (via threading module)
Then all queries shall complete without deadlock or crash
And all results shall be correct
```

### AC-THREAD-002: GIL Release During Execution

```gherkin
Given an open Database
When a long-running query is executing
Then other Python threads shall be able to make progress (GIL released)
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

## AC-STUB: Type Stubs

### AC-STUB-001: mypy Strict Validation

```gherkin
Given the _cypherlite.pyi type stub file
When `mypy --strict tests/test_database.py` is executed
Then it shall pass with no errors
```

### AC-STUB-002: Type Completeness

```gherkin
Given the type stub file
When inspecting its contents
Then every public class shall have type annotations for all methods
And every function parameter shall have a type annotation
And every return type shall be annotated
```

---

## AC-PACKAGE: Python Package

### AC-PACKAGE-001: pip Install from Wheel

```gherkin
Given a wheel built by `maturin build --release`
When `pip install cypherlite-*.whl` is executed
Then `import cypherlite` shall succeed
And `cypherlite.version()` shall return a valid version string
```

### AC-PACKAGE-002: maturin develop

```gherkin
Given the crates/cypherlite-python directory
When `maturin develop` is executed in a virtualenv
Then `import cypherlite` shall succeed
And all pytest tests shall pass
```

---

## AC-TEST: Test Coverage

### AC-TEST-001: Rust Unit Tests

```gherkin
Given the cypherlite-python crate
When `cargo test -p cypherlite-python` is executed
Then all Rust unit tests shall pass
```

### AC-TEST-002: pytest Integration Tests

```gherkin
Given a maturin develop build
When `pytest tests/ -v` is executed
Then all Python integration tests shall pass
```

### AC-TEST-003: Complete Lifecycle Test

```gherkin
Given a clean temporary directory
When the complete lifecycle is executed:
  db = cypherlite.open(tmp_path / "test.cyl")
  db.execute("CREATE (n:Person {name: 'Alice', age: 30})")
  result = db.execute("MATCH (n:Person) RETURN n.name, n.age")
  assert result.columns == ["n.name", "n.age"]
  assert len(result) == 1
  for row in result:
      assert row["n.name"] == "Alice"
      assert row["n.age"] == 30
  db.close()
Then all assertions shall pass
```

---

## AC-NFR: Non-Functional Requirements

### AC-NFR-001: Multi-Platform Build

```gherkin
Given the cypherlite-python crate
When built on Linux x86_64, macOS aarch64, and Windows x86_64
Then `maturin build` shall succeed on all platforms
And pytest tests shall pass on all platforms
```

### AC-NFR-002: Python Version Compatibility

```gherkin
Given a wheel built with abi3-py38
When installed on Python 3.8, 3.10, 3.12, and 3.13
Then import and basic operations shall succeed on all versions
```

### AC-NFR-003: Idiomatic Python API

```gherkin
Given the cypherlite Python API
When reviewed against PEP 8 and Python conventions
Then context managers shall be supported for resource lifecycle
And iteration shall use __iter__/__next__
And subscript access shall use __getitem__
And optional parameters shall use keyword arguments
And errors shall use the Python exception hierarchy
```

---

## pytest Test Scenarios Summary

| Test File               | Scenarios                                              | Count |
| ----------------------- | ------------------------------------------------------ | ----- |
| `test_database.py`      | open, close, context manager, config, closed state     | ~10   |
| `test_query.py`         | simple, parameterized, CRUD, empty result, errors      | ~12   |
| `test_transaction.py`   | commit, rollback, context manager, conflict, closed    | ~10   |
| `test_result.py`        | columns, len, getitem, negative index, iter, repr      | ~10   |
| `test_values.py`        | all type conversions, roundtrip, edge cases, list       | ~15   |
| `test_errors.py`        | hierarchy, attributes, isinstance, TypeError            | ~8    |
| **Total**               |                                                        | **~65** |
