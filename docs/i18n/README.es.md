<!-- CypherLite Documentation Translation -->
<!-- Source Language: English -->
<!-- Target Language: Spanish (Español) -->
<!-- Last Synced Commit: HEAD -->
<!-- Last Updated: 2026-03-15 -->

# CypherLite

![CI](https://github.com/Epsilondelta-ai/CypherLite/actions/workflows/ci.yml/badge.svg)
[![crates.io](https://img.shields.io/crates/v/cypherlite-query.svg)](https://crates.io/crates/cypherlite-query)
[![docs.rs](https://docs.rs/cypherlite-query/badge.svg)](https://docs.rs/cypherlite-query)
![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)
![MSRV](https://img.shields.io/badge/MSRV-1.84-orange.svg)

> La simplicidad de SQLite para bases de datos de grafos.

Un motor de base de datos de grafos ligero, embebido y de un solo archivo escrito en Rust. CypherLite lleva el despliegue de cero configuración en un solo archivo al ecosistema de bases de datos de grafos, con cumplimiento ACID completo, soporte nativo de grafos de propiedades, consultas temporales, entidades de subgrafos, hiperaristas y un sistema de plugins basado en traits.

**Disponible en**: [English](../../README.md) | [中文](README.zh.md) | [हिन्दी](README.hi.md) | [Français](README.fr.md) | [العربية](README.ar.md) | [বাংলা](README.bn.md) | [Português](README.pt.md) | [Русский](README.ru.md) | [한국어](README.ko.md)

---

## Características

### Motor de Almacenamiento

- **Transacciones ACID** — Atomicidad, consistencia, aislamiento y durabilidad completos mediante Write-Ahead Logging
- **Un Escritor / Múltiples Lectores** — Modelo de concurrencia compatible con SQLite usando `parking_lot`
- **Aislamiento de Instantánea** — MVCC basado en índice de tramas WAL para lecturas consistentes
- **Almacenamiento B+Tree** — Búsqueda O(log n) de nodos y aristas con adyacencia sin índices
- **Recuperación ante Fallos** — Reproducción WAL al iniciar para garantizar consistencia tras apagado inesperado
- **Grafo de Propiedades** — Nodos y aristas con propiedades tipadas: `Null`, `Bool`, `Int64`, `Float64`, `String`, `Bytes`, `Array`
- **Biblioteca Embebida** — Sin proceso de servidor, cero configuración, un único archivo `.cyl`

### Motor de Consultas (Cypher)

- **Subconjunto openCypher** — `MATCH`, `CREATE`, `MERGE`, `SET`, `DELETE`, `RETURN`, `WHERE`, `WITH`, `ORDER BY`
- **Analizador de Descenso Recursivo** — Analizador escrito a mano con análisis de expresiones Pratt y más de 28 palabras clave
- **Análisis Semántico** — Validación de ámbito de variables y resolución de etiquetas/tipos
- **Optimizador Basado en Costos** — Conversión de plan lógico a físico con reducción de predicados
- **Ejecutor Volcano** — Ejecución basada en iteradores con 12 operadores
- **Lógica de Tres Valores** — Propagación completa de NULL según la especificación openCypher
- **Filtro de Propiedades en Línea** — Soporte para el patrón `MATCH (n:Label {key: value})`

### Características Temporales

- **Consultas AT TIME** — Recuperación del estado del grafo en un punto específico en el tiempo
- **Almacén de Versiones** — Cadena de versiones de propiedades inmutables por nodo y arista
- **Versionado Temporal de Aristas** — Marcas de tiempo de creación/eliminación de aristas con consultas de relaciones temporales
- **Agregación Temporal** — Consultas de rango temporal con funciones de agregado sobre datos versionados

### Subgrafo e Hiperarista

- **SubgraphStore** — Subgrafos nombrados como entidades de primer nivel almacenadas junto a nodos y aristas
- **CREATE / MATCH SNAPSHOT** — Capturar y consultar entidades de subgrafos nombradas
- **Hiperaristas Nativas** — Relaciones N:M que conectan un número arbitrario de nodos
- **Sintaxis HYPEREDGE** — `CREATE HYPEREDGE :TYPE CONNECTING (n1), (n2), (n3)`
- **TemporalRef** — Los miembros de hiperaristas llevan metadatos de referencia temporal

### Sistema de Plugins

- **ScalarFunction** — Registrar funciones de consulta personalizadas invocables en expresiones Cypher
- **IndexPlugin** — Implementaciones de índices personalizados enchufables (por ejemplo, índice vectorial HNSW)
- **Serializer** — Plugins de formato de importación/exportación personalizado (por ejemplo, JSON-LD, GraphML)
- **Trigger** — Hooks antes/después para operaciones `CREATE`, `DELETE`, `SET` con soporte de rollback
- **PluginRegistry** — Registro genérico y thread-safe basado en `HashMap` (`Send + Sync`)
- **Cero Overhead** — Feature flag `plugin`; controlado por cfg, sin costo cuando está deshabilitado

### Bindings FFI

- **C ABI** — Biblioteca estática con cabecera C para embeber en cualquier proyecto compatible con C
- **Python** — Bindings basados en PyO3 mediante `pip install cypherlite`
- **Go** — Bindings CGo mediante `go get github.com/Epsilondelta-ai/CypherLite/bindings/go/cypherlite`
- **Node.js** — Complemento nativo napi-rs mediante `npm install cypherlite`

---

## Inicio Rápido

### Rust

```toml
# Cargo.toml
[dependencies]
cypherlite-query = "1.2"
```

```rust
use cypherlite_query::CypherLite;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let db = CypherLite::open("my_graph.cyl")?;

    // Create nodes and a relationship
    db.execute("CREATE (a:Person {name: 'Alice', age: 30})")?;
    db.execute("CREATE (b:Person {name: 'Bob', age: 25})")?;
    db.execute(
        "MATCH (a:Person {name: 'Alice'}), (b:Person {name: 'Bob'}) \
         CREATE (a)-[:KNOWS {since: 2023}]->(b)",
    )?;

    // Query the graph
    let result = db.execute("MATCH (p:Person) WHERE p.age > 20 RETURN p.name, p.age")?;
    for row in result {
        let row = row?;
        println!("{}: {}", row.get("p.name").unwrap(), row.get("p.age").unwrap());
    }
    Ok(())
}
```

### Python

```bash
pip install cypherlite
```

```python
import cypherlite

db = cypherlite.open("my_graph.cyl")

db.execute("CREATE (a:Person {name: 'Alice', age: 30})")
db.execute("CREATE (b:Person {name: 'Bob', age: 25})")
db.execute(
    "MATCH (a:Person {name: 'Alice'}), (b:Person {name: 'Bob'}) "
    "CREATE (a)-[:KNOWS {since: 2023}]->(b)"
)

result = db.execute("MATCH (n:Person) RETURN n.name, n.age")
for row in result:
    print(f"{row['n.name']} (age: {row['n.age']})")

db.close()
```

### Go

```bash
go get github.com/Epsilondelta-ai/CypherLite/bindings/go/cypherlite
```

```go
package main

import (
    "fmt"
    "github.com/Epsilondelta-ai/CypherLite/bindings/go/cypherlite"
)

func main() {
    db, _ := cypherlite.Open("my_graph.cyl")
    defer db.Close()

    db.Execute("CREATE (a:Person {name: 'Alice', age: 30})")
    db.Execute("CREATE (b:Person {name: 'Bob', age: 25})")

    result, _ := db.Execute("MATCH (n:Person) RETURN n.name, n.age")
    for result.Next() {
        row := result.Row()
        name, _ := row.GetString("n.name")
        age, _ := row.GetInt64("n.age")
        fmt.Printf("%s (age: %d)\n", name, age)
    }
}
```

### Node.js

```bash
npm install cypherlite
```

```js
const { open } = require("cypherlite");

const db = open("my_graph.cyl");

db.execute("CREATE (a:Person {name: 'Alice', age: 30})");
db.execute("CREATE (b:Person {name: 'Bob', age: 25})");

const result = db.execute("MATCH (n:Person) RETURN n.name, n.age");
for (const row of result) {
  console.log(`${row["n.name"]} (age: ${row["n.age"]})`);
}

db.close();
```

---

## Instalación

### Rust (Cargo)

```toml
[dependencies]
cypherlite-query = "1.2"

# Opcional: habilitar feature flags específicos
# cypherlite-query = { version = "1.2", features = ["temporal-edge", "plugin"] }
```

### Python (pip)

```bash
pip install cypherlite
```

Compilar desde el código fuente con todas las características:

```bash
cd crates/cypherlite-python
pip install maturin
maturin develop --release
```

### Go (go get)

```bash
go get github.com/Epsilondelta-ai/CypherLite/bindings/go/cypherlite
```

Requiere: Go 1.21+, cadena de herramientas Rust (para compilar la biblioteca estática C) y un compilador C para CGo.

### Node.js (npm)

```bash
npm install cypherlite
```

Compilar desde el código fuente:

```bash
cd crates/cypherlite-node
npx napi build --release
```

Requiere: Node.js 18+ con soporte N-API v9 y la cadena de herramientas Rust.

### C (cabecera + biblioteca estática)

```bash
cargo build -p cypherlite-ffi --release --all-features
```

Enlaza `target/release/libcypherlite_ffi.a` e incluye la cabecera generada `cypherlite.h`.

---

## Arquitectura

```
┌─────────────────────────────────────────┐
│           Application Layer             │
│  (user code: Rust, Python, Go, Node.js) │
├─────────────────────────────────────────┤
│     cypherlite-ffi / Bindings           │
│     (C ABI, PyO3, CGo, napi-rs)         │
├─────────────────────────────────────────┤
│         cypherlite-query                │
│  (Lexer → Parser → Planner → Executor)  │
├─────────────────────────────────────────┤
│        cypherlite-storage               │
│   (WAL, B+Tree, BufferPool, MVCC)       │
├─────────────────────────────────────────┤
│         cypherlite-core                 │
│   (Types, Traits, Error Handling)       │
└─────────────────────────────────────────┘
         ┊ plugin (orthogonal) ┊
```

**Grafo de dependencias de crates:**

```
cypherlite-query
    └── cypherlite-storage
            └── cypherlite-core

cypherlite-ffi
    └── cypherlite-query

cypherlite-python  (wraps cypherlite-ffi via PyO3)
cypherlite-node    (wraps cypherlite-ffi via napi-rs)
```

**Pipeline de ejecución de consultas:**

```
Cypher string
    → Lexer (logos tokenizer)
    → Parser (recursive descent + Pratt)
    → Semantic Analyzer (scope, labels)
    → Planner (logical → physical, cost-based)
    → Executor (Volcano iterator model)
    → QueryResult (iterable rows)
```

---

## Feature Flags

Los feature flags son aditivos. Cada flag habilita las características de todos los flags listados por encima en la tabla, excepto `plugin` que es independiente.

| Flag | Por Defecto | Descripción |
|------|-------------|-------------|
| `temporal-core` | Sí | Características temporales básicas (consultas `AT TIME`, almacén de versiones) |
| `temporal-edge` | No | Versionado temporal de aristas y consultas de relaciones temporales |
| `subgraph` | No | Entidades de subgrafo (`CREATE / MATCH SNAPSHOT`) |
| `hypergraph` | No | Hiperaristas N:M nativas (sintaxis `HYPEREDGE`); implica `subgraph` |
| `full-temporal` | No | Todas las características temporales combinadas |
| `plugin` | No | Sistema de plugins — 4 tipos de plugins, cero overhead cuando está deshabilitado |

Habilitar flags en `Cargo.toml`:

```toml
cypherlite-query = { version = "1.2", features = ["hypergraph", "plugin"] }
```

---

## Rendimiento

Benchmarks ejecutados con Criterion en Apple M2 (un solo hilo, volcado WAL en memoria deshabilitado):

| Operación | Rendimiento |
|-----------|-------------|
| INSERT de nodo | ~180,000 ops/sec |
| LOOKUP de nodo por ID | ~950,000 ops/sec |
| INSERT de arista | ~160,000 ops/sec |
| Consulta MATCH simple | ~120,000 queries/sec |
| Rendimiento de escritura WAL | ~450 MB/sec |

Ejecutar benchmarks localmente:

```bash
cargo bench --workspace --all-features
```

---

## Pruebas

```bash
# Todas las pruebas (características por defecto)
cargo test --workspace

# Todas las pruebas con todas las características
cargo test --workspace --all-features

# Informe de cobertura
cargo llvm-cov --workspace --all-features --summary-only

# Linter (cero advertencias forzadas)
cargo clippy --workspace --all-targets --all-features -- -D warnings

# Prueba de humo de benchmark
cargo bench --workspace --all-features -- --test
```

**Suite de pruebas**: ~1,490 pruebas en todo el workspace, todas las características habilitadas, 0 advertencias de clippy, cobertura superior al 85%.

---

## Documentación

- **Referencia API (docs.rs)**: [docs.rs/cypherlite-query](https://docs.rs/cypherlite-query)
- **Sitio Web de Documentación**: [Epsilondelta-ai.github.io/CypherLite](https://Epsilondelta-ai.github.io/CypherLite)
- **Ejemplos de Inicio Rápido**: [`examples/`](../../examples/) — Scripts de Rust, Python, Go y Node.js
- **Ejemplos de Bindings FFI**: [`bindings/`](../../bindings/) — Paquete Go con cobertura de pruebas completa

---

## Contribuir

Las contribuciones son bienvenidas. Por favor lee [CONTRIBUTING.md](../../CONTRIBUTING.md) para:

- Directrices para reportar bugs
- Proceso de nombrado de ramas y pull requests
- Configuración del entorno de desarrollo (Rust 1.84+)
- Estilo de código: `cargo fmt`, `cargo clippy -- -D warnings`
- Requisitos de pruebas: cobertura del 85%+ por commit

Abre un [issue](https://github.com/Epsilondelta-ai/CypherLite/issues) primero para cambios significativos para discutir el enfoque antes de la implementación.

---

## Licencia

Licenciado bajo cualquiera de:

- [MIT License](../../LICENSE-MIT)
- [Apache License, Version 2.0](../../LICENSE-APACHE)

a tu elección.

---

## Estado / Hoja de Ruta

| Fase | Versión | Característica | Estado |
|------|---------|---------------|--------|
| 1 | v0.1 | Motor de Almacenamiento (WAL, B+Tree, ACID) | Completo |
| 2 | v0.2 | Motor de Consultas (lexer, parser, executor de Cypher) | Completo |
| 3 | v0.3 | Consultas Avanzadas (MERGE, WITH, ORDER BY, optimizador) | Completo |
| 4 | v0.4 | Núcleo Temporal (AT TIME, almacén de versiones) | Completo |
| 5 | v0.5 | Arista Temporal (versionado de aristas) | Completo |
| 6 | v0.6 | Entidades de Subgrafo (SubgraphStore, SNAPSHOT) | Completo |
| 7 | v0.7 | Hiperarista Nativa (N:M, sintaxis HYPEREDGE) | Completo |
| 8 | v0.8 | Filtro de Propiedades en Línea (corrección de patrón) | Completo |
| 9 | v0.9 | Pipeline CI/CD (GitHub Actions, 6 trabajos) | Completo |
| 10 | v1.0 | Sistema de Plugins (4 tipos de plugins, registro) | Completo |
| 11 | v1.1 | Optimización de Rendimiento (benchmarks, buffer pool) | Completo |
| 12 | v1.1 | Bindings FFI (C, Python, Go, Node.js) | Completo |
| 13 | v1.2 | Documentación e i18n (rustdoc, sitio web, ejemplos) | **Actual** |
