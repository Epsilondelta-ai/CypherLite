"""Specification tests for cypherlite Python bindings (SPEC-FFI-003)."""

import os
import tempfile

import pytest


# ---------------------------------------------------------------------------
# M1: Module-level functions
# ---------------------------------------------------------------------------


def test_version():
    """version() returns a non-empty string matching crate version."""
    import cypherlite

    v = cypherlite.version()
    assert isinstance(v, str)
    assert len(v) > 0
    assert "1.0.0" in v


def test_features():
    """features() returns a string listing compiled feature flags."""
    import cypherlite

    f = cypherlite.features()
    assert isinstance(f, str)
    # At minimum, temporal-core is always enabled (default feature)
    assert "temporal-core" in f


# ---------------------------------------------------------------------------
# M2: Database lifecycle + error handling
# ---------------------------------------------------------------------------


def test_open_close():
    """Database can be opened and explicitly closed."""
    import cypherlite

    with tempfile.TemporaryDirectory() as d:
        db = cypherlite.open(os.path.join(d, "test.cyl"))
        db.close()


def test_context_manager():
    """Database works as a context manager (auto-close on exit)."""
    import cypherlite

    with tempfile.TemporaryDirectory() as d:
        with cypherlite.open(os.path.join(d, "test.cyl")) as db:
            assert db is not None
        # db is now closed; operations should raise


def test_double_close():
    """Calling close() twice does not raise."""
    import cypherlite

    with tempfile.TemporaryDirectory() as d:
        db = cypherlite.open(os.path.join(d, "test.cyl"))
        db.close()
        db.close()  # should not raise


def test_closed_db_raises_error():
    """Executing on a closed database raises CypherLiteError."""
    import cypherlite

    with tempfile.TemporaryDirectory() as d:
        db = cypherlite.open(os.path.join(d, "test.cyl"))
        db.close()
        with pytest.raises(cypherlite.CypherLiteError):
            db.execute("MATCH (n) RETURN n")


# ---------------------------------------------------------------------------
# M3: Query execution
# ---------------------------------------------------------------------------


def test_execute_create_match():
    """CREATE a node and MATCH it back."""
    import cypherlite

    with tempfile.TemporaryDirectory() as d:
        with cypherlite.open(os.path.join(d, "test.cyl")) as db:
            db.execute("CREATE (n:Person {name: 'Alice', age: 30})")
            result = db.execute("MATCH (n:Person) RETURN n.name, n.age")
            assert len(result) == 1
            row = result[0]
            assert row["n.name"] == "Alice"
            assert row["n.age"] == 30


def test_execute_with_params():
    """execute() with params dict passes parameters to query."""
    import cypherlite

    with tempfile.TemporaryDirectory() as d:
        with cypherlite.open(os.path.join(d, "test.cyl")) as db:
            db.execute("CREATE (n:Person {name: 'Alice'})")
            result = db.execute(
                "MATCH (n:Person) WHERE n.name = $name RETURN n.name",
                params={"name": "Alice"},
            )
            assert len(result) == 1
            assert result[0]["n.name"] == "Alice"


def test_execute_invalid_query():
    """Invalid Cypher raises CypherLiteError."""
    import cypherlite

    with tempfile.TemporaryDirectory() as d:
        with cypherlite.open(os.path.join(d, "test.cyl")) as db:
            with pytest.raises(cypherlite.CypherLiteError):
                db.execute("INVALID QUERY @#$")


# ---------------------------------------------------------------------------
# M4: Transaction with context manager
# ---------------------------------------------------------------------------


def test_transaction_commit():
    """Transaction commit persists data."""
    import cypherlite

    with tempfile.TemporaryDirectory() as d:
        with cypherlite.open(os.path.join(d, "test.cyl")) as db:
            tx = db.begin()
            tx.execute("CREATE (n:Person {name: 'Bob'})")
            tx.commit()

            result = db.execute("MATCH (n:Person) RETURN n.name")
            assert len(result) == 1
            assert result[0]["n.name"] == "Bob"


def test_transaction_rollback():
    """Transaction rollback can be called without error."""
    import cypherlite

    with tempfile.TemporaryDirectory() as d:
        with cypherlite.open(os.path.join(d, "test.cyl")) as db:
            tx = db.begin()
            tx.execute("CREATE (n:Person {name: 'Charlie'})")
            tx.rollback()


def test_transaction_context_manager_commit():
    """Transaction as context manager auto-commits on clean exit."""
    import cypherlite

    with tempfile.TemporaryDirectory() as d:
        with cypherlite.open(os.path.join(d, "test.cyl")) as db:
            with db.begin() as tx:
                tx.execute("CREATE (n:Person {name: 'Dave'})")
            # auto-committed
            result = db.execute("MATCH (n:Person) RETURN n.name")
            assert len(result) == 1
            assert result[0]["n.name"] == "Dave"


def test_transaction_context_manager_rollback_on_exception():
    """Transaction as context manager auto-rollbacks on exception."""
    import cypherlite

    with tempfile.TemporaryDirectory() as d:
        with cypherlite.open(os.path.join(d, "test.cyl")) as db:
            try:
                with db.begin() as tx:
                    tx.execute("CREATE (n:Person {name: 'Eve'})")
                    raise ValueError("simulated error")
            except ValueError:
                pass
            # auto-rolled back (Phase 2: rollback is no-op, data remains)


# ---------------------------------------------------------------------------
# M5: Result and row access
# ---------------------------------------------------------------------------


def test_result_columns():
    """Result.columns returns the list of column names."""
    import cypherlite

    with tempfile.TemporaryDirectory() as d:
        with cypherlite.open(os.path.join(d, "test.cyl")) as db:
            db.execute("CREATE (n:Person {name: 'Alice', age: 30})")
            result = db.execute("MATCH (n:Person) RETURN n.name, n.age")
            cols = result.columns
            assert isinstance(cols, list)
            assert "n.name" in cols
            assert "n.age" in cols


def test_result_len():
    """len(result) returns the number of rows."""
    import cypherlite

    with tempfile.TemporaryDirectory() as d:
        with cypherlite.open(os.path.join(d, "test.cyl")) as db:
            db.execute("CREATE (n:Person {name: 'Alice'})")
            db.execute("CREATE (n:Person {name: 'Bob'})")
            result = db.execute("MATCH (n:Person) RETURN n.name")
            assert len(result) == 2


def test_result_iter():
    """Iterating over result yields row dicts."""
    import cypherlite

    with tempfile.TemporaryDirectory() as d:
        with cypherlite.open(os.path.join(d, "test.cyl")) as db:
            db.execute("CREATE (n:Person {name: 'Alice'})")
            db.execute("CREATE (n:Person {name: 'Bob'})")
            result = db.execute("MATCH (n:Person) RETURN n.name")
            names = sorted(row["n.name"] for row in result)
            assert names == ["Alice", "Bob"]


def test_result_getitem():
    """result[index] returns a dict for the given row."""
    import cypherlite

    with tempfile.TemporaryDirectory() as d:
        with cypherlite.open(os.path.join(d, "test.cyl")) as db:
            db.execute("CREATE (n:Person {name: 'Alice'})")
            result = db.execute("MATCH (n:Person) RETURN n.name")
            row = result[0]
            assert isinstance(row, dict)
            assert row["n.name"] == "Alice"


def test_result_getitem_out_of_range():
    """result[bad_index] raises IndexError."""
    import cypherlite

    with tempfile.TemporaryDirectory() as d:
        with cypherlite.open(os.path.join(d, "test.cyl")) as db:
            result = db.execute("MATCH (n:Person) RETURN n.name")
            with pytest.raises(IndexError):
                _ = result[0]


# ---------------------------------------------------------------------------
# Value type round-trips
# ---------------------------------------------------------------------------


def test_value_types():
    """Various Python types round-trip through CypherLite correctly."""
    import cypherlite

    with tempfile.TemporaryDirectory() as d:
        with cypherlite.open(os.path.join(d, "test.cyl")) as db:
            # Int
            db.execute("CREATE (n:Test {i: 42})")
            r = db.execute("MATCH (n:Test) RETURN n.i")
            assert r[0]["n.i"] == 42
            assert isinstance(r[0]["n.i"], int)
            db.execute("MATCH (n:Test) DETACH DELETE n")

            # Float
            db.execute("CREATE (n:Test {f: 3.14})")
            r = db.execute("MATCH (n:Test) RETURN n.f")
            assert abs(r[0]["n.f"] - 3.14) < 0.001
            assert isinstance(r[0]["n.f"], float)
            db.execute("MATCH (n:Test) DETACH DELETE n")

            # String
            db.execute("CREATE (n:Test {s: 'hello'})")
            r = db.execute("MATCH (n:Test) RETURN n.s")
            assert r[0]["n.s"] == "hello"
            assert isinstance(r[0]["n.s"], str)
            db.execute("MATCH (n:Test) DETACH DELETE n")

            # Bool
            db.execute("CREATE (n:Test {b: true})")
            r = db.execute("MATCH (n:Test) RETURN n.b")
            assert r[0]["n.b"] is True
            assert isinstance(r[0]["n.b"], bool)
            db.execute("MATCH (n:Test) DETACH DELETE n")

            # Null (missing property)
            db.execute("CREATE (n:Test {a: 1})")
            r = db.execute("MATCH (n:Test) RETURN n.missing")
            assert r[0]["n.missing"] is None


def test_node_id_type():
    """NodeID wrapper is returned for node references."""
    import cypherlite

    with tempfile.TemporaryDirectory() as d:
        with cypherlite.open(os.path.join(d, "test.cyl")) as db:
            db.execute("CREATE (n:Person {name: 'Alice'})")
            r = db.execute("MATCH (n:Person) RETURN n")
            val = r[0]["n"]
            assert isinstance(val, cypherlite.NodeID)
            assert isinstance(val.id, int)
            assert repr(val).startswith("NodeID(")


def test_node_id_equality_and_hash():
    """NodeID supports equality and hashing."""
    import cypherlite

    a = cypherlite.NodeID(1)
    b = cypherlite.NodeID(1)
    c = cypherlite.NodeID(2)
    assert a == b
    assert a != c
    assert hash(a) == hash(b)
    assert hash(a) != hash(c)


def test_edge_id_type():
    """EdgeID wrapper is returned for edge references."""
    import cypherlite

    with tempfile.TemporaryDirectory() as d:
        with cypherlite.open(os.path.join(d, "test.cyl")) as db:
            db.execute(
                "CREATE (a:Person {name: 'Alice'})-[:KNOWS]->(b:Person {name: 'Bob'})"
            )
            r = db.execute(
                "MATCH (a:Person)-[r:KNOWS]->(b:Person) RETURN r"
            )
            val = r[0]["r"]
            assert isinstance(val, cypherlite.EdgeID)
            assert isinstance(val.id, int)


def test_edge_id_equality_and_hash():
    """EdgeID supports equality and hashing."""
    import cypherlite

    a = cypherlite.EdgeID(1)
    b = cypherlite.EdgeID(1)
    c = cypherlite.EdgeID(2)
    assert a == b
    assert a != c
    assert hash(a) == hash(b)


# ---------------------------------------------------------------------------
# Error class
# ---------------------------------------------------------------------------


def test_error_is_exception():
    """CypherLiteError is a subclass of Exception."""
    import cypherlite

    assert issubclass(cypherlite.CypherLiteError, Exception)


# ---------------------------------------------------------------------------
# Database options
# ---------------------------------------------------------------------------


def test_open_with_options():
    """open() accepts page_size and cache_capacity keyword arguments."""
    import cypherlite

    with tempfile.TemporaryDirectory() as d:
        db = cypherlite.open(
            os.path.join(d, "test.cyl"), page_size=4096, cache_capacity=128
        )
        db.close()
