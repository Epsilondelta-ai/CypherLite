#!/usr/bin/env python3
"""
python_quickstart.py -- Demonstrates CypherLite usage from Python via PyO3 bindings.

This is an ILLUSTRATIVE example showing the Python API surface. To run it,
you must first build and install the cypherlite Python package:

    cd crates/cypherlite-python
    maturin develop --release
    python examples/python_quickstart.py

Prerequisites:
    - Python 3.8+
    - maturin (`pip install maturin`)
    - Rust toolchain

The cypherlite package provides:
    - cypherlite.open(path) -> Database
    - Database.execute(query, params=None) -> Result
    - Database.begin() -> Transaction
    - Database.close()
    - Result: iterable rows with column access
    - cypherlite.version() -> str
    - cypherlite.features() -> str
"""

import os
import tempfile

import cypherlite


def main():
    print("=== CypherLite Python Quickstart ===\n")

    # Print version and compiled features
    print(f"Version:  {cypherlite.version()}")
    print(f"Features: {cypherlite.features()}\n")

    # Open a database in a temporary directory
    tmp_dir = tempfile.mkdtemp()
    db_path = os.path.join(tmp_dir, "quickstart.cyl")
    db = cypherlite.open(db_path)

    # -- CREATE nodes --------------------------------------------------------
    print("1. Creating nodes...")
    db.execute("CREATE (a:Person {name: 'Alice', age: 30})")
    db.execute("CREATE (b:Person {name: 'Bob', age: 25})")
    print("   Created Alice and Bob\n")

    # -- CREATE relationship -------------------------------------------------
    print("2. Creating relationship...")
    db.execute(
        "MATCH (a:Person {name: 'Alice'}), (b:Person {name: 'Bob'}) "
        "CREATE (a)-[:KNOWS {since: 2023}]->(b)"
    )
    print("   Alice -[:KNOWS]-> Bob\n")

    # -- MATCH + RETURN: read nodes ------------------------------------------
    print("3. Querying all persons...")
    result = db.execute("MATCH (n:Person) RETURN n.name, n.age")
    for row in result:
        print(f"   {row['n.name']} (age: {row['n.age']})")
    print()

    # -- Parameterized query -------------------------------------------------
    print("4. Parameterized query: find person by name...")
    result = db.execute(
        "MATCH (n:Person {name: $name}) RETURN n.name, n.age",
        params={"name": "Alice"},
    )
    for row in result:
        print(f"   Found: {row['n.name']}, age {row['n.age']}")
    print()

    # -- UPDATE with SET -----------------------------------------------------
    print("5. Updating Bob's age...")
    db.execute("MATCH (b:Person {name: 'Bob'}) SET b.age = 26")
    result = db.execute("MATCH (b:Person {name: 'Bob'}) RETURN b.age")
    for row in result:
        print(f"   Bob's new age: {row['b.age']}")
    print()

    # -- Transaction example -------------------------------------------------
    print("6. Transaction example...")
    tx = db.begin()
    tx.execute("CREATE (c:Person {name: 'Carol', age: 28})")
    tx.commit()
    result = db.execute("MATCH (n:Person) RETURN n.name")
    names = [row["n.name"] for row in result]
    print(f"   After commit: {names}\n")

    # -- DELETE --------------------------------------------------------------
    print("7. Deleting Carol...")
    db.execute("MATCH (c:Person {name: 'Carol'}) DETACH DELETE c")
    result = db.execute("MATCH (n:Person) RETURN n.name")
    names = [row["n.name"] for row in result]
    print(f"   Remaining: {names}\n")

    # -- Cleanup -------------------------------------------------------------
    db.close()
    print("=== Done! ===")


if __name__ == "__main__":
    main()
