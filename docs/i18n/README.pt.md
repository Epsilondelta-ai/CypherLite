<!-- CypherLite Documentation Translation -->
<!-- Source Language: English -->
<!-- Target Language: Portuguese (Português) -->
<!-- Last Synced Commit: HEAD -->
<!-- Last Updated: 2026-03-15 -->

# CypherLite

![CI](https://github.com/Epsilondelta-ai/CypherLite/actions/workflows/ci.yml/badge.svg)
[![crates.io](https://img.shields.io/crates/v/cypherlite-query.svg)](https://crates.io/crates/cypherlite-query)
[![docs.rs](https://docs.rs/cypherlite-query/badge.svg)](https://docs.rs/cypherlite-query)
![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)
![MSRV](https://img.shields.io/badge/MSRV-1.84-orange.svg)

> A simplicidade do SQLite para bancos de dados de grafos.

Um motor de banco de dados de grafos leve, embarcado e de arquivo único escrito em Rust. O CypherLite traz a implantação de zero configuração em arquivo único para o ecossistema de bancos de dados de grafos — com conformidade ACID completa, suporte nativo a grafos de propriedades, consultas temporais, entidades de subgrafos, hiperarestas e um sistema de plugins baseado em traits.

**Disponível em**: [English](../../README.md) | [中文](README.zh.md) | [हिन्दी](README.hi.md) | [Español](README.es.md) | [Français](README.fr.md) | [العربية](README.ar.md) | [বাংলা](README.bn.md) | [Русский](README.ru.md) | [한국어](README.ko.md)

---

## Funcionalidades

### Motor de Armazenamento

- **Transações ACID** — Atomicidade, consistência, isolamento e durabilidade completos via Write-Ahead Logging
- **Um Escritor / Múltiplos Leitores** — Modelo de concorrência compatível com SQLite usando `parking_lot`
- **Isolamento por Snapshot** — MVCC baseado em índice de frames WAL para leituras consistentes
- **Armazenamento B+Tree** — Busca O(log n) de nós e arestas com adjacência sem índice
- **Recuperação após Falha** — Repetição do WAL na inicialização para garantir consistência após desligamento inesperado
- **Grafo de Propriedades** — Nós e arestas com propriedades tipadas: `Null`, `Bool`, `Int64`, `Float64`, `String`, `Bytes`, `Array`
- **Biblioteca Embarcada** — Sem processo de servidor, zero configuração, arquivo único `.cyl`

### Motor de Consultas (Cypher)

- **Subconjunto openCypher** — `MATCH`, `CREATE`, `MERGE`, `SET`, `DELETE`, `RETURN`, `WHERE`, `WITH`, `ORDER BY`
- **Analisador de Descida Recursiva** — Analisador escrito à mão com análise de expressões Pratt e mais de 28 palavras-chave
- **Análise Semântica** — Validação de escopo de variáveis e resolução de rótulos/tipos
- **Otimizador Baseado em Custo** — Conversão de plano lógico para físico com redução de predicados
- **Executor Volcano** — Execução baseada em iteradores com 12 operadores
- **Lógica de Três Valores** — Propagação completa de NULL conforme a especificação openCypher
- **Filtro de Propriedades Inline** — Suporte ao padrão `MATCH (n:Label {key: value})`

### Funcionalidades Temporais

- **Consultas AT TIME** — Recuperação do estado do grafo em um ponto específico no tempo
- **Armazenamento de Versões** — Cadeia de versões de propriedades imutável por nó e aresta
- **Versionamento Temporal de Arestas** — Timestamps de criação/exclusão de arestas com consultas de relações temporais
- **Agregação Temporal** — Consultas em intervalos temporais com funções de agregado sobre dados versionados

### Subgrafo e Hiperaresta

- **SubgraphStore** — Subgrafos nomeados como entidades de primeira classe armazenadas junto a nós e arestas
- **CREATE / MATCH SNAPSHOT** — Capturar e consultar entidades de subgrafos nomeadas
- **Hiperarestas Nativas** — Relações N:M conectando um número arbitrário de nós
- **Sintaxe HYPEREDGE** — `CREATE HYPEREDGE :TYPE CONNECTING (n1), (n2), (n3)`
- **TemporalRef** — Membros de hiperarestas carregam metadados de referência temporal

### Sistema de Plugins

- **ScalarFunction** — Registrar funções de consulta personalizadas invocáveis em expressões Cypher
- **IndexPlugin** — Implementações de índice personalizadas plugáveis (por ex. índice vetorial HNSW)
- **Serializer** — Plugins de formato de importação/exportação personalizado (por ex. JSON-LD, GraphML)
- **Trigger** — Hooks antes/depois para operações `CREATE`, `DELETE`, `SET` com suporte a rollback
- **PluginRegistry** — Registro genérico thread-safe baseado em `HashMap` (`Send + Sync`)
- **Zero Overhead** — Feature flag `plugin`; controlado por cfg, sem custo quando desabilitado

### Bindings FFI

- **C ABI** — Biblioteca estática com cabeçalho C para incorporação em qualquer projeto compatível com C
- **Python** — Bindings baseados em PyO3 via `pip install cypherlite`
- **Go** — Bindings CGo via `go get github.com/Epsilondelta-ai/CypherLite/bindings/go/cypherlite`
- **Node.js** — Módulo nativo napi-rs via `npm install cypherlite`

---

## Início Rápido

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

## Instalação

### Rust (Cargo)

```toml
[dependencies]
cypherlite-query = "1.2"

# Opcional: habilitar feature flags específicas
# cypherlite-query = { version = "1.2", features = ["temporal-edge", "plugin"] }
```

### Python (pip)

```bash
pip install cypherlite
```

Compilar a partir do código fonte com todas as funcionalidades:

```bash
cd crates/cypherlite-python
pip install maturin
maturin develop --release
```

### Go (go get)

```bash
go get github.com/Epsilondelta-ai/CypherLite/bindings/go/cypherlite
```

Requer: Go 1.21+, cadeia de ferramentas Rust (para compilar a biblioteca estática C) e um compilador C para CGo.

### Node.js (npm)

```bash
npm install cypherlite
```

Compilar a partir do código fonte:

```bash
cd crates/cypherlite-node
npx napi build --release
```

Requer: Node.js 18+ com suporte N-API v9 e a cadeia de ferramentas Rust.

### C (cabeçalho + biblioteca estática)

```bash
cargo build -p cypherlite-ffi --release --all-features
```

Vincule `target/release/libcypherlite_ffi.a` e inclua o cabeçalho gerado `cypherlite.h`.

---

## Arquitetura

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

**Grafo de dependências de crates:**

```
cypherlite-query
    └── cypherlite-storage
            └── cypherlite-core

cypherlite-ffi
    └── cypherlite-query

cypherlite-python  (wraps cypherlite-ffi via PyO3)
cypherlite-node    (wraps cypherlite-ffi via napi-rs)
```

**Pipeline de execução de consultas:**

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

As feature flags são aditivas. Cada flag habilita as funcionalidades de todas as flags listadas acima na tabela, exceto `plugin` que é independente.

| Flag | Padrão | Descrição |
|------|--------|-----------|
| `temporal-core` | Sim | Funcionalidades temporais básicas (consultas `AT TIME`, armazenamento de versões) |
| `temporal-edge` | Não | Versionamento temporal de arestas e consultas de relações temporais |
| `subgraph` | Não | Entidades de subgrafo (`CREATE / MATCH SNAPSHOT`) |
| `hypergraph` | Não | Hiperarestas N:M nativas (sintaxe `HYPEREDGE`); implica `subgraph` |
| `full-temporal` | Não | Todas as funcionalidades temporais combinadas |
| `plugin` | Não | Sistema de plugins — 4 tipos de plugins, zero overhead quando desabilitado |

Habilitar flags no `Cargo.toml`:

```toml
cypherlite-query = { version = "1.2", features = ["hypergraph", "plugin"] }
```

---

## Desempenho

Benchmarks executados com Criterion em Apple M2 (single-thread, descarga WAL em memória desabilitada):

| Operação | Throughput |
|----------|-----------|
| INSERT de nó | ~180,000 ops/sec |
| LOOKUP de nó por ID | ~950,000 ops/sec |
| INSERT de aresta | ~160,000 ops/sec |
| Consulta MATCH simples | ~120,000 queries/sec |
| Throughput de escrita WAL | ~450 MB/sec |

Executar benchmarks localmente:

```bash
cargo bench --workspace --all-features
```

---

## Testes

```bash
# Todos os testes (funcionalidades padrão)
cargo test --workspace

# Todos os testes com todas as funcionalidades
cargo test --workspace --all-features

# Relatório de cobertura
cargo llvm-cov --workspace --all-features --summary-only

# Linter (zero avisos impostos)
cargo clippy --workspace --all-targets --all-features -- -D warnings

# Teste de fumaça de benchmark
cargo bench --workspace --all-features -- --test
```

**Suite de testes**: ~1.490 testes em todo o workspace, todas as funcionalidades habilitadas, 0 avisos de clippy, cobertura acima de 85%.

---

## Documentação

- **Referência API (docs.rs)**: [docs.rs/cypherlite-query](https://docs.rs/cypherlite-query)
- **Site de Documentação**: [Epsilondelta-ai.github.io/CypherLite](https://Epsilondelta-ai.github.io/CypherLite)
- **Exemplos de Início Rápido**: [`examples/`](../../examples/) — Scripts de Rust, Python, Go e Node.js
- **Exemplos de Bindings FFI**: [`bindings/`](../../bindings/) — Pacote Go com cobertura de testes completa

---

## Contribuindo

Contribuições são bem-vindas. Por favor leia [CONTRIBUTING.md](../../CONTRIBUTING.md) para:

- Diretrizes para reportar bugs
- Processo de nomenclatura de branches e pull requests
- Configuração do ambiente de desenvolvimento (Rust 1.84+)
- Estilo de código: `cargo fmt`, `cargo clippy -- -D warnings`
- Requisitos de testes: cobertura de 85%+ por commit

Abra uma [issue](https://github.com/Epsilondelta-ai/CypherLite/issues) primeiro para mudanças significativas para discutir a abordagem antes da implementação.

---

## Licença

Licenciado sob qualquer um dos seguintes:

- [MIT License](../../LICENSE-MIT)
- [Apache License, Version 2.0](../../LICENSE-APACHE)

à sua escolha.

---

## Status / Roadmap

| Fase | Versão | Funcionalidade | Status |
|------|--------|---------------|--------|
| 1 | v0.1 | Motor de Armazenamento (WAL, B+Tree, ACID) | Completo |
| 2 | v0.2 | Motor de Consultas (lexer, parser, executor Cypher) | Completo |
| 3 | v0.3 | Consultas Avançadas (MERGE, WITH, ORDER BY, otimizador) | Completo |
| 4 | v0.4 | Núcleo Temporal (AT TIME, armazenamento de versões) | Completo |
| 5 | v0.5 | Aresta Temporal (versionamento de arestas) | Completo |
| 6 | v0.6 | Entidades de Subgrafo (SubgraphStore, SNAPSHOT) | Completo |
| 7 | v0.7 | Hiperaresta Nativa (N:M, sintaxe HYPEREDGE) | Completo |
| 8 | v0.8 | Filtro de Propriedades Inline (correção de padrão) | Completo |
| 9 | v0.9 | Pipeline CI/CD (GitHub Actions, 6 jobs) | Completo |
| 10 | v1.0 | Sistema de Plugins (4 tipos de plugins, registro) | Completo |
| 11 | v1.1 | Otimização de Desempenho (benchmarks, buffer pool) | Completo |
| 12 | v1.1 | Bindings FFI (C, Python, Go, Node.js) | Completo |
| 13 | v1.2 | Documentação e i18n (rustdoc, site, exemplos) | **Atual** |
