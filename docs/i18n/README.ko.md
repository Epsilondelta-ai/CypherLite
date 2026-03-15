<!-- CypherLite Documentation Translation -->
<!-- Source Language: English -->
<!-- Target Language: Korean (한국어) -->
<!-- Last Synced Commit: HEAD -->
<!-- Last Updated: 2026-03-15 -->

# CypherLite

![CI](https://github.com/Epsilondelta-ai/CypherLite/actions/workflows/ci.yml/badge.svg)
[![crates.io](https://img.shields.io/crates/v/cypherlite-query.svg)](https://crates.io/crates/cypherlite-query)
[![docs.rs](https://docs.rs/cypherlite-query/badge.svg)](https://docs.rs/cypherlite-query)
![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)
![MSRV](https://img.shields.io/badge/MSRV-1.84-orange.svg)

> 그래프 데이터베이스를 위한 SQLite 수준의 간결함.

Rust로 작성된 경량 임베디드 단일 파일 그래프 데이터베이스 엔진입니다. CypherLite는 완전한 ACID 준수, 네이티브 속성 그래프 지원, 시간 기반 쿼리, 서브그래프 엔티티, 하이퍼에지, 트레이트 기반 플러그인 시스템을 갖춘 제로 설정 단일 파일 배포를 그래프 데이터베이스 생태계에 제공합니다.

**다른 언어로 보기**: [English](../../README.md) | [中文](README.zh.md) | [हिन्दी](README.hi.md) | [Español](README.es.md) | [Français](README.fr.md) | [العربية](README.ar.md) | [বাংলা](README.bn.md) | [Português](README.pt.md) | [Русский](README.ru.md)

---

## 기능

### 스토리지 엔진

- **ACID 트랜잭션** — Write-Ahead Logging을 통한 완전한 원자성, 일관성, 격리성, 내구성
- **단일 쓰기 / 다중 읽기** — `parking_lot`을 사용한 SQLite 호환 동시성 모델
- **스냅샷 격리** — 일관된 읽기를 위한 WAL 프레임 인덱스 기반 MVCC
- **B+Tree 스토리지** — 인덱스 없는 인접성을 통한 O(log n) 노드 및 에지 조회
- **크래시 복구** — 예상치 못한 종료 후 일관성을 위한 시작 시 WAL 재생
- **속성 그래프** — 타입이 지정된 속성을 가진 노드와 에지: `Null`, `Bool`, `Int64`, `Float64`, `String`, `Bytes`, `Array`
- **임베디드 라이브러리** — 서버 프로세스 없음, 제로 설정, 단일 `.cyl` 파일

### 쿼리 엔진 (Cypher)

- **openCypher 서브셋** — `MATCH`, `CREATE`, `MERGE`, `SET`, `DELETE`, `RETURN`, `WHERE`, `WITH`, `ORDER BY`
- **재귀 하향 파서** — Pratt 표현식 파싱과 28개 이상의 키워드를 지원하는 수작업 파서
- **의미론적 분석** — 변수 범위 유효성 검사 및 레이블/타입 해결
- **비용 기반 옵티마이저** — 술어 푸시다운을 통한 논리적-물리적 플랜 변환
- **볼케이노 실행기** — 12개의 연산자를 갖춘 이터레이터 기반 실행
- **삼값 논리** — openCypher 명세에 따른 완전한 NULL 전파
- **인라인 속성 필터** — `MATCH (n:Label {key: value})` 패턴 지원

### 시간 기반 기능

- **AT TIME 쿼리** — 특정 시점의 그래프 상태 검색
- **버전 스토어** — 노드와 에지별 불변 속성 버전 체인
- **시간적 에지 버전 관리** — 에지 생성/삭제 타임스탬프와 시간적 관계 쿼리
- **시간적 집계** — 버전 관리된 데이터에 대한 집계 함수를 포함한 시간 범위 쿼리

### 서브그래프 및 하이퍼에지

- **SubgraphStore** — 노드 및 에지와 함께 저장된 일급 엔티티로서의 명명된 서브그래프
- **CREATE / MATCH SNAPSHOT** — 명명된 서브그래프 엔티티 캡처 및 쿼리
- **네이티브 하이퍼에지** — 임의의 수의 노드를 연결하는 N:M 관계
- **HYPEREDGE 구문** — `CREATE HYPEREDGE :TYPE CONNECTING (n1), (n2), (n3)`
- **TemporalRef** — 하이퍼에지 멤버는 시간적 참조 메타데이터를 포함

### 플러그인 시스템

- **ScalarFunction** — Cypher 표현식에서 호출 가능한 커스텀 쿼리 함수 등록
- **IndexPlugin** — 플러그 가능한 커스텀 인덱스 구현 (예: HNSW 벡터 인덱스)
- **Serializer** — 커스텀 가져오기/내보내기 형식 플러그인 (예: JSON-LD, GraphML)
- **Trigger** — 롤백 지원을 포함한 `CREATE`, `DELETE`, `SET` 작업의 사전/사후 훅
- **PluginRegistry** — 제네릭 스레드 안전 `HashMap` 기반 레지스트리 (`Send + Sync`)
- **제로 오버헤드** — `plugin` 기능 플래그; cfg 게이팅, 비활성화 시 비용 없음

### FFI 바인딩

- **C ABI** — C 호환 프로젝트에 임베딩을 위한 C 헤더가 포함된 정적 라이브러리
- **Python** — `pip install cypherlite`를 통한 PyO3 기반 바인딩
- **Go** — `go get github.com/Epsilondelta-ai/CypherLite/bindings/go/cypherlite`를 통한 CGo 바인딩
- **Node.js** — `npm install cypherlite`를 통한 napi-rs 네이티브 애드온

---

## 빠른 시작

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

## 설치

### Rust (Cargo)

```toml
[dependencies]
cypherlite-query = "1.2"

# 선택 사항: 특정 기능 플래그 활성화
# cypherlite-query = { version = "1.2", features = ["temporal-edge", "plugin"] }
```

### Python (pip)

```bash
pip install cypherlite
```

모든 기능을 포함하여 소스에서 빌드:

```bash
cd crates/cypherlite-python
pip install maturin
maturin develop --release
```

### Go (go get)

```bash
go get github.com/Epsilondelta-ai/CypherLite/bindings/go/cypherlite
```

요구사항: Go 1.21+, Rust 툴체인 (C 정적 라이브러리 빌드용), CGo용 C 컴파일러.

### Node.js (npm)

```bash
npm install cypherlite
```

소스에서 빌드:

```bash
cd crates/cypherlite-node
npx napi build --release
```

요구사항: N-API v9 지원 Node.js 18+와 Rust 툴체인.

### C (헤더 + 정적 라이브러리)

```bash
cargo build -p cypherlite-ffi --release --all-features
```

`target/release/libcypherlite_ffi.a`를 링크하고 생성된 `cypherlite.h` 헤더를 포함하세요.

---

## 아키텍처

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

**Crate 의존성 그래프:**

```
cypherlite-query
    └── cypherlite-storage
            └── cypherlite-core

cypherlite-ffi
    └── cypherlite-query

cypherlite-python  (wraps cypherlite-ffi via PyO3)
cypherlite-node    (wraps cypherlite-ffi via napi-rs)
```

**쿼리 실행 파이프라인:**

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

## 기능 플래그

기능 플래그는 누적됩니다. 각 플래그는 테이블에서 위에 나열된 모든 플래그의 기능을 활성화하며, 독립적인 `plugin`은 예외입니다.

| 플래그 | 기본값 | 설명 |
|--------|--------|------|
| `temporal-core` | 예 | 핵심 시간 기반 기능 (`AT TIME` 쿼리, 버전 스토어) |
| `temporal-edge` | 아니오 | 시간적 에지 버전 관리 및 시간적 관계 쿼리 |
| `subgraph` | 아니오 | 서브그래프 엔티티 (`CREATE / MATCH SNAPSHOT`) |
| `hypergraph` | 아니오 | 네이티브 N:M 하이퍼에지 (`HYPEREDGE` 구문); `subgraph` 포함 |
| `full-temporal` | 아니오 | 모든 시간 기반 기능 결합 |
| `plugin` | 아니오 | 플러그인 시스템 — 4가지 플러그인 유형, 비활성화 시 제로 오버헤드 |

`Cargo.toml`에서 플래그 활성화:

```toml
cypherlite-query = { version = "1.2", features = ["hypergraph", "plugin"] }
```

---

## 성능

Apple M2에서 Criterion으로 실행한 벤치마크 (단일 스레드, 메모리 WAL 플러시 비활성화):

| 작업 | 처리량 |
|------|--------|
| 노드 INSERT | ~180,000 ops/sec |
| ID로 노드 LOOKUP | ~950,000 ops/sec |
| 에지 INSERT | ~160,000 ops/sec |
| 단순 MATCH 쿼리 | ~120,000 queries/sec |
| WAL 쓰기 처리량 | ~450 MB/sec |

로컬에서 벤치마크 실행:

```bash
cargo bench --workspace --all-features
```

---

## 테스트

```bash
# 모든 테스트 (기본 기능)
cargo test --workspace

# 모든 기능으로 모든 테스트
cargo test --workspace --all-features

# 커버리지 보고서
cargo llvm-cov --workspace --all-features --summary-only

# 린터 (제로 경고 강제)
cargo clippy --workspace --all-targets --all-features -- -D warnings

# 벤치마크 스모크 테스트
cargo bench --workspace --all-features -- --test
```

**테스트 스위트**: 워크스페이스 전체 ~1,490개 테스트, 모든 기능 활성화, clippy 경고 0개, 커버리지 85% 이상.

---

## 문서

- **API 참조 (docs.rs)**: [docs.rs/cypherlite-query](https://docs.rs/cypherlite-query)
- **문서 웹사이트**: [Epsilondelta-ai.github.io/CypherLite](https://Epsilondelta-ai.github.io/CypherLite)
- **빠른 시작 예제**: [`examples/`](../../examples/) — Rust, Python, Go, Node.js 스크립트
- **FFI 바인딩 예제**: [`bindings/`](../../bindings/) — 전체 테스트 커버리지가 있는 Go 패키지

---

## 기여하기

기여를 환영합니다. 다음 사항에 대해 [CONTRIBUTING.md](../../CONTRIBUTING.md)를 읽어주세요:

- 버그 보고 가이드라인
- 브랜치 명명 및 풀 리퀘스트 프로세스
- 개발 환경 설정 (Rust 1.84+)
- 코드 스타일: `cargo fmt`, `cargo clippy -- -D warnings`
- 테스트 요구사항: 커밋당 85% 이상 커버리지

중요한 변경 사항의 경우 구현 전에 접근 방식을 논의하기 위해 먼저 [이슈](https://github.com/Epsilondelta-ai/CypherLite/issues)를 열어주세요.

---

## 라이선스

다음 중 하나의 라이선스 하에 제공됩니다:

- [MIT License](../../LICENSE-MIT)
- [Apache License, Version 2.0](../../LICENSE-APACHE)

선택은 귀하에게 있습니다.

---

## 상태 / 로드맵

| 단계 | 버전 | 기능 | 상태 |
|------|------|------|------|
| 1 | v0.1 | 스토리지 엔진 (WAL, B+Tree, ACID) | 완료 |
| 2 | v0.2 | 쿼리 엔진 (Cypher 렉서, 파서, 실행기) | 완료 |
| 3 | v0.3 | 고급 쿼리 (MERGE, WITH, ORDER BY, 옵티마이저) | 완료 |
| 4 | v0.4 | 시간 기반 핵심 (AT TIME, 버전 스토어) | 완료 |
| 5 | v0.5 | 시간적 에지 (에지 버전 관리) | 완료 |
| 6 | v0.6 | 서브그래프 엔티티 (SubgraphStore, SNAPSHOT) | 완료 |
| 7 | v0.7 | 네이티브 하이퍼에지 (N:M, HYPEREDGE 구문) | 완료 |
| 8 | v0.8 | 인라인 속성 필터 (패턴 수정) | 완료 |
| 9 | v0.9 | CI/CD 파이프라인 (GitHub Actions, 6개 작업) | 완료 |
| 10 | v1.0 | 플러그인 시스템 (4가지 플러그인 유형, 레지스트리) | 완료 |
| 11 | v1.1 | 성능 최적화 (벤치마크, 버퍼 풀) | 완료 |
| 12 | v1.1 | FFI 바인딩 (C, Python, Go, Node.js) | 완료 |
| 13 | v1.2 | 문서화 및 i18n (rustdoc, 웹사이트, 예제) | **현재** |
