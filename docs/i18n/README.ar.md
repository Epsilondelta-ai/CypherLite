<!-- CypherLite Documentation Translation -->
<!-- Source Language: English -->
<!-- Target Language: Arabic (العربية) -->
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

> بساطة SQLite لقواعد بيانات الرسوم البيانية.

محرك قاعدة بيانات رسومية خفيف الوزن ومدمج وأحادي الملف مكتوب بلغة Rust. يجلب CypherLite النشر بدون إعداد في ملف واحد إلى نظام قواعد بيانات الرسوم البيانية — مع امتثال كامل لـ ACID، ودعم أصلي لرسوم بيانات الخصائص، والاستعلامات الزمنية، وكيانات الرسوم البيانية الفرعية، والحواف الفائقة، ونظام إضافات قائم على الـ traits.

**متوفر بلغات أخرى**: [English](../../README.md) | [中文](README.zh.md) | [हिन्दी](README.hi.md) | [Español](README.es.md) | [Français](README.fr.md) | [বাংলা](README.bn.md) | [Português](README.pt.md) | [Русский](README.ru.md) | [한국어](README.ko.md)

---

## الميزات

### محرك التخزين

- **معاملات ACID** — الذرية والاتساق والعزل والمتانة الكاملة عبر Write-Ahead Logging
- **كاتب واحد / قراء متعددون** — نموذج تزامن متوافق مع SQLite باستخدام `parking_lot`
- **عزل اللقطة** — MVCC قائم على مؤشر إطار WAL للقراءات المتسقة
- **تخزين B+Tree** — بحث O(log n) عن العقد والحواف مع تجاور بدون فهرس
- **استعادة بعد الانهيار** — إعادة تشغيل WAL عند البدء لضمان الاتساق بعد الإيقاف غير المتوقع
- **رسم بياني للخصائص** — عقد وحواف ذات خصائص مكتوبة: `Null`، `Bool`، `Int64`، `Float64`، `String`، `Bytes`، `Array`
- **مكتبة مدمجة** — لا توجد عملية خادم، إعداد صفري، ملف `.cyl` واحد

### محرك الاستعلام (Cypher)

- **مجموعة فرعية من openCypher** — `MATCH`، `CREATE`، `MERGE`، `SET`، `DELETE`، `RETURN`، `WHERE`، `WITH`، `ORDER BY`
- **محلل نحوي تنازلي متكرر** — محلل مكتوب يدويًا مع تحليل تعبيرات Pratt وأكثر من 28 كلمة مفتاحية
- **التحليل الدلالي** — التحقق من نطاق المتغيرات وتحليل التسميات/الأنواع
- **محسّن قائم على التكلفة** — تحويل الخطة المنطقية إلى مادية مع دفع الشروط
- **منفّذ Volcano** — تنفيذ قائم على المكررات مع 12 عاملًا
- **المنطق ثلاثي القيم** — نشر كامل لـ NULL وفق مواصفة openCypher
- **فلتر الخصائص المضمّن** — دعم نمط `MATCH (n:Label {key: value})`

### الميزات الزمنية

- **استعلامات AT TIME** — استرجاع حالة الرسم البياني في نقطة زمنية محددة
- **مخزن الإصدارات** — سلسلة إصدارات خصائص غير قابلة للتغيير لكل عقدة وحافة
- **إصدار الحواف الزمنية** — طوابع زمنية لإنشاء/حذف الحواف مع استعلامات العلاقات الزمنية
- **التجميع الزمني** — استعلامات نطاق زمني مع دوال تجميع على البيانات المُصدَرة

### الرسم البياني الفرعي والحواف الفائقة

- **SubgraphStore** — رسوم بيانية فرعية مُسمَّاة كمكونات أولية مخزَّنة جانبًا مع العقد والحواف
- **CREATE / MATCH SNAPSHOT** — التقاط الرسوم البيانية الفرعية المُسمَّاة والاستعلام عنها
- **حواف فائقة أصلية** — علاقات N:M تربط عددًا عشوائيًا من العقد
- **صياغة HYPEREDGE** — `CREATE HYPEREDGE :TYPE CONNECTING (n1), (n2), (n3)`
- **TemporalRef** — أعضاء الحواف الفائقة يحملون بيانات وصفية للمرجع الزمني

### نظام الإضافات

- **ScalarFunction** — تسجيل دوال استعلام مخصصة قابلة للاستدعاء في تعبيرات Cypher
- **IndexPlugin** — تطبيقات فهرس مخصصة قابلة للتوصيل (مثل فهرس HNSW المتجهي)
- **Serializer** — إضافات تنسيق استيراد/تصدير مخصصة (مثل JSON-LD، GraphML)
- **Trigger** — خطافات قبل/بعد لعمليات `CREATE`، `DELETE`، `SET` مع دعم التراجع
- **PluginRegistry** — سجل عام آمن للخيوط قائم على `HashMap` (`Send + Sync`)
- **تكلفة صفرية** — علم الميزة `plugin`؛ مُحكَم بـ cfg، لا تكلفة عند التعطيل

### ارتباطات FFI

- **C ABI** — مكتبة ساكنة مع رأس C للتضمين في أي مشروع متوافق مع C
- **Python** — ارتباطات قائمة على PyO3 عبر `pip install cypherlite`
- **Go** — ارتباطات CGo عبر `go get github.com/Epsilondelta-ai/CypherLite/bindings/go/cypherlite`
- **Node.js** — إضافة أصلية napi-rs عبر `npm install cypherlite`

---

## البدء السريع

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

## التثبيت

### Rust (Cargo)

```toml
[dependencies]
cypherlite-query = "1.2"

# اختياري: تمكين أعلام ميزات محددة
# cypherlite-query = { version = "1.2", features = ["temporal-edge", "plugin"] }
```

### Python (pip)

```bash
pip install cypherlite
```

البناء من المصدر بجميع الميزات:

```bash
cd crates/cypherlite-python
pip install maturin
maturin develop --release
```

### Go (go get)

```bash
go get github.com/Epsilondelta-ai/CypherLite/bindings/go/cypherlite
```

المتطلبات: Go 1.21+، سلسلة أدوات Rust (لبناء المكتبة الساكنة C)، ومترجم C لـ CGo.

### Node.js (npm)

```bash
npm install cypherlite
```

البناء من المصدر:

```bash
cd crates/cypherlite-node
npx napi build --release
```

المتطلبات: Node.js 18+ مع دعم N-API v9 وسلسلة أدوات Rust.

### C (الرأس + المكتبة الساكنة)

```bash
cargo build -p cypherlite-ffi --release --all-features
```

اربط `target/release/libcypherlite_ffi.a` وأدرج الرأس المُولَّد `cypherlite.h`.

---

## البنية المعمارية

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

**رسم بياني لتبعيات crate:**

```
cypherlite-query
    └── cypherlite-storage
            └── cypherlite-core

cypherlite-ffi
    └── cypherlite-query

cypherlite-python  (wraps cypherlite-ffi via PyO3)
cypherlite-node    (wraps cypherlite-ffi via napi-rs)
```

**خط أنابيب تنفيذ الاستعلام:**

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

## أعلام الميزات

أعلام الميزات تراكمية. يُمكّن كل علم ميزات جميع الأعلام المدرجة فوقه في الجدول، باستثناء `plugin` الذي مستقل.

| العلم | الافتراضي | الوصف |
|-------|----------|-------|
| `temporal-core` | نعم | ميزات زمنية أساسية (استعلامات `AT TIME`، مخزن الإصدارات) |
| `temporal-edge` | لا | إصدار الحواف الزمنية واستعلامات العلاقات الزمنية |
| `subgraph` | لا | كيانات الرسم البياني الفرعي (`CREATE / MATCH SNAPSHOT`) |
| `hypergraph` | لا | حواف فائقة N:M أصلية (صياغة `HYPEREDGE`)؛ يستلزم `subgraph` |
| `full-temporal` | لا | جميع الميزات الزمنية مجمّعة |
| `plugin` | لا | نظام الإضافات — 4 أنواع إضافات، تكلفة صفرية عند التعطيل |

تمكين الأعلام في `Cargo.toml`:

```toml
cypherlite-query = { version = "1.2", features = ["hypergraph", "plugin"] }
```

---

## الأداء

اختبارات الأداء التي أُجريت باستخدام Criterion على Apple M2 (خيط واحد، مسح WAL في الذاكرة معطّل):

| العملية | الإنتاجية |
|---------|----------|
| إدخال عقدة INSERT | ~180,000 ops/sec |
| بحث عقدة LOOKUP بالمعرف | ~950,000 ops/sec |
| إدخال حافة INSERT | ~160,000 ops/sec |
| استعلام MATCH بسيط | ~120,000 queries/sec |
| إنتاجية كتابة WAL | ~450 MB/sec |

تشغيل اختبارات الأداء محليًا:

```bash
cargo bench --workspace --all-features
```

---

## الاختبار

```bash
# جميع الاختبارات (الميزات الافتراضية)
cargo test --workspace

# جميع الاختبارات بجميع الميزات
cargo test --workspace --all-features

# تقرير التغطية
cargo llvm-cov --workspace --all-features --summary-only

# المدقق (فرض صفر تحذيرات)
cargo clippy --workspace --all-targets --all-features -- -D warnings

# اختبار دخان لاختبارات الأداء
cargo bench --workspace --all-features -- --test
```

**مجموعة الاختبارات**: ~1,490 اختبارًا عبر مساحة العمل، جميع الميزات مُمكَّنة، 0 تحذيرات clippy، تغطية 85%+.

---

## التوثيق

- **مرجع API (docs.rs)**: [docs.rs/cypherlite-query](https://docs.rs/cypherlite-query)
- **موقع التوثيق**: [Epsilondelta-ai.github.io/CypherLite](https://Epsilondelta-ai.github.io/CypherLite)
- **أمثلة البدء السريع**: [`examples/`](../../examples/) — نصوص Rust وPython وGo وNode.js
- **أمثلة ارتباطات FFI**: [`bindings/`](../../bindings/) — حزمة Go بتغطية اختبارات كاملة

---

## المساهمة

المساهمات مرحّب بها. يُرجى قراءة [CONTRIBUTING.md](../../CONTRIBUTING.md) للاطلاع على:

- إرشادات الإبلاغ عن الأخطاء
- عملية تسمية الفروع وطلبات السحب
- إعداد بيئة التطوير (Rust 1.84+)
- أسلوب الكود: `cargo fmt`، `cargo clippy -- -D warnings`
- متطلبات الاختبار: تغطية 85%+ لكل commit

افتح [مشكلة](https://github.com/Epsilondelta-ai/CypherLite/issues) أولًا للتغييرات الكبيرة لمناقشة النهج قبل التنفيذ.

---

## الترخيص

مرخّص بموجب أحد الترخيصين:

- [MIT License](../../LICENSE-MIT)
- [Apache License, Version 2.0](../../LICENSE-APACHE)

حسب اختيارك.

---

## الحالة / خارطة الطريق

| المرحلة | الإصدار | الميزة | الحالة |
|---------|--------|--------|--------|
| 1 | v0.1 | محرك التخزين (WAL، B+Tree، ACID) | مكتمل |
| 2 | v0.2 | محرك الاستعلام (lexer وparser وexecutor لـ Cypher) | مكتمل |
| 3 | v0.3 | الاستعلامات المتقدمة (MERGE، WITH، ORDER BY، المحسّن) | مكتمل |
| 4 | v0.4 | النواة الزمنية (AT TIME، مخزن الإصدارات) | مكتمل |
| 5 | v0.5 | الحافة الزمنية (إصدار الحواف) | مكتمل |
| 6 | v0.6 | كيانات الرسم البياني الفرعي (SubgraphStore، SNAPSHOT) | مكتمل |
| 7 | v0.7 | الحافة الفائقة الأصلية (N:M، صياغة HYPEREDGE) | مكتمل |
| 8 | v0.8 | فلتر الخصائص المضمّن (إصلاح النمط) | مكتمل |
| 9 | v0.9 | خط أنابيب CI/CD (GitHub Actions، 6 مهام) | مكتمل |
| 10 | v1.0 | نظام الإضافات (4 أنواع إضافات، السجل) | مكتمل |
| 11 | v1.1 | تحسين الأداء (اختبارات الأداء، مجمع المخازن المؤقتة) | مكتمل |
| 12 | v1.1 | ارتباطات FFI (C، Python، Go، Node.js) | مكتمل |
| 13 | v1.2 | التوثيق و i18n (rustdoc، الموقع، الأمثلة) | **الحالي** |
