<!-- CypherLite Documentation Translation -->
<!-- Source Language: English -->
<!-- Target Language: Hindi (हिन्दी) -->
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

> ग्राफ़ डेटाबेस के लिए SQLite जैसी सरलता।

Rust में लिखा हुआ एक हल्का, एम्बेडेड, एकल-फ़ाइल ग्राफ़ डेटाबेस इंजन। CypherLite ग्राफ़ डेटाबेस इकोसिस्टम में ज़ीरो-कॉन्फ़िग, सिंगल-फ़ाइल डिप्लॉयमेंट लाता है — पूर्ण ACID अनुपालन, नेटिव प्रॉपर्टी ग्राफ़ सपोर्ट, टेम्पोरल क्वेरी, सबग्राफ एंटिटी, हाइपरएज और ट्रेट-आधारित प्लगइन सिस्टम के साथ।

**अन्य भाषाओं में उपलब्ध**: [English](../../README.md) | [中文](README.zh.md) | [Español](README.es.md) | [Français](README.fr.md) | [العربية](README.ar.md) | [বাংলা](README.bn.md) | [Português](README.pt.md) | [Русский](README.ru.md) | [한국어](README.ko.md)

---

## विशेषताएँ

### स्टोरेज इंजन

- **ACID ट्रांज़ैक्शन** — Write-Ahead Logging के ज़रिए पूर्ण परमाणुता, संगति, अलगाव और स्थायित्व
- **एकल-लेखक / बहु-पाठक** — `parking_lot` का उपयोग करते हुए SQLite-संगत कंकरेंसी मॉडल
- **स्नैपशॉट आइसोलेशन** — संगत रीड के लिए WAL फ्रेम इंडेक्स-आधारित MVCC
- **B+Tree स्टोरेज** — इंडेक्स-फ्री एडजेसेंसी के साथ O(log n) नोड और एज लुकअप
- **क्रैश रिकवरी** — अप्रत्याशित शटडाउन के बाद संगति के लिए स्टार्टअप पर WAL रिप्ले
- **प्रॉपर्टी ग्राफ़** — टाइप्ड प्रॉपर्टी के साथ नोड और एज: `Null`, `Bool`, `Int64`, `Float64`, `String`, `Bytes`, `Array`
- **एम्बेडेड लाइब्रेरी** — कोई सर्वर प्रक्रिया नहीं, ज़ीरो कॉन्फ़िगरेशन, एकल `.cyl` फ़ाइल

### क्वेरी इंजन (Cypher)

- **openCypher सबसेट** — `MATCH`, `CREATE`, `MERGE`, `SET`, `DELETE`, `RETURN`, `WHERE`, `WITH`, `ORDER BY`
- **रिकर्सिव डिसेंट पार्सर** — Pratt एक्सप्रेशन पार्सिंग और 28+ कीवर्ड के साथ हस्त-लिखित पार्सर
- **सेमांटिक एनालिसिस** — वेरिएबल स्कोप वैलिडेशन और लेबल/टाइप रेज़ोल्यूशन
- **कॉस्ट-बेस्ड ऑप्टिमाइज़र** — प्रेडिकेट पुशडाउन के साथ लॉजिकल-टू-फिज़िकल प्लान कन्वर्ज़न
- **वोल्केनो एक्ज़ीक्यूटर** — 12 ऑपरेटर के साथ इटरेटर-आधारित एक्ज़ीक्यूशन
- **त्रि-मूल्य तर्क** — openCypher विनिर्देश के अनुसार पूर्ण NULL प्रसार
- **इनलाइन प्रॉपर्टी फ़िल्टर** — `MATCH (n:Label {key: value})` पैटर्न सपोर्ट

### टेम्पोरल फ़ीचर

- **AT TIME क्वेरी** — किसी विशिष्ट समय बिंदु पर ग्राफ़ स्टेट रिट्रीवल
- **वर्ज़न स्टोर** — प्रत्येक नोड और एज के लिए अपरिवर्तनीय प्रॉपर्टी वर्ज़न चेन
- **टेम्पोरल एज वर्ज़निंग** — एज क्रिएशन/डिलीशन टाइमस्टैम्प और टेम्पोरल रिलेशनशिप क्वेरी
- **टेम्पोरल एग्रीगेशन** — वर्ज़न किए गए डेटा पर एग्रीगेट फ़ंक्शन के साथ टाइम-रेंज क्वेरी

### सबग्राफ और हाइपरएज

- **SubgraphStore** — नोड और एज के साथ-साथ फर्स्ट-क्लास एंटिटी के रूप में नेम्ड सबग्राफ
- **CREATE / MATCH SNAPSHOT** — नेम्ड सबग्राफ एंटिटी को कैप्चर और क्वेरी करें
- **नेटिव हाइपरएज** — मनमाने संख्या के नोड्स को जोड़ने वाले N:M रिलेशन
- **HYPEREDGE सिंटैक्स** — `CREATE HYPEREDGE :TYPE CONNECTING (n1), (n2), (n3)`
- **TemporalRef** — हाइपरएज सदस्य टेम्पोरल रेफरेंस मेटाडेटा वहन करते हैं

### प्लगइन सिस्टम

- **ScalarFunction** — Cypher एक्सप्रेशन में कॉल करने योग्य कस्टम क्वेरी फ़ंक्शन रजिस्टर करें
- **IndexPlugin** — प्लग करने योग्य कस्टम इंडेक्स इम्प्लीमेंटेशन (जैसे HNSW वेक्टर इंडेक्स)
- **Serializer** — कस्टम इम्पोर्ट/एक्सपोर्ट फॉर्मेट प्लगइन (जैसे JSON-LD, GraphML)
- **Trigger** — रोलबैक सपोर्ट के साथ `CREATE`, `DELETE`, `SET` ऑपरेशन के पहले/बाद हुक
- **PluginRegistry** — जेनेरिक, थ्रेड-सेफ `HashMap`-आधारित रजिस्ट्री (`Send + Sync`)
- **ज़ीरो ओवरहेड** — `plugin` फ़ीचर फ्लैग; cfg-गेटेड, अक्षम होने पर कोई लागत नहीं

### FFI बाइंडिंग

- **C ABI** — किसी भी C-संगत प्रोजेक्ट में एम्बेड करने के लिए C हेडर के साथ स्टैटिक लाइब्रेरी
- **Python** — `pip install cypherlite` के माध्यम से PyO3-आधारित बाइंडिंग
- **Go** — `go get github.com/Epsilondelta-ai/CypherLite/bindings/go/cypherlite` के माध्यम से CGo बाइंडिंग
- **Node.js** — `npm install cypherlite` के माध्यम से napi-rs नेटिव ऐडऑन

---

## त्वरित शुरुआत

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

## इंस्टॉलेशन

### Rust (Cargo)

```toml
[dependencies]
cypherlite-query = "1.2"

# वैकल्पिक: विशिष्ट फ़ीचर फ्लैग सक्षम करें
# cypherlite-query = { version = "1.2", features = ["temporal-edge", "plugin"] }
```

### Python (pip)

```bash
pip install cypherlite
```

सभी फ़ीचर के साथ सोर्स से बिल्ड करें:

```bash
cd crates/cypherlite-python
pip install maturin
maturin develop --release
```

### Go (go get)

```bash
go get github.com/Epsilondelta-ai/CypherLite/bindings/go/cypherlite
```

आवश्यकताएँ: Go 1.21+, Rust टूलचेन (C स्टैटिक लाइब्रेरी बिल्ड करने के लिए), और CGo के लिए C कंपाइलर।

### Node.js (npm)

```bash
npm install cypherlite
```

सोर्स से बिल्ड करें:

```bash
cd crates/cypherlite-node
npx napi build --release
```

आवश्यकताएँ: N-API v9 सपोर्ट के साथ Node.js 18+ और Rust टूलचेन।

### C (हेडर + स्टैटिक लाइब्रेरी)

```bash
cargo build -p cypherlite-ffi --release --all-features
```

`target/release/libcypherlite_ffi.a` लिंक करें और जेनरेट हुआ `cypherlite.h` हेडर शामिल करें।

---

## आर्किटेक्चर

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

**Crate डिपेंडेंसी ग्राफ़:**

```
cypherlite-query
    └── cypherlite-storage
            └── cypherlite-core

cypherlite-ffi
    └── cypherlite-query

cypherlite-python  (wraps cypherlite-ffi via PyO3)
cypherlite-node    (wraps cypherlite-ffi via napi-rs)
```

**क्वेरी एक्ज़ीक्यूशन पाइपलाइन:**

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

## फ़ीचर फ्लैग

फ़ीचर फ्लैग संचयी हैं। प्रत्येक फ्लैग तालिका में उसके ऊपर सूचीबद्ध सभी फ्लैग की फ़ीचर को सक्षम करता है, `plugin` को छोड़कर जो स्वतंत्र है।

| फ्लैग | डिफ़ॉल्ट | विवरण |
|-------|---------|-------|
| `temporal-core` | हाँ | मूल टेम्पोरल फ़ीचर (`AT TIME` क्वेरी, वर्ज़न स्टोर) |
| `temporal-edge` | नहीं | टेम्पोरल एज वर्ज़निंग और टेम्पोरल रिलेशनशिप क्वेरी |
| `subgraph` | नहीं | सबग्राफ एंटिटी (`CREATE / MATCH SNAPSHOT`) |
| `hypergraph` | नहीं | नेटिव N:M हाइपरएज (`HYPEREDGE` सिंटैक्स); `subgraph` इम्प्लाई करता है |
| `full-temporal` | नहीं | सभी टेम्पोरल फ़ीचर संयुक्त |
| `plugin` | नहीं | प्लगइन सिस्टम — 4 प्लगइन टाइप, अक्षम होने पर ज़ीरो ओवरहेड |

`Cargo.toml` में फ्लैग सक्षम करें:

```toml
cypherlite-query = { version = "1.2", features = ["hypergraph", "plugin"] }
```

---

## परफॉर्मेंस

Apple M2 पर Criterion के साथ चलाए गए बेंचमार्क (सिंगल-थ्रेडेड, इन-मेमोरी WAL फ्लश अक्षम):

| ऑपरेशन | थ्रूपुट |
|---------|---------|
| नोड INSERT | ~180,000 ops/sec |
| ID से नोड LOOKUP | ~950,000 ops/sec |
| एज INSERT | ~160,000 ops/sec |
| सरल MATCH क्वेरी | ~120,000 queries/sec |
| WAL राइट थ्रूपुट | ~450 MB/sec |

स्थानीय रूप से बेंचमार्क चलाएँ:

```bash
cargo bench --workspace --all-features
```

---

## टेस्टिंग

```bash
# सभी टेस्ट (डिफ़ॉल्ट फ़ीचर)
cargo test --workspace

# सभी फ़ीचर के साथ सभी टेस्ट
cargo test --workspace --all-features

# कवरेज रिपोर्ट
cargo llvm-cov --workspace --all-features --summary-only

# लिंटर (ज़ीरो वार्निंग लागू)
cargo clippy --workspace --all-targets --all-features -- -D warnings

# बेंचमार्क स्मोक टेस्ट
cargo bench --workspace --all-features -- --test
```

**टेस्ट सुइट**: वर्कस्पेस में ~1,490 टेस्ट, सभी फ़ीचर सक्षम, 0 clippy वार्निंग, 85%+ कवरेज।

---

## डॉक्यूमेंटेशन

- **API रेफरेंस (docs.rs)**: [docs.rs/cypherlite-query](https://docs.rs/cypherlite-query)
- **डॉक्यूमेंटेशन वेबसाइट**: [Epsilondelta-ai.github.io/CypherLite](https://Epsilondelta-ai.github.io/CypherLite)
- **त्वरित शुरुआत के उदाहरण**: [`examples/`](../../examples/) — Rust, Python, Go और Node.js स्क्रिप्ट
- **FFI बाइंडिंग उदाहरण**: [`bindings/`](../../bindings/) — पूर्ण टेस्ट कवरेज के साथ Go पैकेज

---

## योगदान

योगदान का स्वागत है। कृपया निम्न के लिए [CONTRIBUTING.md](../../CONTRIBUTING.md) पढ़ें:

- बग रिपोर्टिंग गाइडलाइन
- ब्रांच नेमिंग और पुल रिक्वेस्ट प्रक्रिया
- डेवलपमेंट सेटअप (Rust 1.84+)
- कोड स्टाइल: `cargo fmt`, `cargo clippy -- -D warnings`
- टेस्ट आवश्यकताएँ: प्रति कमिट 85%+ कवरेज

महत्वपूर्ण बदलावों के लिए, इम्प्लीमेंटेशन से पहले दृष्टिकोण पर चर्चा करने के लिए पहले एक [issue](https://github.com/Epsilondelta-ai/CypherLite/issues) खोलें।

---

## लाइसेंस

निम्न में से किसी एक के तहत लाइसेंस प्राप्त:

- [MIT License](../../LICENSE-MIT)
- [Apache License, Version 2.0](../../LICENSE-APACHE)

आपकी पसंद पर।

---

## स्थिति / रोडमैप

| चरण | संस्करण | फ़ीचर | स्थिति |
|-----|---------|-------|--------|
| 1 | v0.1 | स्टोरेज इंजन (WAL, B+Tree, ACID) | पूर्ण |
| 2 | v0.2 | क्वेरी इंजन (Cypher लेक्सर, पार्सर, एक्ज़ीक्यूटर) | पूर्ण |
| 3 | v0.3 | उन्नत क्वेरी (MERGE, WITH, ORDER BY, ऑप्टिमाइज़र) | पूर्ण |
| 4 | v0.4 | टेम्पोरल कोर (AT TIME, वर्ज़न स्टोर) | पूर्ण |
| 5 | v0.5 | टेम्पोरल एज (एज वर्ज़निंग) | पूर्ण |
| 6 | v0.6 | सबग्राफ एंटिटी (SubgraphStore, SNAPSHOT) | पूर्ण |
| 7 | v0.7 | नेटिव हाइपरएज (N:M, HYPEREDGE सिंटैक्स) | पूर्ण |
| 8 | v0.8 | इनलाइन प्रॉपर्टी फ़िल्टर (पैटर्न फिक्स) | पूर्ण |
| 9 | v0.9 | CI/CD पाइपलाइन (GitHub Actions, 6 जॉब) | पूर्ण |
| 10 | v1.0 | प्लगइन सिस्टम (4 प्लगइन टाइप, रजिस्ट्री) | पूर्ण |
| 11 | v1.1 | परफॉर्मेंस ऑप्टिमाइज़ेशन (बेंचमार्क, बफर पूल) | पूर्ण |
| 12 | v1.1 | FFI बाइंडिंग (C, Python, Go, Node.js) | पूर्ण |
| 13 | v1.2 | डॉक्यूमेंटेशन और i18n (rustdoc, वेबसाइट, उदाहरण) | **वर्तमान** |
