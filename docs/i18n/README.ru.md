<!-- CypherLite Documentation Translation -->
<!-- Source Language: English -->
<!-- Target Language: Russian (Русский) -->
<!-- Last Synced Commit: HEAD -->
<!-- Last Updated: 2026-03-15 -->

# CypherLite

![CI](https://github.com/Epsilondelta-ai/CypherLite/actions/workflows/ci.yml/badge.svg)
[![crates.io](https://img.shields.io/crates/v/cypherlite-query.svg)](https://crates.io/crates/cypherlite-query)
[![docs.rs](https://docs.rs/cypherlite-query/badge.svg)](https://docs.rs/cypherlite-query)
![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)
![MSRV](https://img.shields.io/badge/MSRV-1.84-orange.svg)

> Простота SQLite для графовых баз данных.

Лёгкий встраиваемый однофайловый движок графовой базы данных, написанный на Rust. CypherLite привносит развёртывание с нулевой конфигурацией в один файл в экосистему графовых баз данных — с полным соответствием ACID, нативной поддержкой графов свойств, темпоральными запросами, подграфовыми сущностями, гиперрёбрами и системой плагинов на основе трейтов.

**Доступно на**: [English](../../README.md) | [中文](README.zh.md) | [हिन्दी](README.hi.md) | [Español](README.es.md) | [Français](README.fr.md) | [العربية](README.ar.md) | [বাংলা](README.bn.md) | [Português](README.pt.md) | [한국어](README.ko.md)

---

## Возможности

### Движок хранения

- **ACID-транзакции** — Полная атомарность, согласованность, изолированность и долговечность с помощью журнала предварительной записи (WAL)
- **Один писатель / Множество читателей** — Модель параллелизма, совместимая с SQLite, с использованием `parking_lot`
- **Изоляция по снимку** — MVCC на основе индекса фреймов WAL для согласованного чтения
- **Хранение B+Tree** — Поиск O(log n) узлов и рёбер с безиндексной смежностью
- **Восстановление после сбоев** — Воспроизведение WAL при запуске для обеспечения согласованности после неожиданного отключения
- **Граф свойств** — Узлы и рёбра с типизированными свойствами: `Null`, `Bool`, `Int64`, `Float64`, `String`, `Bytes`, `Array`
- **Встраиваемая библиотека** — Без серверного процесса, нулевая конфигурация, единственный файл `.cyl`

### Движок запросов (Cypher)

- **Подмножество openCypher** — `MATCH`, `CREATE`, `MERGE`, `SET`, `DELETE`, `RETURN`, `WHERE`, `WITH`, `ORDER BY`
- **Рекурсивно-нисходящий парсер** — Написанный вручную парсер с парсингом выражений Pratt и поддержкой 28+ ключевых слов
- **Семантический анализ** — Проверка области видимости переменных и разрешение меток/типов
- **Оптимизатор на основе стоимости** — Преобразование логического плана в физический с отрезанием предикатов
- **Исполнитель Volcano** — Выполнение на основе итераторов с 12 операторами
- **Трёхзначная логика** — Полное распространение NULL согласно спецификации openCypher
- **Встроенный фильтр свойств** — Поддержка шаблона `MATCH (n:Label {key: value})`

### Темпоральные возможности

- **Запросы AT TIME** — Получение состояния графа в определённый момент времени
- **Хранилище версий** — Неизменяемая цепочка версий свойств для каждого узла и ребра
- **Темпоральное версионирование рёбер** — Временные метки создания/удаления рёбер с темпоральными запросами отношений
- **Темпоральная агрегация** — Запросы по временным диапазонам с агрегатными функциями над версионированными данными

### Подграфы и гиперрёбра

- **SubgraphStore** — Именованные подграфы как первоклассные сущности, хранящиеся наряду с узлами и рёбрами
- **CREATE / MATCH SNAPSHOT** — Захват и запрос именованных подграфовых сущностей
- **Нативные гиперрёбра** — N:M-отношения, соединяющие произвольное количество узлов
- **Синтаксис HYPEREDGE** — `CREATE HYPEREDGE :TYPE CONNECTING (n1), (n2), (n3)`
- **TemporalRef** — Члены гиперрёбер несут метаданные темпоральных ссылок

### Система плагинов

- **ScalarFunction** — Регистрация пользовательских функций запросов, вызываемых в выражениях Cypher
- **IndexPlugin** — Подключаемые реализации пользовательских индексов (например, векторный индекс HNSW)
- **Serializer** — Плагины пользовательского формата импорта/экспорта (например, JSON-LD, GraphML)
- **Trigger** — Хуки до/после для операций `CREATE`, `DELETE`, `SET` с поддержкой отката
- **PluginRegistry** — Обобщённый потокобезопасный реестр на основе `HashMap` (`Send + Sync`)
- **Нулевые накладные расходы** — Feature flag `plugin`; управляется через cfg, без затрат при отключении

### FFI-привязки

- **C ABI** — Статическая библиотека с C-заголовком для встраивания в любой C-совместимый проект
- **Python** — Привязки на основе PyO3 через `pip install cypherlite`
- **Go** — Привязки CGo через `go get github.com/Epsilondelta-ai/CypherLite/bindings/go/cypherlite`
- **Node.js** — Нативный модуль napi-rs через `npm install cypherlite`

---

## Быстрый старт

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

## Установка

### Rust (Cargo)

```toml
[dependencies]
cypherlite-query = "1.2"

# Опционально: включить конкретные feature flags
# cypherlite-query = { version = "1.2", features = ["temporal-edge", "plugin"] }
```

### Python (pip)

```bash
pip install cypherlite
```

Сборка из исходников со всеми возможностями:

```bash
cd crates/cypherlite-python
pip install maturin
maturin develop --release
```

### Go (go get)

```bash
go get github.com/Epsilondelta-ai/CypherLite/bindings/go/cypherlite
```

Требуется: Go 1.21+, инструментарий Rust (для сборки статической библиотеки C) и C-компилятор для CGo.

### Node.js (npm)

```bash
npm install cypherlite
```

Сборка из исходников:

```bash
cd crates/cypherlite-node
npx napi build --release
```

Требуется: Node.js 18+ с поддержкой N-API v9 и инструментарий Rust.

### C (заголовок + статическая библиотека)

```bash
cargo build -p cypherlite-ffi --release --all-features
```

Подключите `target/release/libcypherlite_ffi.a` и включите сгенерированный заголовок `cypherlite.h`.

---

## Архитектура

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

**Граф зависимостей crate:**

```
cypherlite-query
    └── cypherlite-storage
            └── cypherlite-core

cypherlite-ffi
    └── cypherlite-query

cypherlite-python  (wraps cypherlite-ffi via PyO3)
cypherlite-node    (wraps cypherlite-ffi via napi-rs)
```

**Конвейер выполнения запросов:**

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

Feature flags являются аддитивными. Каждый флаг включает функциональность всех флагов, указанных выше в таблице, кроме `plugin`, который является независимым.

| Флаг | По умолчанию | Описание |
|------|--------------|----------|
| `temporal-core` | Да | Базовые темпоральные возможности (запросы `AT TIME`, хранилище версий) |
| `temporal-edge` | Нет | Темпоральное версионирование рёбер и запросы темпоральных отношений |
| `subgraph` | Нет | Подграфовые сущности (`CREATE / MATCH SNAPSHOT`) |
| `hypergraph` | Нет | Нативные N:M гиперрёбра (синтаксис `HYPEREDGE`); подразумевает `subgraph` |
| `full-temporal` | Нет | Все темпоральные возможности вместе |
| `plugin` | Нет | Система плагинов — 4 типа плагинов, нулевые накладные расходы при отключении |

Включение флагов в `Cargo.toml`:

```toml
cypherlite-query = { version = "1.2", features = ["hypergraph", "plugin"] }
```

---

## Производительность

Тесты производительности запущены с Criterion на Apple M2 (однопоточный режим, сброс WAL в памяти отключён):

| Операция | Пропускная способность |
|----------|----------------------|
| INSERT узла | ~180,000 ops/sec |
| LOOKUP узла по ID | ~950,000 ops/sec |
| INSERT ребра | ~160,000 ops/sec |
| Простой запрос MATCH | ~120,000 queries/sec |
| Скорость записи WAL | ~450 MB/sec |

Запуск тестов производительности локально:

```bash
cargo bench --workspace --all-features
```

---

## Тестирование

```bash
# Все тесты (возможности по умолчанию)
cargo test --workspace

# Все тесты со всеми возможностями
cargo test --workspace --all-features

# Отчёт о покрытии
cargo llvm-cov --workspace --all-features --summary-only

# Линтер (принудительный ноль предупреждений)
cargo clippy --workspace --all-targets --all-features -- -D warnings

# Дымовой тест бенчмарков
cargo bench --workspace --all-features -- --test
```

**Набор тестов**: ~1490 тестов по всему workspace, все возможности включены, 0 предупреждений clippy, покрытие 85%+.

---

## Документация

- **Справочник API (docs.rs)**: [docs.rs/cypherlite-query](https://docs.rs/cypherlite-query)
- **Сайт документации**: [Epsilondelta-ai.github.io/CypherLite](https://Epsilondelta-ai.github.io/CypherLite)
- **Примеры быстрого старта**: [`examples/`](../../examples/) — скрипты на Rust, Python, Go и Node.js
- **Примеры FFI-привязок**: [`bindings/`](../../bindings/) — пакет Go с полным покрытием тестами

---

## Участие в разработке

Вклад приветствуется. Пожалуйста, прочитайте [CONTRIBUTING.md](../../CONTRIBUTING.md) для получения информации о:

- Руководстве по сообщениям об ошибках
- Процессе именования веток и pull request
- Настройке среды разработки (Rust 1.84+)
- Стиле кода: `cargo fmt`, `cargo clippy -- -D warnings`
- Требованиях к тестам: покрытие 85%+ на коммит

Для значительных изменений сначала откройте [issue](https://github.com/Epsilondelta-ai/CypherLite/issues) для обсуждения подхода перед реализацией.

---

## Лицензия

Лицензировано на выбор под одной из:

- [MIT License](../../LICENSE-MIT)
- [Apache License, Version 2.0](../../LICENSE-APACHE)

---

## Статус / Дорожная карта

| Фаза | Версия | Функциональность | Статус |
|------|--------|-----------------|--------|
| 1 | v0.1 | Движок хранения (WAL, B+Tree, ACID) | Завершено |
| 2 | v0.2 | Движок запросов (лексер, парсер, исполнитель Cypher) | Завершено |
| 3 | v0.3 | Расширенные запросы (MERGE, WITH, ORDER BY, оптимизатор) | Завершено |
| 4 | v0.4 | Темпоральное ядро (AT TIME, хранилище версий) | Завершено |
| 5 | v0.5 | Темпоральные рёбра (версионирование рёбер) | Завершено |
| 6 | v0.6 | Подграфовые сущности (SubgraphStore, SNAPSHOT) | Завершено |
| 7 | v0.7 | Нативные гиперрёбра (N:M, синтаксис HYPEREDGE) | Завершено |
| 8 | v0.8 | Встроенный фильтр свойств (исправление шаблона) | Завершено |
| 9 | v0.9 | Конвейер CI/CD (GitHub Actions, 6 заданий) | Завершено |
| 10 | v1.0 | Система плагинов (4 типа плагинов, реестр) | Завершено |
| 11 | v1.1 | Оптимизация производительности (бенчмарки, буферный пул) | Завершено |
| 12 | v1.1 | FFI-привязки (C, Python, Go, Node.js) | Завершено |
| 13 | v1.2 | Документация и i18n (rustdoc, сайт, примеры) | **Текущий** |
