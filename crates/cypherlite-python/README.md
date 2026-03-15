<p align="center">
  <img src="https://raw.githubusercontent.com/Epsilondelta-ai/CypherLite/main/assets/logo.png" alt="CypherLite" width="120">
</p>

# CypherLite

[![PyPI](https://img.shields.io/pypi/v/cypherlite.svg)](https://pypi.org/project/cypherlite/)
[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](https://github.com/Epsilondelta-ai/CypherLite)

> SQLite-like simplicity for graph databases.

**CypherLite** is a lightweight, embedded, single-file graph database engine written in Rust with Python bindings via PyO3. Zero-config, ACID-compliant, with native Cypher query support.

## Installation

```bash
pip install cypherlite
```

Pre-built wheels are available for:
- Linux (x86_64, aarch64)
- macOS (x86_64, arm64)
- Windows (x86_64)

## Quick Start

```python
import cypherlite

# Open (or create) a database
db = cypherlite.open("my_graph.cyl")

# Create nodes and relationships
db.execute("CREATE (a:Person {name: 'Alice', age: 30})")
db.execute("CREATE (b:Person {name: 'Bob', age: 25})")
db.execute("""
    MATCH (a:Person {name: 'Alice'}), (b:Person {name: 'Bob'})
    CREATE (a)-[:KNOWS {since: 2024}]->(b)
""")

# Query the graph
result = db.execute("MATCH (p:Person) RETURN p.name, p.age ORDER BY p.age")
for row in result:
    print(f"{row['p.name']}: {row['p.age']}")

# Parameterized queries
result = db.execute(
    "MATCH (p:Person) WHERE p.name = $name RETURN p.age",
    params={"name": "Alice"}
)

# Transactions
tx = db.begin()
tx.execute("CREATE (c:Person {name: 'Charlie', age: 35})")
tx.commit()

db.close()
```

## Features

- **ACID Transactions** with Write-Ahead Logging
- **Cypher Queries**: CREATE, MATCH, SET, DELETE, MERGE, WITH, ORDER BY, LIMIT
- **Temporal Queries**: AT TIME, BETWEEN TIME for point-in-time lookups
- **Subgraph Snapshots**: CREATE SNAPSHOT for graph state capture
- **Hyperedges**: Native N:M relationship support
- **Plugin System**: Custom scalar functions, triggers, serializers
- **Single-file Database**: Zero configuration, embedded in your application

## Links

- [Documentation](https://epsilondelta-ai.github.io/CypherLite/en/)
- [GitHub Repository](https://github.com/Epsilondelta-ai/CypherLite)
- [Rust API (docs.rs)](https://docs.rs/cypherlite-query)
- [Changelog](https://github.com/Epsilondelta-ai/CypherLite/blob/main/CHANGELOG.md)

## License

MIT OR Apache-2.0
