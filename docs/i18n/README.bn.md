<!-- CypherLite Documentation Translation -->
<!-- Source Language: English -->
<!-- Target Language: Bengali (বাংলা) -->
<!-- Last Synced Commit: HEAD -->
<!-- Last Updated: 2026-03-15 -->

# CypherLite

![CI](https://github.com/Epsilondelta-ai/CypherLite/actions/workflows/ci.yml/badge.svg)
[![crates.io](https://img.shields.io/crates/v/cypherlite-query.svg)](https://crates.io/crates/cypherlite-query)
[![docs.rs](https://docs.rs/cypherlite-query/badge.svg)](https://docs.rs/cypherlite-query)
![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)
![MSRV](https://img.shields.io/badge/MSRV-1.84-orange.svg)

> গ্রাফ ডেটাবেসের জন্য SQLite-এর মতো সরলতা।

Rust-এ লেখা একটি হালকা, এমবেডেড, একক-ফাইল গ্রাফ ডেটাবেস ইঞ্জিন। CypherLite গ্রাফ ডেটাবেস ইকোসিস্টেমে জিরো-কনফিগ, সিঙ্গেল-ফাইল ডিপ্লয়মেন্ট নিয়ে আসে — সম্পূর্ণ ACID সম্মতি, নেটিভ প্রপার্টি গ্রাফ সাপোর্ট, টেম্পোরাল কোয়েরি, সাবগ্রাফ এন্টিটি, হাইপারএজ এবং ট্রেট-ভিত্তিক প্লাগইন সিস্টেম সহ।

**অন্য ভাষায় পড়ুন**: [English](../../README.md) | [中文](README.zh.md) | [हिन्दी](README.hi.md) | [Español](README.es.md) | [Français](README.fr.md) | [العربية](README.ar.md) | [Português](README.pt.md) | [Русский](README.ru.md) | [한국어](README.ko.md)

---

## বৈশিষ্ট্যসমূহ

### স্টোরেজ ইঞ্জিন

- **ACID ট্রানজেকশন** — Write-Ahead Logging-এর মাধ্যমে সম্পূর্ণ পারমাণবিকতা, সামঞ্জস্য, বিচ্ছিন্নতা এবং স্থায়িত্ব
- **একক লেখক / একাধিক পাঠক** — `parking_lot` ব্যবহার করে SQLite-সামঞ্জস্যপূর্ণ কনকারেন্সি মডেল
- **স্ন্যাপশট আইসোলেশন** — সামঞ্জস্যপূর্ণ রিডের জন্য WAL ফ্রেম ইন্ডেক্স-ভিত্তিক MVCC
- **B+Tree স্টোরেজ** — ইন্ডেক্স-ফ্রি অ্যাডজেসেন্সি সহ O(log n) নোড এবং এজ লুকআপ
- **ক্র্যাশ রিকভারি** — অপ্রত্যাশিত শাটডাউনের পরে সামঞ্জস্যের জন্য স্টার্টআপে WAL রিপ্লে
- **প্রপার্টি গ্রাফ** — টাইপযুক্ত প্রপার্টি সহ নোড এবং এজ: `Null`, `Bool`, `Int64`, `Float64`, `String`, `Bytes`, `Array`
- **এমবেডেড লাইব্রেরি** — কোনো সার্ভার প্রক্রিয়া নেই, জিরো কনফিগারেশন, একটি `.cyl` ফাইল

### কোয়েরি ইঞ্জিন (Cypher)

- **openCypher সাবসেট** — `MATCH`, `CREATE`, `MERGE`, `SET`, `DELETE`, `RETURN`, `WHERE`, `WITH`, `ORDER BY`
- **রিকার্সিভ ডিসেন্ট পার্সার** — Pratt এক্সপ্রেশন পার্সিং এবং ২৮+ কীওয়ার্ড সহ হাতে লেখা পার্সার
- **সিমান্টিক অ্যানালাইসিস** — ভেরিয়েবল স্কোপ ভ্যালিডেশন এবং লেবেল/টাইপ রেজোলিউশন
- **কস্ট-বেসড অপ্টিমাইজার** — প্রেডিকেট পুশডাউন সহ লজিক্যাল-টু-ফিজিক্যাল প্ল্যান কনভার্সন
- **ভলকানো এক্সিকিউটর** — ১২টি অপারেটর সহ ইটারেটর-ভিত্তিক এক্সিকিউশন
- **ত্রি-মূল্য যুক্তি** — openCypher স্পেসিফিকেশন অনুযায়ী সম্পূর্ণ NULL প্রোপাগেশন
- **ইনলাইন প্রপার্টি ফিল্টার** — `MATCH (n:Label {key: value})` প্যাটার্ন সাপোর্ট

### টেম্পোরাল ফিচার

- **AT TIME কোয়েরি** — নির্দিষ্ট সময়ে গ্রাফ স্টেট রিট্রিভাল
- **ভার্সন স্টোর** — প্রতিটি নোড এবং এজের জন্য অপরিবর্তনীয় প্রপার্টি ভার্সন চেইন
- **টেম্পোরাল এজ ভার্সনিং** — এজ তৈরি/মুছে ফেলার টাইমস্ট্যাম্প এবং টেম্পোরাল রিলেশনশিপ কোয়েরি
- **টেম্পোরাল অ্যাগ্রিগেশন** — ভার্সনযুক্ত ডেটার উপর অ্যাগ্রিগেট ফাংশন সহ টাইম-রেঞ্জ কোয়েরি

### সাবগ্রাফ এবং হাইপারএজ

- **SubgraphStore** — নোড এবং এজের পাশাপাশি ফার্স্ট-ক্লাস এন্টিটি হিসেবে নেমড সাবগ্রাফ
- **CREATE / MATCH SNAPSHOT** — নেমড সাবগ্রাফ এন্টিটি ক্যাপচার এবং কোয়েরি করুন
- **নেটিভ হাইপারএজ** — স্বেচ্ছাসংখ্যক নোড সংযুক্ত করার N:M সম্পর্ক
- **HYPEREDGE সিনট্যাক্স** — `CREATE HYPEREDGE :TYPE CONNECTING (n1), (n2), (n3)`
- **TemporalRef** — হাইপারএজ সদস্যরা টেম্পোরাল রেফারেন্স মেটাডেটা বহন করে

### প্লাগইন সিস্টেম

- **ScalarFunction** — Cypher এক্সপ্রেশনে কলযোগ্য কাস্টম কোয়েরি ফাংশন নিবন্ধন করুন
- **IndexPlugin** — প্লাগ করার যোগ্য কাস্টম ইন্ডেক্স ইমপ্লিমেন্টেশন (যেমন HNSW ভেক্টর ইন্ডেক্স)
- **Serializer** — কাস্টম ইম্পোর্ট/এক্সপোর্ট ফরম্যাট প্লাগইন (যেমন JSON-LD, GraphML)
- **Trigger** — রোলব্যাক সাপোর্ট সহ `CREATE`, `DELETE`, `SET` অপারেশনের আগে/পরে হুক
- **PluginRegistry** — জেনেরিক, থ্রেড-সেফ `HashMap`-ভিত্তিক রেজিস্ট্রি (`Send + Sync`)
- **জিরো ওভারহেড** — `plugin` ফিচার ফ্ল্যাগ; cfg-গেটেড, নিষ্ক্রিয় হলে কোনো খরচ নেই

### FFI বাইন্ডিং

- **C ABI** — যেকোনো C-সামঞ্জস্যপূর্ণ প্রজেক্টে এমবেড করার জন্য C হেডার সহ স্ট্যাটিক লাইব্রেরি
- **Python** — `pip install cypherlite`-এর মাধ্যমে PyO3-ভিত্তিক বাইন্ডিং
- **Go** — `go get github.com/Epsilondelta-ai/CypherLite/bindings/go/cypherlite`-এর মাধ্যমে CGo বাইন্ডিং
- **Node.js** — `npm install cypherlite`-এর মাধ্যমে napi-rs নেটিভ অ্যাডঅন

---

## দ্রুত শুরু

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

## ইনস্টলেশন

### Rust (Cargo)

```toml
[dependencies]
cypherlite-query = "1.2"

# ঐচ্ছিক: নির্দিষ্ট ফিচার ফ্ল্যাগ সক্রিয় করুন
# cypherlite-query = { version = "1.2", features = ["temporal-edge", "plugin"] }
```

### Python (pip)

```bash
pip install cypherlite
```

সমস্ত ফিচার সহ সোর্স থেকে বিল্ড করুন:

```bash
cd crates/cypherlite-python
pip install maturin
maturin develop --release
```

### Go (go get)

```bash
go get github.com/Epsilondelta-ai/CypherLite/bindings/go/cypherlite
```

প্রয়োজনীয়তা: Go 1.21+, Rust টুলচেইন (C স্ট্যাটিক লাইব্রেরি বিল্ড করতে), এবং CGo-র জন্য C কম্পাইলার।

### Node.js (npm)

```bash
npm install cypherlite
```

সোর্স থেকে বিল্ড করুন:

```bash
cd crates/cypherlite-node
npx napi build --release
```

প্রয়োজনীয়তা: N-API v9 সাপোর্ট সহ Node.js 18+ এবং Rust টুলচেইন।

### C (হেডার + স্ট্যাটিক লাইব্রেরি)

```bash
cargo build -p cypherlite-ffi --release --all-features
```

`target/release/libcypherlite_ffi.a` লিংক করুন এবং জেনারেট হওয়া `cypherlite.h` হেডার অন্তর্ভুক্ত করুন।

---

## আর্কিটেকচার

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

**Crate ডিপেন্ডেন্সি গ্রাফ:**

```
cypherlite-query
    └── cypherlite-storage
            └── cypherlite-core

cypherlite-ffi
    └── cypherlite-query

cypherlite-python  (wraps cypherlite-ffi via PyO3)
cypherlite-node    (wraps cypherlite-ffi via napi-rs)
```

**কোয়েরি এক্সিকিউশন পাইপলাইন:**

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

## ফিচার ফ্ল্যাগ

ফিচার ফ্ল্যাগ সংযোজনযোগ্য। প্রতিটি ফ্ল্যাগ টেবিলে উপরে তালিকাভুক্ত সমস্ত ফ্ল্যাগের ফিচার সক্রিয় করে, `plugin` ছাড়া যা স্বাধীন।

| ফ্ল্যাগ | ডিফল্ট | বিবরণ |
|---------|--------|-------|
| `temporal-core` | হ্যাঁ | মূল টেম্পোরাল ফিচার (`AT TIME` কোয়েরি, ভার্সন স্টোর) |
| `temporal-edge` | না | টেম্পোরাল এজ ভার্সনিং এবং টেম্পোরাল রিলেশনশিপ কোয়েরি |
| `subgraph` | না | সাবগ্রাফ এন্টিটি (`CREATE / MATCH SNAPSHOT`) |
| `hypergraph` | না | নেটিভ N:M হাইপারএজ (`HYPEREDGE` সিনট্যাক্স); `subgraph` অন্তর্ভুক্ত |
| `full-temporal` | না | সমস্ত টেম্পোরাল ফিচার একত্রিত |
| `plugin` | না | প্লাগইন সিস্টেম — ৪টি প্লাগইন টাইপ, নিষ্ক্রিয় হলে জিরো ওভারহেড |

`Cargo.toml`-এ ফ্ল্যাগ সক্রিয় করুন:

```toml
cypherlite-query = { version = "1.2", features = ["hypergraph", "plugin"] }
```

---

## পারফরম্যান্স

Apple M2-এ Criterion দিয়ে চালানো বেঞ্চমার্ক (সিঙ্গেল-থ্রেডেড, ইন-মেমোরি WAL ফ্লাশ নিষ্ক্রিয়):

| অপারেশন | থ্রুপুট |
|---------|---------|
| নোড INSERT | ~180,000 ops/sec |
| ID দিয়ে নোড LOOKUP | ~950,000 ops/sec |
| এজ INSERT | ~160,000 ops/sec |
| সরল MATCH কোয়েরি | ~120,000 queries/sec |
| WAL রাইট থ্রুপুট | ~450 MB/sec |

স্থানীয়ভাবে বেঞ্চমার্ক চালান:

```bash
cargo bench --workspace --all-features
```

---

## পরীক্ষা

```bash
# সমস্ত পরীক্ষা (ডিফল্ট ফিচার)
cargo test --workspace

# সমস্ত ফিচার সহ সমস্ত পরীক্ষা
cargo test --workspace --all-features

# কভারেজ রিপোর্ট
cargo llvm-cov --workspace --all-features --summary-only

# লিন্টার (জিরো ওয়ার্নিং প্রয়োগ)
cargo clippy --workspace --all-targets --all-features -- -D warnings

# বেঞ্চমার্ক স্মোক টেস্ট
cargo bench --workspace --all-features -- --test
```

**টেস্ট স্যুট**: ওয়ার্কস্পেস জুড়ে ~১,৪৯০টি পরীক্ষা, সমস্ত ফিচার সক্রিয়, ০টি clippy ওয়ার্নিং, ৮৫%+ কভারেজ।

---

## ডকুমেন্টেশন

- **API রেফারেন্স (docs.rs)**: [docs.rs/cypherlite-query](https://docs.rs/cypherlite-query)
- **ডকুমেন্টেশন ওয়েবসাইট**: [Epsilondelta-ai.github.io/CypherLite](https://Epsilondelta-ai.github.io/CypherLite)
- **দ্রুত শুরুর উদাহরণ**: [`examples/`](../../examples/) — Rust, Python, Go এবং Node.js স্ক্রিপ্ট
- **FFI বাইন্ডিং উদাহরণ**: [`bindings/`](../../bindings/) — সম্পূর্ণ টেস্ট কভারেজ সহ Go প্যাকেজ

---

## অবদান রাখুন

অবদান স্বাগত। অনুগ্রহ করে নিম্নলিখিতের জন্য [CONTRIBUTING.md](../../CONTRIBUTING.md) পড়ুন:

- বাগ রিপোর্টিং নির্দেশিকা
- ব্রাঞ্চ নামকরণ এবং পুল রিকোয়েস্ট প্রক্রিয়া
- ডেভেলপমেন্ট সেটআপ (Rust 1.84+)
- কোড স্টাইল: `cargo fmt`, `cargo clippy -- -D warnings`
- পরীক্ষার প্রয়োজনীয়তা: প্রতি কমিটে ৮৫%+ কভারেজ

উল্লেখযোগ্য পরিবর্তনের জন্য বাস্তবায়নের আগে পদ্ধতি নিয়ে আলোচনা করতে প্রথমে একটি [ইস্যু](https://github.com/Epsilondelta-ai/CypherLite/issues) খুলুন।

---

## লাইসেন্স

নিম্নলিখিতের যেকোনো একটির অধীনে লাইসেন্সপ্রাপ্ত:

- [MIT License](../../LICENSE-MIT)
- [Apache License, Version 2.0](../../LICENSE-APACHE)

আপনার পছন্দ অনুযায়ী।

---

## অবস্থা / রোডম্যাপ

| পর্যায় | সংস্করণ | ফিচার | অবস্থা |
|--------|---------|-------|--------|
| ১ | v0.1 | স্টোরেজ ইঞ্জিন (WAL, B+Tree, ACID) | সম্পূর্ণ |
| ২ | v0.2 | কোয়েরি ইঞ্জিন (Cypher লেক্সার, পার্সার, এক্সিকিউটর) | সম্পূর্ণ |
| ৩ | v0.3 | উন্নত কোয়েরি (MERGE, WITH, ORDER BY, অপ্টিমাইজার) | সম্পূর্ণ |
| ৪ | v0.4 | টেম্পোরাল কোর (AT TIME, ভার্সন স্টোর) | সম্পূর্ণ |
| ৫ | v0.5 | টেম্পোরাল এজ (এজ ভার্সনিং) | সম্পূর্ণ |
| ৬ | v0.6 | সাবগ্রাফ এন্টিটি (SubgraphStore, SNAPSHOT) | সম্পূর্ণ |
| ৭ | v0.7 | নেটিভ হাইপারএজ (N:M, HYPEREDGE সিনট্যাক্স) | সম্পূর্ণ |
| ৮ | v0.8 | ইনলাইন প্রপার্টি ফিল্টার (প্যাটার্ন ফিক্স) | সম্পূর্ণ |
| ৯ | v0.9 | CI/CD পাইপলাইন (GitHub Actions, ৬টি জব) | সম্পূর্ণ |
| ১০ | v1.0 | প্লাগইন সিস্টেম (৪টি প্লাগইন টাইপ, রেজিস্ট্রি) | সম্পূর্ণ |
| ১১ | v1.1 | পারফরম্যান্স অপ্টিমাইজেশন (বেঞ্চমার্ক, বাফার পুল) | সম্পূর্ণ |
| ১২ | v1.1 | FFI বাইন্ডিং (C, Python, Go, Node.js) | সম্পূর্ণ |
| ১৩ | v1.2 | ডকুমেন্টেশন ও i18n (rustdoc, ওয়েবসাইট, উদাহরণ) | **বর্তমান** |
