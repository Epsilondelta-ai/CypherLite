<p align="center">
  <img src="https://raw.githubusercontent.com/Epsilondelta-ai/CypherLite/main/assets/logo.png" alt="CypherLite" width="120">
</p>

# CypherLite

[![npm](https://img.shields.io/npm/v/cypherlite.svg)](https://www.npmjs.com/package/cypherlite)
[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](https://github.com/Epsilondelta-ai/CypherLite)

> SQLite-like simplicity for graph databases.

**CypherLite** is a lightweight, embedded, single-file graph database engine written in Rust with Node.js bindings via napi-rs. Zero-config, ACID-compliant, with native Cypher query support.

## Installation

```bash
npm install cypherlite
```

Pre-built native addons are available for:
- Linux (x86_64, aarch64)
- macOS (x86_64, arm64)
- Windows (x86_64)

## Quick Start

```javascript
const { open } = require('cypherlite');

// Open (or create) a database
const db = open('my_graph.cyl');

// Create nodes and relationships
db.execute("CREATE (a:Person {name: 'Alice', age: 30})");
db.execute("CREATE (b:Person {name: 'Bob', age: 25})");
db.execute(`
  MATCH (a:Person {name: 'Alice'}), (b:Person {name: 'Bob'})
  CREATE (a)-[:KNOWS {since: 2024}]->(b)
`);

// Query the graph
const result = db.execute(
  'MATCH (p:Person) RETURN p.name, p.age ORDER BY p.age'
);
for (const row of result) {
  console.log(`${row['p.name']}: ${row['p.age']}`);
}

// Parameterized queries
const alice = db.execute(
  'MATCH (p:Person) WHERE p.name = $name RETURN p.age',
  { name: 'Alice' }
);

// Transactions
const tx = db.begin();
tx.execute("CREATE (c:Person {name: 'Charlie', age: 35})");
tx.commit();

db.close();
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
