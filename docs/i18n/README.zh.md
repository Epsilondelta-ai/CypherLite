<!-- CypherLite Documentation Translation -->
<!-- Source Language: English -->
<!-- Target Language: Chinese Simplified (中文) -->
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

> 图数据库的 SQLite 级简洁体验。

一款用 Rust 编写的轻量级嵌入式单文件图数据库引擎。CypherLite 将零配置、单文件部署带入图数据库生态系统——支持完整 ACID 合规、原生属性图、时态查询、子图实体、超边以及基于 trait 的插件系统。

**其他语言版本**: [English](../../README.md) | [हिन्दी](README.hi.md) | [Español](README.es.md) | [Français](README.fr.md) | [العربية](README.ar.md) | [বাংলা](README.bn.md) | [Português](README.pt.md) | [Русский](README.ru.md) | [한국어](README.ko.md)

---

## 功能特性

### 存储引擎

- **ACID 事务** — 通过预写式日志（WAL）实现完整的原子性、一致性、隔离性和持久性
- **单写多读** — 使用 `parking_lot` 实现的 SQLite 兼容并发模型
- **快照隔离** — 基于 WAL 帧索引的 MVCC 实现一致性读取
- **B+Tree 存储** — O(log n) 节点和边的查找，支持无索引邻接
- **崩溃恢复** — 启动时重放 WAL 日志，确保意外关机后的数据一致性
- **属性图** — 节点和边支持多种类型的属性：`Null`、`Bool`、`Int64`、`Float64`、`String`、`Bytes`、`Array`
- **嵌入式库** — 无服务器进程，零配置，单个 `.cyl` 文件

### 查询引擎（Cypher）

- **openCypher 子集** — 支持 `MATCH`、`CREATE`、`MERGE`、`SET`、`DELETE`、`RETURN`、`WHERE`、`WITH`、`ORDER BY`
- **递归下降解析器** — 手写解析器，配合 Pratt 表达式解析，支持 28+ 个关键字
- **语义分析** — 变量作用域验证及标签/类型解析
- **基于代价的优化器** — 逻辑到物理计划转换，支持谓词下推
- **火山模型执行器** — 基于迭代器的执行方式，支持 12 种操作符
- **三值逻辑** — 完整的 NULL 传播，符合 openCypher 规范
- **内联属性过滤** — 支持 `MATCH (n:Label {key: value})` 模式

### 时态特性

- **AT TIME 查询** — 点时间图状态检索
- **版本存储** — 每个节点和边维护不可变属性版本链
- **时态边版本控制** — 边的创建/删除时间戳，支持时态关系查询
- **时态聚合** — 针对版本化数据的时间范围查询与聚合函数

### 子图与超边

- **SubgraphStore** — 命名子图作为一等实体，与节点和边并列存储
- **CREATE / MATCH SNAPSHOT** — 捕获并查询命名子图实体
- **原生超边** — 连接任意数量节点的 N:M 关系
- **HYPEREDGE 语法** — `CREATE HYPEREDGE :TYPE CONNECTING (n1), (n2), (n3)`
- **TemporalRef** — 超边成员携带时态引用元数据

### 插件系统

- **ScalarFunction** — 注册可在 Cypher 表达式中调用的自定义查询函数
- **IndexPlugin** — 可插拔的自定义索引实现（例如 HNSW 向量索引）
- **Serializer** — 自定义导入/导出格式插件（例如 JSON-LD、GraphML）
- **Trigger** — 针对 `CREATE`、`DELETE`、`SET` 操作的前后钩子，支持回滚
- **PluginRegistry** — 泛型线程安全的基于 `HashMap` 的注册表（`Send + Sync`）
- **零开销** — 通过 `plugin` feature flag 进行 cfg 门控，禁用时无性能损耗

### FFI 绑定

- **C ABI** — 带有 C 头文件的静态库，可嵌入任何兼容 C 的项目
- **Python** — 基于 PyO3 的绑定，通过 `pip install cypherlite` 安装
- **Go** — CGo 绑定，通过 `go get github.com/Epsilondelta-ai/CypherLite/bindings/go/cypherlite` 获取
- **Node.js** — 基于 napi-rs 的原生插件，通过 `npm install cypherlite` 安装

---

## 快速开始

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

## 安装

### Rust (Cargo)

```toml
[dependencies]
cypherlite-query = "1.2"

# 可选：启用特定 feature flag
# cypherlite-query = { version = "1.2", features = ["temporal-edge", "plugin"] }
```

### Python (pip)

```bash
pip install cypherlite
```

从源码构建（包含所有特性）：

```bash
cd crates/cypherlite-python
pip install maturin
maturin develop --release
```

### Go (go get)

```bash
go get github.com/Epsilondelta-ai/CypherLite/bindings/go/cypherlite
```

依赖：Go 1.21+、Rust 工具链（用于构建 C 静态库）以及用于 CGo 的 C 编译器。

### Node.js (npm)

```bash
npm install cypherlite
```

从源码构建：

```bash
cd crates/cypherlite-node
npx napi build --release
```

依赖：Node.js 18+（支持 N-API v9）以及 Rust 工具链。

### C（头文件 + 静态库）

```bash
cargo build -p cypherlite-ffi --release --all-features
```

链接 `target/release/libcypherlite_ffi.a` 并包含生成的 `cypherlite.h` 头文件。

---

## 架构

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

**Crate 依赖图：**

```
cypherlite-query
    └── cypherlite-storage
            └── cypherlite-core

cypherlite-ffi
    └── cypherlite-query

cypherlite-python  (wraps cypherlite-ffi via PyO3)
cypherlite-node    (wraps cypherlite-ffi via napi-rs)
```

**查询执行流程：**

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

Feature flag 是累加的。每个 flag 会启用表中其上方所有 flag 的功能，`plugin` 除外（它是独立的）。

| Flag | 默认值 | 描述 |
|------|--------|------|
| `temporal-core` | 是 | 核心时态特性（`AT TIME` 查询、版本存储） |
| `temporal-edge` | 否 | 时态边版本控制和时态关系查询 |
| `subgraph` | 否 | 子图实体（`CREATE / MATCH SNAPSHOT`） |
| `hypergraph` | 否 | 原生 N:M 超边（`HYPEREDGE` 语法）；隐含启用 `subgraph` |
| `full-temporal` | 否 | 所有时态特性的组合 |
| `plugin` | 否 | 插件系统 — 4 种插件类型，禁用时零开销 |

在 `Cargo.toml` 中启用 flag：

```toml
cypherlite-query = { version = "1.2", features = ["hypergraph", "plugin"] }
```

---

## 性能

基准测试在 Apple M2 上使用 Criterion 运行（单线程，内存 WAL 刷新已禁用）：

| 操作 | 吞吐量 |
|------|--------|
| 节点 INSERT | ~180,000 ops/sec |
| 按 ID 查找节点 | ~950,000 ops/sec |
| 边 INSERT | ~160,000 ops/sec |
| 简单 MATCH 查询 | ~120,000 queries/sec |
| WAL 写入吞吐量 | ~450 MB/sec |

本地运行基准测试：

```bash
cargo bench --workspace --all-features
```

---

## 测试

```bash
# 所有测试（默认特性）
cargo test --workspace

# 所有测试（所有特性）
cargo test --workspace --all-features

# 覆盖率报告
cargo llvm-cov --workspace --all-features --summary-only

# 静态分析（强制零警告）
cargo clippy --workspace --all-targets --all-features -- -D warnings

# 基准测试冒烟测试
cargo bench --workspace --all-features -- --test
```

**测试套件**：整个工作区约 1,490 个测试，所有特性已启用，0 个 clippy 警告，覆盖率 85% 以上。

---

## 文档

- **API 参考 (docs.rs)**: [docs.rs/cypherlite-query](https://docs.rs/cypherlite-query)
- **文档网站**: [Epsilondelta-ai.github.io/CypherLite](https://Epsilondelta-ai.github.io/CypherLite)
- **快速开始示例**: [`examples/`](../../examples/) — Rust、Python、Go 和 Node.js 脚本
- **FFI 绑定示例**: [`bindings/`](../../bindings/) — 带完整测试覆盖的 Go 包

---

## 贡献

欢迎贡献。请阅读 [CONTRIBUTING.md](../../CONTRIBUTING.md) 了解：

- 错误报告指南
- 分支命名和拉取请求流程
- 开发环境搭建（Rust 1.84+）
- 代码风格：`cargo fmt`、`cargo clippy -- -D warnings`
- 测试要求：每次提交覆盖率 85% 以上

对于重大变更，请先提 [issue](https://github.com/Epsilondelta-ai/CypherLite/issues) 讨论实现方案。

---

## 许可证

双许可证，任选其一：

- [MIT License](../../LICENSE-MIT)
- [Apache License, Version 2.0](../../LICENSE-APACHE)

---

## 状态 / 路线图

| 阶段 | 版本 | 特性 | 状态 |
|------|------|------|------|
| 1 | v0.1 | 存储引擎（WAL、B+Tree、ACID） | 已完成 |
| 2 | v0.2 | 查询引擎（Cypher 词法器、解析器、执行器） | 已完成 |
| 3 | v0.3 | 高级查询（MERGE、WITH、ORDER BY、优化器） | 已完成 |
| 4 | v0.4 | 时态核心（AT TIME、版本存储） | 已完成 |
| 5 | v0.5 | 时态边（边版本控制） | 已完成 |
| 6 | v0.6 | 子图实体（SubgraphStore、SNAPSHOT） | 已完成 |
| 7 | v0.7 | 原生超边（N:M、HYPEREDGE 语法） | 已完成 |
| 8 | v0.8 | 内联属性过滤（模式修复） | 已完成 |
| 9 | v0.9 | CI/CD 流水线（GitHub Actions，6 个作业） | 已完成 |
| 10 | v1.0 | 插件系统（4 种插件类型、注册表） | 已完成 |
| 11 | v1.1 | 性能优化（基准测试、缓冲池） | 已完成 |
| 12 | v1.1 | FFI 绑定（C、Python、Go、Node.js） | 已完成 |
| 13 | v1.2 | 文档与 i18n（rustdoc、网站、示例） | **当前** |
