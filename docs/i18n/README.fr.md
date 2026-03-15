<!-- CypherLite Documentation Translation -->
<!-- Source Language: English -->
<!-- Target Language: French (Français) -->
<!-- Last Synced Commit: HEAD -->
<!-- Last Updated: 2026-03-15 -->

<p align="center">
  <img src="../../assets/logo.png" alt="CypherLite Logo" width="180">
</p>

<h1 align="center">CypherLite</h1>

<p align="center">
  <img src="https://github.com/Epsilondelta-ai/CypherLite/actions/workflows/ci.yml/badge.svg" alt="CI">
  <a href="https://crates.io/crates/cypherlite-query"><img src="https://img.shields.io/crates/v/cypherlite-query.svg" alt="crates.io"></a>
  <a href="https://docs.rs/cypherlite-query"><img src="https://docs.rs/cypherlite-query/badge.svg" alt="docs.rs"></a>
  <img src="https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg" alt="License">
  <img src="https://img.shields.io/badge/MSRV-1.84-orange.svg" alt="MSRV">
</p>

```
        (\-.
        / _`>  CypherLite
       / /
      / /      Lightweight Embedded
     / /       Graph Database
    / /
   (,/
    ``
```

> La simplicité de SQLite pour les bases de données graphes.

Un moteur de base de données graphe léger, embarqué et mono-fichier écrit en Rust. CypherLite apporte le déploiement zéro configuration en fichier unique à l'écosystème des bases de données graphes, avec une conformité ACID complète, la prise en charge native des graphes de propriétés, les requêtes temporelles, les entités sous-graphes, les hyperarêtes et un système de plugins basé sur les traits.

**Disponible en**: [English](../../README.md) | [中文](README.zh.md) | [हिन्दी](README.hi.md) | [Español](README.es.md) | [العربية](README.ar.md) | [বাংলা](README.bn.md) | [Português](README.pt.md) | [Русский](README.ru.md) | [한국어](README.ko.md)

---

## Fonctionnalités

### Moteur de Stockage

- **Transactions ACID** — Atomicité, cohérence, isolation et durabilité complètes via Write-Ahead Logging
- **Un Écrivain / Multiples Lecteurs** — Modèle de concurrence compatible SQLite utilisant `parking_lot`
- **Isolation par Snapshot** — MVCC basé sur l'index des trames WAL pour des lectures cohérentes
- **Stockage B+Tree** — Recherche O(log n) de nœuds et d'arêtes avec adjacence sans index
- **Récupération après Crash** — Rejeu du WAL au démarrage pour assurer la cohérence après un arrêt inattendu
- **Graphe de Propriétés** — Nœuds et arêtes avec des propriétés typées : `Null`, `Bool`, `Int64`, `Float64`, `String`, `Bytes`, `Array`
- **Bibliothèque Embarquée** — Aucun processus serveur, zéro configuration, un seul fichier `.cyl`

### Moteur de Requêtes (Cypher)

- **Sous-ensemble openCypher** — `MATCH`, `CREATE`, `MERGE`, `SET`, `DELETE`, `RETURN`, `WHERE`, `WITH`, `ORDER BY`
- **Analyseur à Descente Récursive** — Analyseur écrit à la main avec analyse d'expressions Pratt et plus de 28 mots-clés
- **Analyse Sémantique** — Validation de la portée des variables et résolution des étiquettes/types
- **Optimiseur Basé sur les Coûts** — Conversion de plan logique en physique avec poussée de prédicats
- **Exécuteur Volcano** — Exécution basée sur les itérateurs avec 12 opérateurs
- **Logique à Trois Valeurs** — Propagation complète de NULL selon la spécification openCypher
- **Filtre de Propriétés en Ligne** — Support du pattern `MATCH (n:Label {key: value})`

### Fonctionnalités Temporelles

- **Requêtes AT TIME** — Récupération de l'état du graphe à un instant précis
- **Magasin de Versions** — Chaîne de versions de propriétés immuable par nœud et arête
- **Versionnage Temporel des Arêtes** — Horodatages de création/suppression d'arêtes avec requêtes de relations temporelles
- **Agrégation Temporelle** — Requêtes sur des plages temporelles avec fonctions d'agrégation sur des données versionnées

### Sous-graphe et Hyperarête

- **SubgraphStore** — Sous-graphes nommés comme entités de premier ordre stockées aux côtés des nœuds et des arêtes
- **CREATE / MATCH SNAPSHOT** — Capturer et interroger des entités de sous-graphes nommées
- **Hyperarêtes Natives** — Relations N:M connectant un nombre arbitraire de nœuds
- **Syntaxe HYPEREDGE** — `CREATE HYPEREDGE :TYPE CONNECTING (n1), (n2), (n3)`
- **TemporalRef** — Les membres des hyperarêtes portent des métadonnées de référence temporelle

### Système de Plugins

- **ScalarFunction** — Enregistrer des fonctions de requête personnalisées appelables dans les expressions Cypher
- **IndexPlugin** — Implémentations d'index personnalisées enfichables (par ex. index vectoriel HNSW)
- **Serializer** — Plugins de format d'import/export personnalisé (par ex. JSON-LD, GraphML)
- **Trigger** — Hooks avant/après pour les opérations `CREATE`, `DELETE`, `SET` avec support de rollback
- **PluginRegistry** — Registre générique thread-safe basé sur `HashMap` (`Send + Sync`)
- **Zéro Surcharge** — Feature flag `plugin` ; contrôlé par cfg, sans coût quand désactivé

### Bindings FFI

- **C ABI** — Bibliothèque statique avec en-tête C pour l'intégration dans tout projet compatible C
- **Python** — Bindings basés sur PyO3 via `pip install cypherlite`
- **Go** — Bindings CGo via `go get github.com/Epsilondelta-ai/CypherLite/bindings/go/cypherlite`
- **Node.js** — Module natif napi-rs via `npm install cypherlite`

---

## Démarrage Rapide

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

## Installation

### Rust (Cargo)

```toml
[dependencies]
cypherlite-query = "1.2"

# Optionnel : activer des feature flags spécifiques
# cypherlite-query = { version = "1.2", features = ["temporal-edge", "plugin"] }
```

### Python (pip)

```bash
pip install cypherlite
```

Compiler depuis les sources avec toutes les fonctionnalités :

```bash
cd crates/cypherlite-python
pip install maturin
maturin develop --release
```

### Go (go get)

```bash
go get github.com/Epsilondelta-ai/CypherLite/bindings/go/cypherlite
```

Prérequis : Go 1.21+, chaîne d'outils Rust (pour compiler la bibliothèque statique C), et un compilateur C pour CGo.

### Node.js (npm)

```bash
npm install cypherlite
```

Compiler depuis les sources :

```bash
cd crates/cypherlite-node
npx napi build --release
```

Prérequis : Node.js 18+ avec support N-API v9 et la chaîne d'outils Rust.

### C (en-tête + bibliothèque statique)

```bash
cargo build -p cypherlite-ffi --release --all-features
```

Liez `target/release/libcypherlite_ffi.a` et incluez l'en-tête généré `cypherlite.h`.

---

## Architecture

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

**Graphe de dépendances des crates :**

```
cypherlite-query
    └── cypherlite-storage
            └── cypherlite-core

cypherlite-ffi
    └── cypherlite-query

cypherlite-python  (wraps cypherlite-ffi via PyO3)
cypherlite-node    (wraps cypherlite-ffi via napi-rs)
```

**Pipeline d'exécution des requêtes :**

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

Les feature flags sont additifs. Chaque flag active les fonctionnalités de tous les flags listés au-dessus dans le tableau, sauf `plugin` qui est indépendant.

| Flag | Par Défaut | Description |
|------|------------|-------------|
| `temporal-core` | Oui | Fonctionnalités temporelles de base (requêtes `AT TIME`, magasin de versions) |
| `temporal-edge` | Non | Versionnage temporel des arêtes et requêtes de relations temporelles |
| `subgraph` | Non | Entités de sous-graphe (`CREATE / MATCH SNAPSHOT`) |
| `hypergraph` | Non | Hyperarêtes N:M natives (syntaxe `HYPEREDGE`) ; implique `subgraph` |
| `full-temporal` | Non | Toutes les fonctionnalités temporelles combinées |
| `plugin` | Non | Système de plugins — 4 types de plugins, zéro surcharge quand désactivé |

Activer les flags dans `Cargo.toml` :

```toml
cypherlite-query = { version = "1.2", features = ["hypergraph", "plugin"] }
```

---

## Performances

Benchmarks exécutés avec Criterion sur Apple M2 (mono-thread, vidage WAL en mémoire désactivé) :

| Opération | Débit |
|-----------|-------|
| INSERT de nœud | ~180,000 ops/sec |
| LOOKUP de nœud par ID | ~950,000 ops/sec |
| INSERT d'arête | ~160,000 ops/sec |
| Requête MATCH simple | ~120,000 queries/sec |
| Débit d'écriture WAL | ~450 MB/sec |

Exécuter les benchmarks localement :

```bash
cargo bench --workspace --all-features
```

---

## Tests

```bash
# Tous les tests (fonctionnalités par défaut)
cargo test --workspace

# Tous les tests avec toutes les fonctionnalités
cargo test --workspace --all-features

# Rapport de couverture
cargo llvm-cov --workspace --all-features --summary-only

# Linter (zéro avertissement imposé)
cargo clippy --workspace --all-targets --all-features -- -D warnings

# Test de fumée des benchmarks
cargo bench --workspace --all-features -- --test
```

**Suite de tests** : ~1,490 tests dans tout le workspace, toutes les fonctionnalités activées, 0 avertissement clippy, couverture supérieure à 85%.

---

## Documentation

- **Référence API (docs.rs)** : [docs.rs/cypherlite-query](https://docs.rs/cypherlite-query)
- **Site Web de Documentation** : [Epsilondelta-ai.github.io/CypherLite](https://Epsilondelta-ai.github.io/CypherLite)
- **Exemples de Démarrage Rapide** : [`examples/`](../../examples/) — Scripts Rust, Python, Go et Node.js
- **Exemples de Bindings FFI** : [`bindings/`](../../bindings/) — Package Go avec couverture de tests complète

---

## Contribuer

Les contributions sont les bienvenues. Veuillez lire [CONTRIBUTING.md](../../CONTRIBUTING.md) pour :

- Directives pour signaler des bugs
- Processus de nommage des branches et des pull requests
- Configuration de l'environnement de développement (Rust 1.84+)
- Style de code : `cargo fmt`, `cargo clippy -- -D warnings`
- Exigences de tests : couverture de 85%+ par commit

Ouvrez d'abord une [issue](https://github.com/Epsilondelta-ai/CypherLite/issues) pour les changements significatifs afin de discuter de l'approche avant l'implémentation.

---

## Licence

Sous licence au choix :

- [MIT License](../../LICENSE-MIT)
- [Apache License, Version 2.0](../../LICENSE-APACHE)

---

## Statut / Feuille de Route

| Phase | Version | Fonctionnalité | Statut |
|-------|---------|----------------|--------|
| 1 | v0.1 | Moteur de Stockage (WAL, B+Tree, ACID) | Terminé |
| 2 | v0.2 | Moteur de Requêtes (lexer, parser, executor Cypher) | Terminé |
| 3 | v0.3 | Requêtes Avancées (MERGE, WITH, ORDER BY, optimiseur) | Terminé |
| 4 | v0.4 | Noyau Temporel (AT TIME, magasin de versions) | Terminé |
| 5 | v0.5 | Arête Temporelle (versionnage des arêtes) | Terminé |
| 6 | v0.6 | Entités Sous-graphe (SubgraphStore, SNAPSHOT) | Terminé |
| 7 | v0.7 | Hyperarête Native (N:M, syntaxe HYPEREDGE) | Terminé |
| 8 | v0.8 | Filtre de Propriétés en Ligne (correction du pattern) | Terminé |
| 9 | v0.9 | Pipeline CI/CD (GitHub Actions, 6 jobs) | Terminé |
| 10 | v1.0 | Système de Plugins (4 types de plugins, registre) | Terminé |
| 11 | v1.1 | Optimisation des Performances (benchmarks, buffer pool) | Terminé |
| 12 | v1.1 | Bindings FFI (C, Python, Go, Node.js) | Terminé |
| 13 | v1.2 | Documentation et i18n (rustdoc, site web, exemples) | **Actuel** |
