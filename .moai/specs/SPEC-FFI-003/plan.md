# SPEC-FFI-003: Implementation Plan

| Field     | Value                                   |
| --------- | --------------------------------------- |
| SPEC      | SPEC-FFI-003                            |
| Title     | Python Bindings for CypherLite via PyO3 |
| Mode      | TDD (RED-GREEN-REFACTOR)                |
| Crate     | cypherlite-python                       |

---

## Milestones

### M1: Crate Setup and Build Infrastructure (Primary Goal)

**Objective**: Establish `cypherlite-python` crate with PyO3 + maturin build pipeline.

**Tasks**:
1. Create `crates/cypherlite-python/Cargo.toml`
   - `[lib]` with `crate-type = ["cdylib"]`, `name = "_cypherlite"`
   - `pyo3` dependency with `abi3-py38`, `extension-module` features
   - Feature flag re-export from `cypherlite-query`
2. Create `crates/cypherlite-python/pyproject.toml`
   - `[build-system]` with `maturin` backend
   - `[project]` metadata: name=`cypherlite`, requires-python=`>=3.8`
   - `[tool.maturin]` with `python-source = "python"`, `module-name = "cypherlite._cypherlite"`
3. Create `crates/cypherlite-python/python/cypherlite/__init__.py`
   - Re-export public API from `_cypherlite`
4. Create `crates/cypherlite-python/python/cypherlite/py.typed` (empty PEP 561 marker)
5. Create `crates/cypherlite-python/src/lib.rs`
   - `#[pymodule]` function for `_cypherlite`
   - `version()` and `features()` module-level functions
6. Add `crates/cypherlite-python` to workspace `Cargo.toml` members
7. Verify `maturin develop` builds and `import cypherlite` succeeds

**Requirements**: REQ-FFI-PY-001 ~ REQ-FFI-PY-006, REQ-FFI-PY-090 ~ REQ-FFI-PY-091

**Acceptance**: `maturin develop` succeeds, `cypherlite.version()` returns `"1.1.0"`, `cypherlite.features()` returns feature string.

---

### M2: Database Lifecycle and Context Manager (Primary Goal)

**Objective**: Implement `Database` PyO3 class with open/close/context manager.

**Tasks**:
1. Create `src/database.rs`
   - `#[pyclass] struct Database` wrapping `Mutex<Option<CypherLite>>`
   - `#[new]` constructor (private; users call `cypherlite.open()`)
   - `close(&self)` method: takes Mutex, drops inner `CypherLite`, sets to `None`
   - `__enter__` / `__exit__` for context manager protocol
   - Closed-state check on all methods (raises `CypherLiteError`)
2. Create module-level `open(path, *, page_size=None, cache_capacity=None) -> Database` function
   - Construct `DatabaseConfig` from kwargs
   - Call `CypherLite::open(config)` with GIL released
   - Wrap result in `Database`
3. Implement thread safety: `Mutex<Option<CypherLite>>` with `py.allow_threads` for GIL release
4. RED: Write pytest tests for open/close/context manager lifecycle
5. GREEN: Make tests pass
6. REFACTOR: Clean up error conversion

**Requirements**: REQ-FFI-PY-010 ~ REQ-FFI-PY-015, REQ-FFI-PY-080

**Acceptance**: Database open/close works, context manager auto-closes, closed state detected and raises error.

---

### M3: Query Execution and Error Handling (Primary Goal)

**Objective**: Implement `execute()` and `execute(query, params={...})` with error mapping.

**Tasks**:
1. Create `src/error.rs`
   - Define `CypherLiteError` Python exception class via `create_exception!`
   - Define subclass exceptions: `ParseError`, `ExecutionError`, `TransactionConflictError`, `IoError`
   - Implement `From<cypherlite_core::CypherLiteError>` for PyErr conversion
   - Register exceptions in `#[pymodule]`
2. Create `src/value.rs`
   - `rust_value_to_python(py, &Value) -> PyObject` conversion function
   - `python_to_rust_value(obj: &Bound<PyAny>) -> PyResult<Value>` conversion function
   - Handle all base type conversions (None, bool, int, float, str, bytes, list, datetime)
3. Implement `Database.execute(query: &str) -> Result` in `src/database.rs`
   - Acquire Mutex, release GIL during `CypherLite::execute()`
   - Wrap `QueryResult` into Python `Result` object
4. Implement `Database.execute(query: &str, params: Option<HashMap<String, PyObject>>) -> Result`
   - Convert Python dict values to Rust `HashMap<String, Value>`
   - Call `execute_with_params()`
5. RED: Write pytest tests for queries, parameterized queries, error cases
6. GREEN: Make tests pass
7. REFACTOR: Optimize value conversion

**Requirements**: REQ-FFI-PY-020 ~ REQ-FFI-PY-022, REQ-FFI-PY-070 ~ REQ-FFI-PY-074

**Acceptance**: Queries execute and return results, params convert correctly, errors raise correct exception types.

---

### M4: Transaction with Context Manager (Secondary Goal)

**Objective**: Implement `Transaction` class with context manager auto-commit/rollback.

**Tasks**:
1. Create `src/transaction.rs`
   - Design for Rust lifetime management: `Transaction<'_>` cannot be stored in PyO3 class directly
   - Strategy: Store `Arc<Mutex<Option<CypherLite>>>` in Transaction, use explicit begin/commit/rollback at Rust level
   - `#[pyclass] struct Transaction` wrapping transaction state
   - `execute(query)` and `execute(query, params={...})` methods
   - `commit()` and `rollback()` methods (invalidate handle)
   - `__enter__` / `__exit__` (auto-commit on clean exit, auto-rollback on exception)
   - Closed-state check on all methods
2. Implement `Database.begin() -> Transaction`
   - Return `Transaction` object bound to this database
3. RED: Write pytest tests for transaction lifecycle, context manager behavior
4. GREEN: Make tests pass
5. REFACTOR: Ensure clean error propagation in __exit__

**Requirements**: REQ-FFI-PY-030 ~ REQ-FFI-PY-036

**Acceptance**: Transactions commit/rollback correctly, context manager auto-handles, double-use raises error.

**Technical Note**: Rust `Transaction<'_>` has a lifetime tied to `&mut CypherLite`. PyO3 classes cannot store references with lifetimes. The implementation must use one of these strategies:
- Option A: Hold `Arc<Mutex<CypherLite>>` in Transaction and use explicit transaction state management at the storage engine level.
- Option B: Use `unsafe` transmute to erase the lifetime (not recommended).
- Option C: Use a pool pattern where Transaction takes ownership temporarily.

Recommended: Option A (explicit state management via Mutex).

---

### M5: Result and Row Access (Secondary Goal)

**Objective**: Implement `Result` and `Row` classes with Python protocols (iter, len, getitem).

**Tasks**:
1. Create `src/result.rs`
   - `#[pyclass] struct Result` storing columns (`Vec<String>`) and rows (converted Python data)
   - Strategy: Convert all `QueryResult` data to Python objects at construction time (eager conversion), so `Result` holds only Python-owned data
   - `columns` property -> `list[str]`
   - `__len__` -> row count
   - `__getitem__(index: int)` -> `Row` (supports negative indices)
   - `__iter__` / `__next__` -> yield `Row` objects
   - `__repr__` -> `<CypherLiteResult columns=[...] rows=N>`
2. Create `src/row.rs`
   - `#[pyclass] struct Row` storing column names and values (Python objects)
   - `__getitem__(key: str | int)` -> Python value
   - `keys()` -> `list[str]`
   - `__len__` -> column count
   - `__repr__` -> `<CypherLiteRow {...}>`
3. RED: Write pytest tests for all access patterns
4. GREEN: Make tests pass
5. REFACTOR: Optimize iteration, ensure clean repr

**Requirements**: REQ-FFI-PY-040 ~ REQ-FFI-PY-044, REQ-FFI-PY-050 ~ REQ-FFI-PY-054

**Acceptance**: Full iteration, indexing, key access, negative indices, repr all work correctly.

---

### M6: Value Type Mapping (Secondary Goal)

**Objective**: Complete bidirectional Value conversion including feature-gated types.

**Tasks**:
1. Implement `NodeID`, `EdgeID` Python classes in `src/types.rs`
   - `#[pyclass]` wrapping `u64`
   - `__repr__`, `__eq__`, `__hash__`, `__int__` methods
2. Implement `datetime` conversion in `src/value.rs`
   - `Value::DateTime(ms)` -> `datetime.datetime.fromtimestamp(ms/1000, tz=utc)`
   - `datetime.datetime` -> `Value::DateTime(timestamp_ms)`
3. Implement feature-gated types (`#[cfg(feature = "subgraph")]`):
   - `SubgraphID` class
4. Implement feature-gated types (`#[cfg(feature = "hypergraph")]`):
   - `HyperEdgeID` class
   - `TemporalNodeRef` class (wrapping node_id + timestamp)
5. Complete `python_to_rust_value()` for all parameter types including `list` (recursive), `datetime`
6. RED: Write pytest roundtrip tests for all value types
7. GREEN: Make tests pass
8. REFACTOR: DRY conversion code

**Requirements**: REQ-FFI-PY-060 ~ REQ-FFI-PY-063

**Acceptance**: All value types round-trip correctly, feature-gated types compile conditionally, unsupported types raise TypeError.

---

### M7: Type Stubs, Tests, Documentation (Final Goal)

**Objective**: Complete type stubs, comprehensive pytest suite, and documentation.

**Tasks**:
1. Create `python/cypherlite/_cypherlite.pyi`
   - Type annotations for all classes: `Database`, `Transaction`, `Result`, `Row`
   - Type annotations for module functions: `open()`, `version()`, `features()`
   - Type annotations for exception classes
   - Type annotations for ID types: `NodeID`, `EdgeID`
   - Overloaded `__getitem__` signatures for str/int keys
2. Validate stubs with `mypy --strict` against test files
3. Write comprehensive pytest suite:
   - `tests/test_database.py`: lifecycle, context manager, closed state
   - `tests/test_query.py`: simple query, parameterized query, CRUD operations
   - `tests/test_transaction.py`: commit, rollback, context manager, conflict
   - `tests/test_result.py`: columns, iteration, indexing, row access
   - `tests/test_values.py`: all type conversions, roundtrip, edge cases
   - `tests/test_errors.py`: all error types, exception hierarchy, isinstance checks
   - `tests/conftest.py`: shared fixtures (temp database, sample data)
4. Create `crates/cypherlite-python/README.md`
5. Update CI to include Python test job (`maturin develop` + `pytest`)

**Requirements**: REQ-FFI-PY-100 ~ REQ-FFI-PY-110, REQ-FFI-PY-TEST-*, REQ-FFI-PY-DOC-*, REQ-FFI-PY-NFR-*

**Acceptance**: mypy passes strict mode, all pytest tests pass, README includes examples, CI green.

---

## File Impact Analysis

### New Files

| File                                              | Purpose                          |
| ------------------------------------------------- | -------------------------------- |
| `crates/cypherlite-python/Cargo.toml`             | Crate manifest (PyO3 + abi3)     |
| `crates/cypherlite-python/pyproject.toml`         | Python build config (maturin)    |
| `crates/cypherlite-python/src/lib.rs`             | PyO3 module root                 |
| `crates/cypherlite-python/src/database.rs`        | Database class (open/close/ctx)  |
| `crates/cypherlite-python/src/transaction.rs`     | Transaction class (commit/rollback/ctx) |
| `crates/cypherlite-python/src/result.rs`          | Result class (iter/len/getitem)  |
| `crates/cypherlite-python/src/row.rs`             | Row class (getitem by name/idx)  |
| `crates/cypherlite-python/src/value.rs`           | Value conversion (Rust <-> Python) |
| `crates/cypherlite-python/src/error.rs`           | Exception class hierarchy        |
| `crates/cypherlite-python/src/types.rs`           | NodeID, EdgeID, etc. Python types |
| `crates/cypherlite-python/python/cypherlite/__init__.py` | Public API re-exports     |
| `crates/cypherlite-python/python/cypherlite/py.typed`    | PEP 561 marker            |
| `crates/cypherlite-python/python/cypherlite/_cypherlite.pyi` | Type stubs            |
| `crates/cypherlite-python/tests/test_database.py` | Database lifecycle tests         |
| `crates/cypherlite-python/tests/test_query.py`    | Query execution tests            |
| `crates/cypherlite-python/tests/test_transaction.py` | Transaction tests             |
| `crates/cypherlite-python/tests/test_result.py`   | Result/Row access tests          |
| `crates/cypherlite-python/tests/test_values.py`   | Value conversion tests           |
| `crates/cypherlite-python/tests/test_errors.py`   | Error handling tests             |
| `crates/cypherlite-python/tests/conftest.py`      | Shared pytest fixtures           |
| `crates/cypherlite-python/README.md`              | Package documentation            |

### Modified Files

| File                    | Change                                              |
| ----------------------- | --------------------------------------------------- |
| `Cargo.toml` (workspace root) | Add `"crates/cypherlite-python"` to `members` |
| `.github/workflows/ci.yml`    | Add Python binding test job                    |

### No-Change Files

- `crates/cypherlite-ffi/` (C ABI layer, not used by Python bindings)
- `crates/cypherlite-core/` (consumed as dependency, no changes needed)
- `crates/cypherlite-storage/` (consumed as dependency, no changes needed)
- `crates/cypherlite-query/` (consumed as dependency, no changes needed)
- `bindings/go/cypherlite/` (Go bindings, independent)

---

## Risk Analysis

### R1: Rust Lifetime in PyO3 (High)

**Risk**: Rust `Transaction<'_>` holds a mutable borrow of `CypherLite`. PyO3 `#[pyclass]` requires `'static` lifetime. Storing a borrowed `Transaction` inside a PyO3 class is not directly possible.

**Mitigation**: Use `Arc<Mutex<Option<CypherLite>>>` shared between `Database` and `Transaction`. Transaction manages explicit begin/commit/rollback state via the Mutex. This mirrors the pattern used in `cypherlite-ffi` (`Mutex<CypherLite>` + `AtomicBool` in-transaction flag).

### R2: GIL and Mutex Interaction (Medium)

**Risk**: Holding both the GIL and the Rust Mutex simultaneously can lead to deadlocks if Python callbacks are involved.

**Mitigation**: Always release GIL (`py.allow_threads`) before acquiring the Rust Mutex. The API is synchronous with no Python callbacks during database operations, so this pattern is safe.

### R3: Value Conversion Performance (Low)

**Risk**: Eager conversion of all `QueryResult` rows to Python objects at result construction may be slow for large result sets.

**Mitigation**: For v1.0, eager conversion is acceptable. Future optimization could implement lazy row conversion (convert on access), but this adds complexity. Document the eager approach and defer lazy conversion to v2.0.

### R4: abi3 Limitation (Low)

**Risk**: abi3 stable ABI limits access to some CPython internals. Some PyO3 features may be unavailable or slower under abi3.

**Mitigation**: PyO3's abi3 support is mature. The CypherLite Python API uses only basic PyO3 features (classes, methods, exceptions, type conversion). No advanced CPython internals are needed.

### R5: maturin Cross-Compilation (Low)

**Risk**: Building wheels for multiple platforms (Linux, macOS, Windows) and architectures requires cross-compilation infrastructure.

**Mitigation**: Use `maturin build --release` with GitHub Actions matrix for platform-specific wheels. `maturin` supports cross-compilation via `--target` flag and Docker-based manylinux builds.

---

## Architecture Design Direction

```
Python (user code)
       |
       v
cypherlite/__init__.py       # Pure Python re-exports
       |
       v
cypherlite/_cypherlite       # PyO3 native module (cdylib)
       |
       v
cypherlite-python (Rust)     # PyO3 wrapper crate
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

**Key Design Decision**: Direct Rust wrapping (not via C ABI) provides:
- Zero overhead from C ABI layer
- Rich type conversion via PyO3's `IntoPyObject`/`FromPyObject` traits
- Native Python exception support
- Automatic memory management (PyO3 handles Python reference counting)
- Simpler build pipeline (no separate C library build step)

---

## Dependencies

### Rust Dependencies (crates/cypherlite-python/Cargo.toml)

| Crate              | Version  | Features                        | Purpose              |
| ------------------ | -------- | ------------------------------- | -------------------- |
| `pyo3`             | `0.23`   | `abi3-py38`, `extension-module` | Python-Rust bindings |
| `cypherlite-query` | `path`   | (all features propagated)       | CypherLite API       |
| `cypherlite-core`  | `path`   | (all features propagated)       | Common types/errors  |

### Python Dependencies (development only)

| Package       | Version  | Purpose              |
| ------------- | -------- | -------------------- |
| `maturin`     | `>=1.8`  | Build tool           |
| `pytest`      | `>=8.0`  | Test framework       |
| `pytest-cov`  | `>=5.0`  | Coverage measurement |
| `mypy`        | `>=1.10` | Type checking        |
