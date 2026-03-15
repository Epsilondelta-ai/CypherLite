# SPEC-FFI-003: Python Bindings via PyO3

| Field     | Value                                              |
| --------- | -------------------------------------------------- |
| ID        | SPEC-FFI-003                                       |
| Title     | Python Bindings for CypherLite via PyO3            |
| Status    | Planned                                            |
| Version   | 1.0.0                                              |
| Created   | 2026-03-15                                         |
| Priority  | High                                               |
| Phase     | 12 / FFI Bindings                                  |
| Crate     | cypherlite-python                                  |
| Depends   | cypherlite-core, cypherlite-storage, cypherlite-query |

---

## 1. Environment

- **Language**: Rust 1.84+ (MSRV), Python 3.8+ (abi3 stable ABI)
- **Binding Framework**: PyO3 0.23+ (direct Rust-Python integration, C ABI bypassed)
- **Build Tool**: maturin 1.8+ (PyO3 standard build/publish tool)
- **Package Name**: `cypherlite` (PyPI), `_cypherlite` (internal native module)
- **CI**: GitHub Actions (existing 6 Rust jobs + new Python test jobs)
- **Target Platforms**: Linux (x86_64, aarch64), macOS (x86_64, aarch64), Windows (x86_64)
- **Feature Flags**: Rust feature flags (`subgraph`, `hypergraph`) propagated from `cypherlite-query`
- **Testing**: pytest 8.x, pytest-cov
- **Type Checking**: `.pyi` type stubs for IDE support (mypy, pyright compatible)
- **Workspace**: Cargo workspace at repository root (`crates/cypherlite-python/`)

## 2. Assumptions

- **A1**: SPEC-FFI-001 Assumption A2 establishes that Python bindings use PyO3 to directly wrap the Rust API, bypassing the C ABI (`cypherlite-ffi`).
- **A2**: PyO3's `abi3` feature enables building a single wheel compatible with Python 3.8+, eliminating per-version wheel builds.
- **A3**: `maturin` is the standard build backend for PyO3 projects and integrates with `pyproject.toml` (PEP 517/518).
- **A4**: The `CypherLite` struct requires `&mut self` for `execute`, `execute_with_params`, and `begin`. PyO3 wrapping will use `Mutex<CypherLite>` internally to provide safe shared access from Python.
- **A5**: Python's GIL ensures single-threaded Python execution. The Mutex exists to satisfy Rust's ownership model; GIL release (`py.allow_threads`) is used during long database operations for concurrency with other Python threads.
- **A6**: Rust `Value` enum variants map to Python native types (None, bool, int, float, str, bytes, list, datetime). NodeID/EdgeID types are exposed as dedicated Python classes.
- **A7**: Feature-gated variants (Subgraph, Hyperedge, TemporalNode) are conditionally compiled in the PyO3 module using `#[cfg(feature = "...")]`.
- **A8**: The Python package will be published to PyPI as `cypherlite`. The internal native module is `_cypherlite`, re-exported by a pure Python `cypherlite/` package.

## 3. Requirements

### 3.1 Crate and Package Setup

**REQ-FFI-PY-001** (Ubiquitous)
The `cypherlite-python` crate shall be located at `crates/cypherlite-python/` and be a member of the Cargo workspace. The crate shall produce a `cdylib` output for Python extension module loading.

**REQ-FFI-PY-002** (Ubiquitous)
The crate `Cargo.toml` shall declare `pyo3` as a dependency with `abi3-py38` and `extension-module` features enabled, ensuring a single wheel covers Python 3.8+.

**REQ-FFI-PY-003** (Ubiquitous)
The crate shall re-export all feature flags from `cypherlite-query` (e.g., `temporal-core`, `subgraph`, `hypergraph`) so that conditional compilation propagates correctly to PyO3 wrapper types.

**REQ-FFI-PY-004** (Ubiquitous)
A `pyproject.toml` shall exist at the crate root (`crates/cypherlite-python/pyproject.toml`) declaring `maturin` as the build backend, with metadata including package name `cypherlite`, Python version requirement `>=3.8`, and project classifiers.

**REQ-FFI-PY-005** (Ubiquitous)
A pure Python package `cypherlite/` shall exist at `crates/cypherlite-python/cypherlite/` containing:
- `__init__.py`: Re-exports all public API from `_cypherlite` native module
- `py.typed`: PEP 561 marker file
- `_cypherlite.pyi`: Type stub file for the native module

**REQ-FFI-PY-006** (Ubiquitous)
The `__init__.py` shall expose the following public API:
- `open(path, *, page_size=None, cache_capacity=None) -> Database`
- `version() -> str`
- `features() -> str`
- `CypherLiteError` exception class
- `NodeID`, `EdgeID` type classes

### 3.2 Database Lifecycle

**REQ-FFI-PY-010** (Event-Driven)
**When** a Python caller invokes `cypherlite.open(path)`, **then** the system shall create a `CypherLite` instance with default `DatabaseConfig` (using the provided path) and return a `Database` object, or raise `CypherLiteError` on failure.

**REQ-FFI-PY-011** (Event-Driven)
**When** a Python caller invokes `cypherlite.open(path, page_size=4096, cache_capacity=1024)`, **then** the system shall create a `CypherLite` instance with the specified configuration parameters.

**REQ-FFI-PY-012** (Event-Driven)
**When** a Python caller invokes `db.close()`, **then** the system shall drop the internal `CypherLite` instance, flushing all pending writes, and mark the `Database` handle as closed.

**REQ-FFI-PY-013** (Unwanted)
The system shall not cause a crash or segfault when `close()` is called multiple times on the same `Database` handle; subsequent calls shall be no-ops.

**REQ-FFI-PY-014** (Event-Driven)
**When** a `Database` object is used as a context manager (`with cypherlite.open(...) as db`), **then** `__enter__` shall return the `Database` itself, and `__exit__` shall call `close()` regardless of whether an exception occurred.

**REQ-FFI-PY-015** (Unwanted)
The system shall not allow operations on a closed `Database`; all methods shall raise `CypherLiteError("Database is closed")` if invoked after `close()`.

### 3.3 Query Execution

**REQ-FFI-PY-020** (Event-Driven)
**When** a Python caller invokes `db.execute(query)`, **then** the system shall acquire the internal Mutex, execute the Cypher query (releasing the GIL during execution via `py.allow_threads`), and return a `Result` object, or raise `CypherLiteError`.

**REQ-FFI-PY-021** (Event-Driven)
**When** a Python caller invokes `db.execute(query, params={"name": "Alice"})`, **then** the system shall convert the Python dict to a Rust `HashMap<String, Value>` using the type mapping (REQ-FFI-PY-060), execute the parameterized query, and return a `Result` object.

**REQ-FFI-PY-022** (Unwanted)
The system shall not accept unsupported parameter value types; it shall raise `TypeError` with a descriptive message listing the unsupported type and the supported types.

### 3.4 Transaction Support

**REQ-FFI-PY-030** (Event-Driven)
**When** a Python caller invokes `db.begin()`, **then** the system shall create a `Transaction` object wrapping the Rust `Transaction<'_>`, or raise `CypherLiteError` if another transaction is already active.

**REQ-FFI-PY-031** (Event-Driven)
**When** a `Transaction` is used as a context manager (`with db.begin() as tx`), **then**:
- `__enter__` shall return the `Transaction` itself
- `__exit__` with no exception shall call `tx.commit()`
- `__exit__` with an exception shall call `tx.rollback()`

**REQ-FFI-PY-032** (Event-Driven)
**When** a Python caller invokes `tx.execute(query)`, **then** the system shall execute the query within the transaction context and return a `Result` object.

**REQ-FFI-PY-033** (Event-Driven)
**When** a Python caller invokes `tx.execute(query, params={...})`, **then** the system shall execute the parameterized query within the transaction context.

**REQ-FFI-PY-034** (Event-Driven)
**When** a Python caller invokes `tx.commit()`, **then** the system shall commit all changes, invalidate the `Transaction` handle, and return `None`.

**REQ-FFI-PY-035** (Event-Driven)
**When** a Python caller invokes `tx.rollback()`, **then** the system shall discard all changes, invalidate the `Transaction` handle, and return `None`.

**REQ-FFI-PY-036** (Unwanted)
The system shall not allow operations on a consumed `Transaction`; `execute`, `commit`, and `rollback` shall raise `CypherLiteError("Transaction is closed")` after commit or rollback.

### 3.5 Result Access

**REQ-FFI-PY-040** (Event-Driven)
**When** a Python caller accesses `result.columns`, **then** the system shall return a `list[str]` containing the column names of the result set.

**REQ-FFI-PY-041** (Event-Driven)
**When** a Python caller invokes `len(result)`, **then** the system shall return the number of rows in the result set (`__len__` protocol).

**REQ-FFI-PY-042** (Event-Driven)
**When** a Python caller invokes `result[index]`, **then** the system shall return a `Row` object for the given index (`__getitem__` protocol), or raise `IndexError` if out of bounds. Negative indices shall be supported (Python convention).

**REQ-FFI-PY-043** (Event-Driven)
**When** a Python caller iterates `for row in result`, **then** the system shall yield `Row` objects one by one (`__iter__` / `__next__` protocol).

**REQ-FFI-PY-044** (Ubiquitous)
The `Result` object shall support `__repr__` returning a human-readable summary: `<CypherLiteResult columns=[...] rows=N>`.

### 3.6 Row Value Access

**REQ-FFI-PY-050** (Event-Driven)
**When** a Python caller invokes `row["column_name"]`, **then** the system shall return the value for the named column converted to a Python type (`__getitem__` with `str` key), or raise `KeyError` if the column does not exist.

**REQ-FFI-PY-051** (Event-Driven)
**When** a Python caller invokes `row[index]`, **then** the system shall return the value at the given column index (`__getitem__` with `int` key), or raise `IndexError` if out of bounds.

**REQ-FFI-PY-052** (Event-Driven)
**When** a Python caller invokes `row.keys()`, **then** the system shall return a `list[str]` of column names for this row.

**REQ-FFI-PY-053** (Ubiquitous)
The `Row` object shall support `__repr__` returning a human-readable summary: `<CypherLiteRow {col1: val1, col2: val2, ...}>`.

**REQ-FFI-PY-054** (Ubiquitous)
The `Row` object shall support `__len__` returning the number of columns.

### 3.7 Value Type Mapping

**REQ-FFI-PY-060** (Ubiquitous)
The system shall map Rust `Value` enum variants to Python types as follows:

| Rust Value Variant     | Python Type          | Notes                                    |
| ---------------------- | -------------------- | ---------------------------------------- |
| `Value::Null`          | `None`               |                                          |
| `Value::Bool(b)`       | `bool`               |                                          |
| `Value::Int64(i)`      | `int`                |                                          |
| `Value::Float64(f)`    | `float`              |                                          |
| `Value::String(s)`     | `str`                |                                          |
| `Value::Bytes(b)`      | `bytes`              |                                          |
| `Value::List(l)`       | `list`               | Recursive conversion of elements         |
| `Value::Node(id)`      | `NodeID(int)`        | Custom Python class wrapping `u64`       |
| `Value::Edge(id)`      | `EdgeID(int)`        | Custom Python class wrapping `u64`       |
| `Value::DateTime(dt)`  | `datetime.datetime`  | Converted from ms-since-epoch to UTC     |

**REQ-FFI-PY-061** (State-Driven)
**While** the `subgraph` feature is enabled, the system shall additionally support:

| Rust Value Variant          | Python Type         |
| --------------------------- | ------------------- |
| `Value::Subgraph(id)`       | `SubgraphID(int)`   |

**REQ-FFI-PY-062** (State-Driven)
**While** the `hypergraph` feature is enabled, the system shall additionally support:

| Rust Value Variant             | Python Type              |
| ------------------------------ | ------------------------ |
| `Value::Hyperedge(id)`         | `HyperEdgeID(int)`       |
| `Value::TemporalNode(id, ts)` | `TemporalNodeRef(int, int)` |

**REQ-FFI-PY-063** (Ubiquitous)
The Python-to-Rust parameter conversion for `execute(query, params={...})` shall support the following Python types:

| Python Type         | Rust Value                    |
| ------------------- | ----------------------------- |
| `None`              | `Value::Null`                 |
| `bool`              | `Value::Bool`                 |
| `int`               | `Value::Int64`                |
| `float`             | `Value::Float64`              |
| `str`               | `Value::String`               |
| `bytes`             | `Value::Bytes`                |
| `list`              | `Value::List` (recursive)     |
| `datetime.datetime` | `Value::DateTime` (to ms UTC) |

### 3.8 Error Handling

**REQ-FFI-PY-070** (Ubiquitous)
The system shall define a Python exception class `CypherLiteError` inheriting from `Exception`, exposed in the `cypherlite` module.

**REQ-FFI-PY-071** (Ubiquitous)
The `CypherLiteError` exception shall carry:
- `message` attribute: human-readable error description (from `CypherLiteError::to_string()`)
- `code` attribute: string error code categorizing the error type (e.g., `"IoError"`, `"ParseError"`, `"TransactionConflict"`)

**REQ-FFI-PY-072** (Event-Driven)
**When** a Rust `CypherLiteError` is returned from any API call, the system shall convert it to a Python `CypherLiteError` exception with the appropriate message and code, and raise it.

**REQ-FFI-PY-073** (Ubiquitous)
The system shall define specific exception subclasses for common error categories:

| Exception Class              | Parent            | Rust Error Variant        |
| ---------------------------- | ----------------- | ------------------------- |
| `CypherLiteError`            | `Exception`       | (base class)              |
| `ParseError`                 | `CypherLiteError` | `ParseError`              |
| `ExecutionError`             | `CypherLiteError` | `ExecutionError`          |
| `TransactionConflictError`   | `CypherLiteError` | `TransactionConflict`     |
| `IoError`                    | `CypherLiteError` | `IoError`                 |

**REQ-FFI-PY-074** (Event-Driven)
**When** a Python caller passes an invalid argument type to any method, the system shall raise standard Python `TypeError` (not `CypherLiteError`).

### 3.9 Thread Safety

**REQ-FFI-PY-080** (Ubiquitous)
The `Database` object shall use `Mutex<CypherLite>` internally to ensure safe access from multiple Python threads. The GIL shall be released (`py.allow_threads`) during Mutex acquisition and database operations, allowing other Python threads to execute.

**REQ-FFI-PY-081** (Ubiquitous)
The `Transaction` object shall NOT be safe to share between threads. It shall be documented as single-thread only.

**REQ-FFI-PY-082** (Ubiquitous)
The `Result` and `Row` objects shall be safe to use from any thread after creation (they hold only Python-owned data, no Rust references).

### 3.10 Library Info Functions

**REQ-FFI-PY-090** (Event-Driven)
**When** a Python caller invokes `cypherlite.version()`, **then** the system shall return the CypherLite version string (e.g., `"1.1.0"`).

**REQ-FFI-PY-091** (Event-Driven)
**When** a Python caller invokes `cypherlite.features()`, **then** the system shall return a comma-separated string of enabled Rust feature flags (e.g., `"temporal-core,subgraph"`).

### 3.11 Type Stubs

**REQ-FFI-PY-100** (Ubiquitous)
The package shall include a `_cypherlite.pyi` type stub file providing complete type annotations for all classes, methods, and functions in the native module.

**REQ-FFI-PY-101** (Ubiquitous)
The type stubs shall be compatible with mypy `--strict` mode and pyright.

**REQ-FFI-PY-102** (Ubiquitous)
The package shall include a `py.typed` marker file (PEP 561) to signal type checker support.

### 3.12 Python Package Structure

**REQ-FFI-PY-110** (Ubiquitous)
The final installed Python package structure shall be:

```
cypherlite/
    __init__.py          # Re-exports from _cypherlite
    py.typed             # PEP 561 marker
    _cypherlite.pyi      # Type stubs for native module
    _cypherlite.*.so     # Native extension (maturin-generated name)
```

---

## 4. Non-Functional Requirements

**REQ-FFI-PY-NFR-001** (Performance)
PyO3 function call overhead shall not exceed 1 microsecond per call (excluding the actual database operation). GIL release during database operations shall enable concurrent Python thread execution.

**REQ-FFI-PY-NFR-002** (Memory)
The PyO3 layer shall not perform hidden heap allocations beyond those documented in the API. Rust-owned data shall be converted to Python-owned objects at return time; no dangling references across the FFI boundary.

**REQ-FFI-PY-NFR-003** (Safety)
Every `unsafe` block in the PyO3 crate shall have a `// SAFETY:` comment documenting the invariants that make the operation sound. The goal is zero `unsafe` usage, as PyO3 handles the raw FFI boundary.

**REQ-FFI-PY-NFR-004** (Compatibility)
The package shall build and pass tests on Linux (x86_64), macOS (x86_64, aarch64), and Windows (x86_64) with Python 3.8, 3.10, 3.12, and 3.13.

**REQ-FFI-PY-NFR-005** (Idiomatic Python)
The API shall follow Python conventions: context managers for resource lifecycle, iterator protocol for result traversal, subscript access for indexing, keyword arguments for optional parameters.

**REQ-FFI-PY-NFR-006** (Install)
The package shall be installable via `pip install cypherlite` (from built wheel) and `maturin develop` (for development).

---

## 5. Testing Requirements

**REQ-FFI-PY-TEST-001** (Ubiquitous)
The crate shall include Rust unit tests verifying PyO3 wrapper correctness (value conversion, error mapping, Mutex behavior).

**REQ-FFI-PY-TEST-002** (Ubiquitous)
The package shall include pytest integration tests (`tests/`) exercising the complete API lifecycle: `open -> execute -> iterate results -> close`.

**REQ-FFI-PY-TEST-003** (Ubiquitous)
pytest tests shall cover all error conditions: closed database, closed transaction, invalid parameter types, parse errors, execution errors, transaction conflicts.

**REQ-FFI-PY-TEST-004** (Ubiquitous)
pytest tests shall verify that all Value type conversions round-trip correctly (Python -> Rust -> Python).

**REQ-FFI-PY-TEST-005** (Ubiquitous)
pytest tests shall verify context manager behavior: auto-close on normal exit, auto-close on exception, auto-commit on normal exit, auto-rollback on exception.

**REQ-FFI-PY-TEST-006** (Ubiquitous)
pytest tests shall verify the iterator protocol: `for row in result`, `len(result)`, `result[i]`, `row["col"]`, `row[0]`.

**REQ-FFI-PY-TEST-007** (Event-Driven)
**When** the `subgraph` or `hypergraph` features are enabled during build, pytest tests shall verify the corresponding feature-gated value types.

**REQ-FFI-PY-TEST-008** (Ubiquitous)
Type stubs shall be validated using `mypy --strict` against the test files to ensure annotation completeness and correctness.

---

## 6. Documentation Requirements

**REQ-FFI-PY-DOC-001** (Ubiquitous)
All PyO3 `#[pyclass]` and `#[pymethods]` shall include Python docstrings describing parameters, return values, exceptions, and usage examples.

**REQ-FFI-PY-DOC-002** (Ubiquitous)
The crate shall include a `README.md` with:
- Installation instructions (`pip install cypherlite` and `maturin develop`)
- Quick start example (open, query, iterate, close)
- Context manager examples (database and transaction)
- Parameter binding example
- Value type mapping table

---

## 7. Out of Scope

- Node.js bindings via napi-rs (future SPEC-FFI-004)
- Async Python API (asyncio integration; synchronous API only in v1.0)
- Plugin system exposure (ScalarFunction, IndexPlugin, Serializer, Trigger)
- StorageEngine direct access (consumers use CypherLite facade only)
- Connection pooling or ORM-like abstractions
- Publishing to PyPI (build infrastructure only; publish is a separate workflow)
- C ABI interaction (this SPEC bypasses cypherlite-ffi entirely)

---

## 8. Traceability

| Requirement           | Plan Milestone | Acceptance Criteria         |
| --------------------- | -------------- | --------------------------- |
| REQ-FFI-PY-001~006    | M1             | AC-SETUP-*                  |
| REQ-FFI-PY-010~015    | M2             | AC-LIFECYCLE-*              |
| REQ-FFI-PY-020~022    | M3             | AC-QUERY-*                  |
| REQ-FFI-PY-030~036    | M4             | AC-TX-*                     |
| REQ-FFI-PY-040~044    | M5             | AC-RESULT-*                 |
| REQ-FFI-PY-050~054    | M5             | AC-ROW-*                    |
| REQ-FFI-PY-060~063    | M6             | AC-VALUE-*                  |
| REQ-FFI-PY-070~074    | M3             | AC-ERROR-*                  |
| REQ-FFI-PY-080~082    | M2             | AC-THREAD-*                 |
| REQ-FFI-PY-090~091    | M1             | AC-INFO-*                   |
| REQ-FFI-PY-100~102    | M7             | AC-STUB-*                   |
| REQ-FFI-PY-110        | M7             | AC-PACKAGE-*                |
| REQ-FFI-PY-TEST-*     | M7             | AC-TEST-*                   |
| REQ-FFI-PY-DOC-*      | M7             | AC-DOC-*                    |
| REQ-FFI-PY-NFR-*      | M7             | AC-NFR-*                    |
