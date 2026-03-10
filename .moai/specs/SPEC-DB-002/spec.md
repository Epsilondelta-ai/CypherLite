---
id: SPEC-DB-002
version: "1.0.0"
status: complete
created: "2026-03-10"
updated: "2026-03-10"
author: epsilondelta
priority: critical
tags: [query-engine, cypher, parser, executor, openCypher]
lifecycle: spec-anchored
---

# SPEC-DB-002: CypherLite Phase 2 - openCypher Subset Query Engine (v0.2)

> CypherLite의 쿼리 엔진 레이어. Phase 1 스토리지 엔진 위에 openCypher 서브셋 파서, 플래너, 실행기를 구축하여 Cypher 문자열로 그래프를 쿼리하고 변경할 수 있는 완전한 쿼리 파이프라인을 제공한다.

---

## HISTORY

| Version | Date | Changes |
|---------|------|---------|
| 1.0.0 | 2026-03-10 | Initial SPEC creation based on research, requirements, architecture drafts |

---

## 1. Environment (환경)

### 1.1 시스템 환경

- **언어**: Rust 1.84+ (Edition 2021)
- **MSRV**: 1.84 (Phase 1과 동일)
- **대상 플랫폼**: Linux (x86_64), macOS (x86_64, aarch64), Windows (x86_64)
- **실행 모델**: 동기, 단일 스레드, WASM 호환 (`std::thread` 사용 금지)
- **WASM 빌드 타겟**: `wasm32-unknown-unknown` (쿼리 코어에 WASI 불필요)

### 1.2 크레이트 구조

- **신규 크레이트**: `cypherlite-query` (`crates/cypherlite-query/`)
- **의존 크레이트**: `cypherlite-core`, `cypherlite-storage`
- **신규 외부 의존성**: `logos = "0.14"` (렉서 전용, 컴파일 타임 코드 생성, 런타임 오버헤드 제로)

### 1.3 워크스페이스 업데이트

```toml
# Cargo.toml (workspace root)
[workspace]
members = [
    "crates/cypherlite-core",
    "crates/cypherlite-storage",
    "crates/cypherlite-query",   # Phase 2 신규
]
```

### 1.4 의존성 그래프

```
cypherlite-query
  +-- cypherlite-core    (LabelRegistry trait, PropertyValue, error types)
  +-- cypherlite-storage (StorageEngine, Catalog, scan APIs)
  +-- logos = "0.14"

cypherlite-storage
  +-- cypherlite-core    (NodeId, EdgeId, PropertyValue, CypherLiteError)

cypherlite-core
  (no dependencies)
```

### 1.5 `cypherlite-query/Cargo.toml`

```toml
[package]
name = "cypherlite-query"
version = "0.1.0"
edition = "2021"
rust-version = "1.84"

[dependencies]
cypherlite-core    = { path = "../cypherlite-core" }
cypherlite-storage = { path = "../cypherlite-storage" }
logos              = "0.14"

[dev-dependencies]
tempfile = "3"
proptest = "1"
```

---

## 2. Assumptions (가정)

### 2.1 스토리지 엔진 통합 가정

- **A-001**: StorageEngine은 레이블과 프로퍼티 키를 `u32` 정수 ID로 저장한다. 쿼리 엔진은 문자열-ID 양방향 매핑을 위한 SymbolTable(Catalog)이 필수적이다.
- **A-002**: StorageEngine은 `!Send`이다 (내부 `parking_lot::MutexGuard`의 `'static` lifetime transmute 때문). 실행기는 반드시 단일 스레드에서 동작해야 한다.
- **A-003**: Phase 1에는 `scan_nodes()`, `scan_nodes_by_label()`, `scan_edges_by_type()` API가 없다. 이 스캔 메서드들을 `cypherlite-storage`에 추가해야 한다.
- **A-004**: `get_edges_for_node()`는 O(E) 복잡도이다 (전체 엣지 선형 스캔). 인덱스 최적화는 Phase 3으로 연기한다.
- **A-005**: `PropertyValue`에 `PartialOrd`가 구현되어 있지 않다. 쿼리 엔진이 타입별 비교 로직을 자체 구현해야 한다.
- **A-006**: Phase 1 B-tree 데이터는 인메모리 전용이다. 프로세스 재시작 시 그래프 데이터가 소실된다. 쿼리 통합 테스트는 동일 엔진 세션 내에서 그래프를 생성해야 한다.

### 2.2 쿼리 엔진 가정

- **A-007**: Phase 2 쿼리 엔진은 비용 기반 옵티마이저 없이 규칙 기반 플래너를 사용한다. 비용 모델은 Phase 3 (인덱스 추가 시점)으로 연기한다.
- **A-008**: 쿼리 결과는 Phase 2에서 전체 수집(materialize) 방식이다. 진정한 스트리밍 QueryResult는 Phase 3으로 연기한다.
- **A-009**: Phase 2 실행기는 P0+P1 절만 실행한다. P2 절(ORDER BY, LIMIT, SKIP, MERGE, WITH)은 파싱은 하되 실행 시 `UnsupportedSyntax` 에러를 반환한다.

---

## 3. Requirements (요구사항)

### 모듈 1: Lexer/Tokenizer (REQ-LEX)

#### REQ-LEX-001 [Ubiquitous]
렉서는 **항상** 유효한 openCypher v1.0 쿼리 문자열을 타입이 지정된 토큰 시퀀스로 데이터 손실 없이 변환한다.

#### REQ-LEX-002 [Ubiquitous]
렉서는 **항상** 모든 키워드(MATCH, WHERE, RETURN 등)에 대해 대소문자를 구분하지 않으며, 식별자와 문자열 리터럴의 대소문자는 보존한다.

#### REQ-LEX-003 [Event-Driven]
**WHEN** 렉서가 인식할 수 없는 문자 시퀀스를 만나면 **THEN** 바이트 오프셋과 문제가 된 문자를 포함한 `LexError`를 발생시킨다.

#### REQ-LEX-004 [Ubiquitous]
렉서는 **항상** 다음 토큰 카테고리를 지원한다: Keywords, Identifiers, Integer literals, Float literals, String literals (작은따옴표/큰따옴표), Boolean literals (true/false), NULL literal, Operators, Punctuation, Comments (단일 행 `//` 전용).

#### REQ-LEX-005 [Ubiquitous]
렉서는 **항상** 노드 및 프로퍼티 이름에 대해 유니코드 식별자(UTF-8 인코딩)를 지원한다.

#### REQ-LEX-006 [State-Driven]
**IF** 문자열 리터럴 스캔 중이면 **THEN** 렉서는 이스케이프 시퀀스를 처리한다: `\n`, `\t`, `\r`, `\\`, `\'`, `\"`, `\uXXXX`.

#### REQ-LEX-007 [Ubiquitous]
렉서는 **항상** 모든 토큰에 대해 소스 위치(line, column, byte offset)를 보존하여 서술적 에러 메시지를 지원한다.

#### REQ-LEX-008 [Optional]
**가능하면** 입력이 비어 있거나 공백과 주석만 포함하는 경우 렉서는 에러 없이 빈 토큰 스트림을 생성한다.

---

### 모듈 2: Parser/AST (REQ-PARSE)

#### REQ-PARSE-001 [Ubiquitous]
파서는 **항상** 렉서의 토큰 스트림을 받아 쿼리 구조를 나타내는 타입이 지정된 AST(Abstract Syntax Tree)를 생성한다.

#### REQ-PARSE-002 [Ubiquitous]
AST는 **항상** 모든 v1.0 절을 표현한다: MATCH, OPTIONAL MATCH, CREATE, MERGE, SET, REMOVE, DELETE, DETACH DELETE, WITH, RETURN, ORDER BY, LIMIT, SKIP, UNWIND, WHERE.

#### REQ-PARSE-003 [Event-Driven]
**WHEN** 파서가 구문 오류를 만나면 **THEN** 문제가 된 토큰, 소스 위치(line, column), 예상되는 토큰이나 구문을 설명하는 사람이 읽을 수 있는 메시지를 포함한 `ParseError`를 반환한다.

#### REQ-PARSE-004 [Ubiquitous]
파서는 **항상** 절에서 사용된 모든 변수 이름이 같은 절에서 도입되었거나 선행 절에서 바인딩되었는지 검증한다 (시맨틱 스코핑 검사).

#### REQ-PARSE-005 [Ubiquitous]
AST는 **항상** 완전히 소유(owned)되어야 한다 (소스 문자열에 대한 borrow 없음). 이는 다단계 쿼리 계획 수립을 지원하기 위함이다.

#### REQ-PARSE-006 [Ubiquitous]
파서는 **항상** 선택적 범위(`*min..max`)를 포함한 가변 길이 관계를 포함하는 모든 v1.0 패턴 구문을 지원한다.

#### REQ-PARSE-007 [State-Driven]
**IF** 쿼리가 여러 절을 포함하면 **THEN** 파서는 절 시퀀스를 쿼리 플래너를 위해 읽기 순서를 보존하는 정렬된 리스트로 모델링한다.

#### REQ-PARSE-008 [Ubiquitous]
파서는 **항상** 재귀 하강(recursive descent) 파서로 구현하며, 컴파일 타임 예측 가능성과 WASM 호환성을 위해 외부 파서 생성기 의존성을 사용하지 않는다.

---

### 모듈 3: Query Planner (REQ-PLAN)

#### REQ-PLAN-001 [Ubiquitous]
쿼리 플래너는 **항상** 파싱된 AST를 물리 실행 계획(연산자 트리)으로 변환한다.

#### REQ-PLAN-002 [Ubiquitous]
플래너는 **항상** MATCH 절에 대해 최저 비용의 노드 스캔 또는 인덱스 스캔을 앵커 노드로 선택하는 규칙 기반 최적화를 적용한다.

#### REQ-PLAN-003 [Ubiquitous]
플래너는 **항상** WHERE 술어(predicate)를 데이터 소스에 최대한 가깝게 이동시킨다 (predicate pushdown).

#### REQ-PLAN-004 [Event-Driven]
**WHEN** MATCH 패턴이 레이블과 인덱싱된 프로퍼티에 대한 동일 술어를 모두 포함하는 노드를 포함하면 **THEN** 플래너는 전체 레이블 스캔보다 인덱스 스캔을 우선한다.

#### REQ-PLAN-005 [Ubiquitous]
플래너는 **항상** 저장된 통계(레이블별 노드 수, 타입별 엣지 수)를 사용하여 각 연산자의 카디널리티를 추정한다.

#### REQ-PLAN-006 [Ubiquitous]
플래너는 **항상** 먼저 논리 계획(관계 대수 스타일)을 생성한 후 물리 계획(이터레이터 모델)으로 변환한다.

#### REQ-PLAN-007 [State-Driven]
**IF** 쿼리가 LIMIT 없는 ORDER BY를 포함하면 **THEN** 플래너는 경고를 발생시키고 예상 결과 집합 크기에 적합한 정렬 알고리즘을 선택한다.

#### REQ-PLAN-008 [Ubiquitous]
플래너는 **항상** 카르테시안 곱(연결되지 않은 MATCH 패턴)을 감지하고 진단 경고를 발생시킨다. 이는 거의 항상 비의도적이기 때문이다.

#### REQ-PLAN-009 [State-Driven]
**IF** 가변 길이 경로 패턴을 계획 중이면 **THEN** 플래너는 무제한 순회를 방지하기 위해 설정 가능한 최대 홉 깊이(기본값: 10)를 적용한다.

---

### 모듈 4: Executor (REQ-EXEC)

#### REQ-EXEC-001 [Ubiquitous]
실행기는 **항상** Volcano/Iterator 모델(open/next/close 인터페이스)을 사용하여 스토리지 엔진에 대해 물리 계획을 평가한다.

#### REQ-EXEC-002 [Ubiquitous]
실행기는 **항상** SPEC-DB-001의 MVCC 트랜잭션 레이어와 통합하며, 모든 읽기 연산에 스냅샷 격리를 사용한다.

#### REQ-EXEC-003 [Event-Driven]
**WHEN** CREATE 절이 실행되면 **THEN** 실행기는 스토리지 엔진의 노드/엣지 삽입 API를 호출하고 새 엔티티의 ID를 쿼리 변수에 바인딩한다.

#### REQ-EXEC-004 [Event-Driven]
**WHEN** MERGE 절이 실행되면 **THEN** 실행기는 먼저 MATCH를 시도하고, 일치하는 항목이 없을 경우에만 동일 트랜잭션 내에서 CREATE 경로를 실행한다.

#### REQ-EXEC-005 [Event-Driven]
**WHEN** 여전히 관계가 있는 노드에 대해 DELETE 절이 실행되면 **THEN** DETACH DELETE가 사용되지 않은 경우 실행기는 `ConstraintError`를 반환한다.

#### REQ-EXEC-006 [Ubiquitous]
실행기는 **항상** openCypher 타입 프로모션 규칙(integer + float = float)에 따라 산술 표현식에 대한 타입 변환(coercion)을 수행한다.

#### REQ-EXEC-007 [Ubiquitous]
실행기는 **항상** 모든 불리언 및 비교 표현식에 대해 삼값 논리(TRUE, FALSE, NULL)를 구현한다.

#### REQ-EXEC-008 [State-Driven]
**IF** 가변 길이 경로 순회 실행 중이면 **THEN** 실행기는 순환 그래프에서 무한 루프를 방지하기 위해 방문한 엣지를 추적한다.

#### REQ-EXEC-009 [State-Driven]
**IF** GROUP BY 절 없이 RETURN에 집계 함수가 존재하면 **THEN** 실행기는 비집계 RETURN 표현식을 암시적 그룹핑 키로 처리한다.

#### REQ-EXEC-010 [Ubiquitous]
실행기는 **항상** 활성 트랜잭션 핸들, 변수 바인딩용 심볼 테이블, 설정 가능한 리소스 제한(스캔된 최대 행 수, 최대 메모리)을 포함하는 `QueryContext` 구조체를 노출한다.

---

### 모듈 5: Result Streaming (REQ-STREAM)

#### REQ-STREAM-001 [Ubiquitous]
결과 스트리밍 레이어는 **항상** 쿼리 결과를 Rust `Iterator<Item = Result<Row>>` 인터페이스로 노출하여 지연(lazy) 행별 소비를 가능하게 한다.

#### REQ-STREAM-002 [Ubiquitous]
`Row`는 **항상** 컬럼 이름에서 `PropertyValue` 변형으로의 맵이며, 실행기의 타입 정보를 보존한다.

#### REQ-STREAM-003 [Event-Driven]
**WHEN** 스트리밍 도중 실행기 에러가 발생하면 **THEN** 이터레이터는 다음 `next()` 호출에서 `Err(QueryError)`를 반환하고 이후 반복을 중단한다.

#### REQ-STREAM-004 [State-Driven]
**IF** LIMIT 절이 존재하면 **THEN** 스트리밍 레이어는 제한에 도달하자마자 실행기를 닫고 모든 리소스를 해제한다.

#### REQ-STREAM-005 [Ubiquitous]
스트리밍 레이어는 **항상** 비스트리밍 소비자를 위해 결과를 `Vec<Row>`로 수집하는 편의 메서드 `collect_all()`을 지원한다.

#### REQ-STREAM-006 [Ubiquitous]
결과 이터레이터는 **항상** `Send`여야 하며, 이는 쿼리 결과가 스레드 경계를 넘어 소비될 수 있도록 하기 위함이다 (비동기 런타임 호환성 필요).

#### REQ-STREAM-007 [Optional]
**가능하면** WASM 기능 플래그가 활성화된 경우 스트리밍 레이어는 단일 스레드 WASM 환경과 호환되는 동기식 비스레드 반복 인터페이스를 노출한다.

---

## 4. openCypher Subset Scope

### v1.0 지원 범위

| Priority | Clause / Feature | 상태 | 설명 |
|----------|-----------------|------|------|
| **P0 (MVP)** | `MATCH` + `RETURN` | 파싱 + 실행 | 핵심 읽기 경로 |
| **P0** | `CREATE` | 파싱 + 실행 | 핵심 쓰기 경로 (노드/엣지 삽입) |
| **P1** | `WHERE` | 파싱 + 실행 | 필수 필터링 |
| **P1** | `SET` / `REMOVE` | 파싱 + 실행 | 프로퍼티 변경 |
| **P1** | `DELETE` / `DETACH DELETE` | 파싱 + 실행 | 노드/엣지 삭제 |
| **P1** | Aggregations (`count`, `sum`, `avg`, `min`, `max`, `collect`) | 파싱 + 실행 | RETURN 내 집계 |
| **P1** | `$param` substitution | 파싱 + 실행 | 매개변수 바인딩 |
| **P2** | `MERGE` | 파싱만 | 멱등 upsert (실행 Phase 3) |
| **P2** | `WITH` | 파싱만 | 서브쿼리 체이닝 (실행 Phase 3) |
| **P2** | `ORDER BY` / `LIMIT` / `SKIP` | 파싱만 | 결과 페이지네이션 (실행 Phase 3) |
| **P2** | `OPTIONAL MATCH` | 파싱만 | Left-join 의미론 (실행 Phase 3) |
| **P2** | `UNWIND` | 파싱만 | 리스트 확장 (실행 Phase 3) |
| **P2** | Variable-length paths `*min..max` | 파싱만 | 가변 길이 경로 (실행 Phase 3) |

### v2.0 이후 연기된 기능

| Feature | 연기 사유 |
|---------|-----------|
| `CALL` subqueries | 서브쿼리 실행 컨텍스트 필요 |
| `UNION` / `UNION ALL` | 브랜치 간 스키마 호환성 검증 필요 |
| `FOREACH` | 표현식 내 mutation; MVCC와의 복잡한 상호작용 |
| Date/time functions | Temporal 기능은 Phase 3 (SPEC-DB-003) 계획 |
| `CASE WHEN` | 파서 복잡도 절감; WHERE + OPTIONAL MATCH로 에뮬레이션 가능 |
| QPP (Quantified Path Patterns) | openCypher 2.0 문법 |
| `shortestPath()` | 경로 알고리즘; v1.1 또는 플러그인 범위 |
| Full-text search | 역인덱스 필요 (플러그인 범위) |
| Graph algorithms | PageRank, community detection 등 (플러그인 범위) |

---

## 5. Architecture Overview (아키텍처 개요)

### 5.1 쿼리 파이프라인

```
Cypher 문자열
   |
   v
[Lexer] -- logos 0.14 DFA 기반 토크나이저
   |
   v
Token Stream
   |
   v
[Parser] -- 재귀 하강 파서 (hand-written)
   |
   v
AST (Abstract Syntax Tree)
   |
   v
[Semantic Analysis] -- 변수 스코프 검증, 레이블/타입 해석 (via LabelRegistry)
   |
   v
Annotated AST (u32 ID로 해석 완료)
   |
   v
[Logical Planner] -- 규칙 기반 최적화, predicate pushdown
   |
   v
Logical Plan
   |
   v
[Physical Planner] -- Volcano Iterator 연산자 트리로 변환
   |
   v
Physical Plan (operator tree)
   |
   v
[Executor] -- StorageEngine과 상호작용, MVCC 트랜잭션 통합
   |
   v
QueryResult (rows)
```

### 5.2 신규 컴포넌트

| Component | 위치 | 역할 |
|-----------|------|------|
| `Catalog` | `cypherlite-storage::catalog` | String <-> u32 양방향 매핑 (레이블, 프로퍼티 키, 관계 타입). 카탈로그 페이지에 영속화. |
| `LabelRegistry` trait | `cypherlite-core::traits` | 시맨틱 분석기가 문자열을 u32 ID로 해석하는 인터페이스. MockCatalog로 단위 테스트 가능. |
| `cypherlite-query` crate | `crates/cypherlite-query/` | 렉서, 파서, AST, 시맨틱 분석, 논리 플래너, 실행기, 공개 API 전체를 포함하는 신규 크레이트. |
| Scan APIs | `cypherlite-storage::StorageEngine` | `scan_nodes()`, `scan_nodes_by_label()`, `scan_edges_by_type()` 추가. |

### 5.3 `StorageEngine` 확장 (Phase 2 범위)

```rust
impl StorageEngine {
    // Phase 2 추가 메서드
    pub fn scan_nodes(&self) -> impl Iterator<Item = &NodeRecord>;
    pub fn scan_nodes_by_label(&self, label_id: u32) -> Vec<NodeRecord>;
    pub fn scan_edges_by_type(&self, type_id: u32) -> Vec<RelationshipRecord>;
}
```

### 5.4 공개 API 개요

```rust
pub struct CypherLite {
    engine: StorageEngine,
    // NOTE: CypherLite is NOT Send (StorageEngine is not Send)
}

impl CypherLite {
    pub fn open(config: DatabaseConfig) -> Result<Self>;
    pub fn execute(&mut self, cypher: &str) -> Result<QueryResult>;
    pub fn execute_with_params(&mut self, cypher: &str, params: &Params) -> Result<QueryResult>;
    pub fn begin(&mut self) -> Result<Transaction<'_>>;
}
```

### 5.5 에러 타입 확장

`CypherLiteError`에 추가될 Phase 2 변형:

| 변형 | 설명 |
|------|------|
| `ParseError { message, span }` | 렉서/파서 오류 (소스 위치 포함) |
| `SemanticError(String)` | 시맨틱 분석 오류 (미선언 변수 등) |
| `ExecutionError(String)` | 쿼리 실행 오류 (타입 불일치 등) |
| `UnsupportedSyntax(String)` | P2 절 실행 시도 시 발생 |

---

## 6. Traceability (추적성)

| 요구사항 | 구현 대상 | 테스트 대상 |
|----------|-----------|-------------|
| REQ-LEX-* | `cypherlite-query::lexer` | `tests/lexer_tests.rs` |
| REQ-PARSE-* | `cypherlite-query::parser` | `tests/parser_tests.rs` |
| REQ-PLAN-* | `cypherlite-query::planner` | `tests/planner_tests.rs` |
| REQ-EXEC-* | `cypherlite-query::executor` | `tests/executor_tests.rs` |
| REQ-STREAM-* | `cypherlite-query::api` | `tests/integration/query_tests.rs` |
| Catalog | `cypherlite-storage::catalog` | `tests/catalog_tests.rs` |
| Scan APIs | `cypherlite-storage::lib` | `tests/scan_tests.rs` |
| LabelRegistry | `cypherlite-core::traits` | `tests/traits_tests.rs` |
