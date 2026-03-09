---
id: SPEC-DB-002
type: plan
version: "1.0.0"
status: draft
created: "2026-03-10"
updated: "2026-03-10"
tags: [query-engine, cypher, parser, executor, openCypher]
---

# SPEC-DB-002 Implementation Plan: CypherLite Query Engine

---

## 1. Technology Stack

| 기술 | 버전 | 용도 | 선택 근거 |
|------|------|------|-----------|
| `logos` | 0.14 | DFA 렉서 생성 | 컴파일 타임 코드 생성, 런타임 오버헤드 제로, MSRV 1.65 호환 |
| Hand-written recursive descent parser | N/A | AST 생성 | 커스텀 에러 복구, WASM 호환, 외부 의존성 없음 |
| Rule-based query planner | N/A | 쿼리 최적화 | Phase 2에서는 비용 모델 불필요; 규칙 기반으로 충분 |
| Volcano iterator model | N/A | 쿼리 실행 | 단일 스레드, 메모리 효율적, `StorageEngine` !Send 제약 준수 |

### 거부된 대안

| 대안 | 거부 사유 |
|------|-----------|
| `nom` 7.x | 키워드 밀집 문법에서 에러 메시지가 빈약함 |
| `pest` 2.x | PEG 백트래킹 최악 O(n^2); 에러 복구가 문법 로직과 분리됨 |
| `lalrpop` 0.22 | 빌드 타임 컴파일 단계 추가; shift-reduce 충돌 디버깅 어려움 |
| `smallvec` | 조기 최적화; Phase 3에서 프로파일링 후 도입 판단 |
| `indexmap` | HashMap으로 Phase 2 충분; 필요 시 Phase 3에서 도입 |

---

## 2. Crate Structure

```
crates/
  cypherlite-core/       (Phase 1 기존 + Phase 2 추가)
    src/
      traits.rs          + LabelRegistry trait 추가
      error.rs           + ParseError, SemanticError, ExecutionError, UnsupportedSyntax 변형 추가

  cypherlite-storage/    (Phase 1 기존 + Phase 2 추가)
    src/
      catalog/           [신규] Catalog 모듈 (BiMap 기반 String <-> u32 매핑)
        mod.rs           Catalog struct, save/load, LabelRegistry impl
      lib.rs             + scan_nodes(), scan_nodes_by_label(), scan_edges_by_type() 추가

  cypherlite-query/      [신규 크레이트]
    src/
      lib.rs             모듈 선언 및 공개 API re-export
      lexer/
        mod.rs           Token enum (#[derive(Logos)]), Span, lex() 함수
      parser/
        mod.rs           parse_query() 진입점
        ast.rs           모든 AST 노드 타입 정의
        expression.rs    Pratt 파서 (표현식 파싱)
        pattern.rs       패턴 파싱 (노드, 관계, 경로)
        clause.rs        절 파싱 (MATCH, CREATE, SET, DELETE 등)
      semantic/
        mod.rs           SemanticAnalyzer
        symbol_table.rs  쿼리 로컬 변수 바인딩 테이블
      planner/
        mod.rs           LogicalPlanner, 논리 계획 노드
        optimize.rs      규칙 기반 최적화 (predicate pushdown, label filter merge)
      executor/
        mod.rs           PhysicalOperator trait, 실행 진입점
        eval.rs          표현식 평가기 (eval, eval_cmp)
        operators/
          node_scan.rs   NodeScanOp
          expand.rs      ExpandOp (엣지 순회)
          filter.rs      FilterOp
          project.rs     ProjectOp
          limit.rs       LimitOp, SkipOp
          sort.rs        SortOp (전체 수집 후 정렬)
          aggregate.rs   AggregateOp
          create.rs      CreateOp
          delete.rs      DeleteOp
          set_props.rs   SetPropsOp
      api/
        mod.rs           CypherLite, QueryResult, Row, Transaction, Params, Value
```

---

## 3. Task Decomposition

### Group A: Catalog (cypherlite-core + cypherlite-storage 확장 -- 모든 쿼리 작업의 전제 조건)

| Task | 설명 | Crate | Priority |
|------|------|-------|----------|
| TASK-001 | `LabelRegistry` trait을 `cypherlite-core/src/traits.rs`에 추가 | core | High |
| TASK-002 | `ParseError`, `SemanticError`, `ExecutionError`, `UnsupportedSyntax` 변형을 `CypherLiteError`에 추가 | core | High |
| TASK-003 | BiMap 기반 `Catalog` struct 구현 (labels, prop_keys, rel_types 네임스페이스) | storage | High |
| TASK-004 | `Catalog::save()` / `load()` 구현 (PageManager 카탈로그 페이지 직렬화) | storage | High |
| TASK-005 | `Catalog`을 `StorageEngine`에 통합 (open 시 로드, `impl LabelRegistry`) | storage | High |

### Group B: Scan APIs (cypherlite-storage 확장 -- 실행기의 전제 조건)

| Task | 설명 | Crate | Priority |
|------|------|-------|----------|
| TASK-006 | `scan_nodes() -> impl Iterator<Item = &NodeRecord>` 추가 | storage | High |
| TASK-007 | `scan_nodes_by_label(label_id: u32) -> Vec<NodeRecord>` 추가 | storage | High |
| TASK-008 | `scan_edges_by_type(type_id: u32) -> Vec<RelationshipRecord>` 추가 | storage | High |
| TASK-009 | 세 스캔 메서드에 대한 테스트 작성 | storage | High |

### Group C: Workspace Scaffold (cypherlite-query 신규 크레이트)

| Task | 설명 | Crate | Priority |
|------|------|-------|----------|
| TASK-010 | 워크스페이스 `Cargo.toml`에 `cypherlite-query` 추가; Cargo.toml 생성 (logos dep) | query | High |
| TASK-011 | `src/lib.rs`에 모듈 스텁 생성: `lexer`, `parser`, `semantic`, `planner`, `executor`, `api` | query | High |

### Group D: Lexer

| Task | 설명 | Crate | Priority |
|------|------|-------|----------|
| TASK-012 | `#[derive(Logos)]`로 `Token` enum 정의; P0/P1/P2 모든 Cypher 키워드 | query | High |
| TASK-013 | 식별자, 정수, 부동소수점, 문자열 리터럴 토큰 규칙 추가 | query | High |
| TASK-014 | 연산자 및 구두점 토큰 추가 | query | High |
| TASK-015 | `Span` struct 정의; 렉서 출력에 통합 | query | High |
| TASK-016 | 단위 테스트: 키워드 구분, 문자열 이스케이프, 에러 토큰 처리 | query | High |

### Group E: Parser -- Expressions

| Task | 설명 | Crate | Priority |
|------|------|-------|----------|
| TASK-017 | `Expression`, `Literal`, `BinaryOp`, `UnaryOp` AST 타입 정의 (`parser/ast.rs`) | query | High |
| TASK-018 | Pratt 파서 구현: 산술 및 비교에 대한 우선순위 클라이밍 | query | High |
| TASK-019 | `parse_literal()`, `parse_parameter()` ($name 치환) 구현 | query | High |
| TASK-020 | `parse_function_call()` 구현: `name(args)` 및 `name(DISTINCT args)` | query | High |
| TASK-021 | `parse_property_access()` 구현: `n.name` 체인 접근 | query | High |
| TASK-022 | 단위 테스트: 우선순위, 비교, NOT/AND/OR, 함수 호출, $params | query | High |

### Group F: Parser -- Patterns

| Task | 설명 | Crate | Priority |
|------|------|-------|----------|
| TASK-023 | `NodePattern`, `RelationshipPattern`, `Pattern`, `PatternChain` AST 타입 정의 | query | High |
| TASK-024 | `parse_node_pattern()` 구현: `(n:Label {prop: val})` | query | High |
| TASK-025 | `parse_relationship_pattern()` 구현: 세 가지 방향 변형 | query | High |
| TASK-026 | `parse_pattern()` 구현: 전체 체인; `*` 범위에 대해 `UnsupportedSyntax` 발생 | query | High |
| TASK-027 | 단위 테스트: 단일 노드, 방향, 무방향, 레이블 없음, 다중 홉 | query | High |

### Group G: Parser -- Clauses

| Task | 설명 | Crate | Priority |
|------|------|-------|----------|
| TASK-028 | `parse_match_clause()` 구현: MATCH / OPTIONAL MATCH + WHERE | query | High |
| TASK-029 | `parse_return_clause()` 구현: RETURN [DISTINCT] items | query | High |
| TASK-030 | ORDER BY, SKIP, LIMIT 파싱 구현 (P2 실행은 연기하되, 파싱은 수행) | query | Medium |
| TASK-031 | `parse_create_clause()` 구현 | query | High |
| TASK-032 | `parse_set_clause()` 및 `parse_remove_clause()` 구현 | query | High |
| TASK-033 | `parse_delete_clause()` 구현 (DETACH 지원 포함) | query | High |
| TASK-034 | 최상위 `parse_query()` 구현: 절 파서에 디스패치 | query | High |
| TASK-035 | 통합 테스트: 전체 쿼리 라운드트립 (파싱 -> AST 형태 검증) | query | High |

### Group H: Semantic Analysis

| Task | 설명 | Crate | Priority |
|------|------|-------|----------|
| TASK-036 | 쿼리 로컬 `SymbolTable` 구현: 변수 바인딩, 스코프 규칙 | query | High |
| TASK-037 | `SemanticAnalyzer::analyze()` 구현: 변수 스코프 검증 | query | High |
| TASK-038 | `&mut dyn LabelRegistry`를 통한 레이블/관계 타입/프로퍼티 키 해석 구현 | query | High |
| TASK-039 | `MockCatalog`을 사용한 단위 테스트; 미선언 변수 에러 테스트 | query | High |

### Group I: Logical Planner

| Task | 설명 | Crate | Priority |
|------|------|-------|----------|
| TASK-040 | `LogicalPlan` enum 정의 (모든 연산자, 레이블/타입에 u32 ID 사용) | query | High |
| TASK-041 | `LogicalPlanner::plan()` 구현: MATCH -> NodeScan + Expand 체인 | query | High |
| TASK-042 | Predicate pushdown 및 label-filter merge 최적화 구현 | query | Medium |
| TASK-043 | 단위 테스트: 단일 노드 MATCH, 2홉 MATCH, MATCH+WHERE, MATCH+CREATE | query | High |

### Group J: Executor

| Task | 설명 | Crate | Priority |
|------|------|-------|----------|
| TASK-044 | `Value`, `Record`, `PhysicalOperator` trait, `Params` 정의 | query | High |
| TASK-045 | `eval()` 표현식 평가기 구현 (타입별 비교 `eval_cmp` 포함) | query | High |
| TASK-046 | `NodeScanOp` 구현 (`scan_nodes` / `scan_nodes_by_label`에 위임) | query | High |
| TASK-047 | `ExpandOp` 구현 (`next_edge_id` 링크드 리스트 순회, O(degree)) | query | High |
| TASK-048 | `FilterOp` 구현 (술어에 `eval()` 호출) | query | High |
| TASK-049 | `ProjectOp` 구현 (RETURN 표현식 평가, 컬럼 이름 변경) | query | High |
| TASK-050 | `LimitOp` 및 `SkipOp` 구현 | query | Medium |
| TASK-051 | `SortOp` 및 `AggregateOp` 구현 (전체 수집 방식) | query | Medium |
| TASK-052 | `CreateOp`, `DeleteOp`, `SetPropsOp` 구현 | query | High |
| TASK-053 | 각 연산자 단위 테스트; 타입 불일치, null 시맨틱 커버 eval 테스트 | query | High |

### Group K: Public API and Integration

| Task | 설명 | Crate | Priority |
|------|------|-------|----------|
| TASK-054 | `QueryResult`, `Row`, `FromValue` trait 구현 | query | High |
| TASK-055 | `CypherLite::open()`, `execute()`, `execute_with_params()` 구현 | query | High |
| TASK-056 | `CypherLite::begin()`, `Transaction::commit()`, `Transaction::rollback()` 구현 | query | High |
| TASK-057 | 엔드투엔드 통합 테스트: 실제 StorageEngine에 대한 MATCH+RETURN, CREATE, WHERE, SET, DELETE | query | High |
| TASK-058 | proptest: 무작위 토큰 시퀀스가 파서에서 패닉을 일으키지 않음 검증 | query | Medium |
| TASK-059 | 벤치마크: 단순 MATCH 및 2홉 MATCH에 대한 parse + plan + execute | query | Medium |

**총 59개 태스크, 11개 그룹.**

### Critical Path (의존성 순서)

```
A (Catalog) -> B (Scan APIs) -> C (Scaffold) -> D (Lexer) -> E+F (Parser)
-> G (Clauses) -> H (Semantic) -> I (Planner) -> J (Executor) -> K (API)
```

---

## 4. Milestone Ordering

### Primary Goal: P0 쿼리 실행 (MATCH + RETURN + CREATE)

- Group A: Catalog (TASK-001 ~ TASK-005)
- Group B: Scan APIs (TASK-006 ~ TASK-009)
- Group C: Scaffold (TASK-010 ~ TASK-011)
- Group D: Lexer (TASK-012 ~ TASK-016)
- Groups E+F: Parser - Expressions + Patterns (TASK-017 ~ TASK-027)
- Group G (일부): MATCH, RETURN, CREATE 절 파싱 (TASK-028, TASK-029, TASK-031, TASK-034)
- Group H: Semantic Analysis (TASK-036 ~ TASK-039)
- Group I (일부): MATCH -> NodeScan + Expand 계획 (TASK-040, TASK-041)
- Group J (일부): NodeScanOp, ExpandOp, ProjectOp, CreateOp (TASK-044 ~ TASK-049, TASK-052)
- Group K: Public API + 통합 테스트 (TASK-054 ~ TASK-057)

### Secondary Goal: P1 절 실행 (WHERE, SET, DELETE)

- Group G (나머지): SET, DELETE, REMOVE 절 파싱 (TASK-032, TASK-033)
- Group I: Predicate pushdown 최적화 (TASK-042)
- Group J (나머지): FilterOp, DeleteOp, SetPropsOp (TASK-048, TASK-052)

### Final Goal: P2 파싱 + 품질 게이트

- Group G: ORDER BY, SKIP, LIMIT 파싱 (TASK-030)
- Group J: LimitOp, SkipOp, SortOp, AggregateOp (TASK-050, TASK-051)
- proptest 및 벤치마크 (TASK-058, TASK-059)

---

## 5. Risk Analysis

### R-001: Grammar Scope Creep (문법 범위 확대)

- **위험**: Phase 2에서 openCypher를 너무 많이 구현하면 완료가 지연된다.
- **완화**: 파서는 P0+P1+P2 구문을 처리하지만, P2 절 실행 시 `UnsupportedSyntax`를 반환. 실행기는 P0+P1만 구현. P2 실행은 Phase 3.

### R-002: StorageEngine Mutability Borrow Checker

- **위험**: `StorageEngine`이 `!Send`이므로 사용자가 멀티 스레드 쿼리 실행이나 비동기 통합을 기대할 수 있다.
- **완화**: `CypherLite: !Send`를 공개 크레이트 루트에 문서화. 비동기 래퍼(`spawn_blocking`)는 FFI/바인딩 레이어 책임이며 코어 라이브러리가 아님. `mvcc.rs:48`의 `unsafe transmute`는 스레딩 추가 전에 해결 필요.

### R-003: Label Resolution Coupling

- **위험**: Catalog 변경(새 레이블 ID)이 내구성이 보장되어야 한다. CREATE가 새 레이블을 추가하면, 체크포인트 전 크래시 시 WAL에서 카탈로그 페이지를 리플레이해야 한다.
- **완화**: Catalog 쓰기는 트랜잭션 커밋 전에 `wal_write_page(CATALOG_PAGE_ID, ...)`를 통해 수행. 복구 시 기존 WAL 복구 경로에 의해 자동 리플레이됨.

### R-004: Parser Error Message Quality

- **위험**: 재귀 하강 파서에서 사용자 친화적인 에러 메시지를 생성하는 것이 어려울 수 있다.
- **완화**: 모든 `ParseError`에 소스 위치(line, column)와 예상 토큰 정보 포함. TASK-035의 통합 테스트에서 에러 메시지 품질 검증.

### R-007: Error Type Extension

- **위험**: `CypherLiteError`에 새 변형 추가 시 기존 `match` 패턴이 깨질 수 있다.
- **완화**: `#[non_exhaustive]` 속성 사용 여부 검토. 구체적 변형(`Box<dyn Error>` 대신)을 사용하여 완전한 패턴 매칭 유지. 기존 Phase 1 코드에 대한 변경 영향 최소화.

### R-008: logos MSRV Drift

- **위험**: logos가 향후 릴리즈에서 MSRV를 1.84 이상으로 올릴 수 있다.
- **완화**: `logos = "0.14"`로 고정. 업그레이드 전 검증.

### R-009: SortOp Memory

- **위험**: `SortOp`이 모든 행을 메모리에 수집하므로 대량 스캔 시 OOM 발생 가능.
- **완화**: `DatabaseConfig`에 `max_sort_rows: usize` (기본 100,000) 추가. 초과 시 `ExecutionError` 반환. 외부 정렬은 Phase 3으로 연기.

---

## 6. MX Tag Strategy

### @MX:ANCHOR 후보 (fan_in >= 3)

| 위치 | 근거 |
|------|------|
| `CypherLite::execute()` 공개 API | 모든 쿼리 경로의 진입점; 최상위 fan_in |
| `PhysicalOperator::next()` trait 메서드 | 모든 연산자가 구현; 실행 파이프라인 핵심 계약 |

### @MX:WARN 후보 (복잡한 생명주기/안전성)

| 위치 | 근거 |
|------|------|
| 실행기-스토리지 borrow 인터페이스 | `&mut StorageEngine` 생명주기가 쿼리 실행 동안 유지되어야 함; 복잡한 borrow 패턴 |
| `eval_cmp` 타입별 비교 | Null 시맨틱과 타입 프로모션이 결합된 복잡한 분기 |

### @MX:NOTE 후보 (설계 근거 문서화)

| 위치 | 근거 |
|------|------|
| `Catalog` / `SymbolTable` 설계 | "SymbolTable"과 "Catalog"의 명명 구분 근거 |
| Volcano 모델 선택 | 단일 스레드, WASM 호환, 메모리 효율적 — 선택 근거 문서화 |
| `PropertyValue` <-> `Value` 변환 | 두 표현 간 변환 규칙과 대칭성 근거 |

---

## 7. Architecture Design Direction

### 7.1 Volcano Iterator Model

단일 스레드 풀 기반(Volcano) 모델을 채택한다:
- `std::thread` 요구사항 없음 (WASM 호환성)
- `StorageEngine`이 `!Send`이므로 스레드 간 이동 불가
- 스트리밍에 메모리 효율적
- 연산자 간 단순한 합성

### 7.2 Typed Value Comparison

`PropertyValue`에 `PartialOrd`가 없으므로 표현식 평가기가 타입별 비교를 구현한다:

- Integer op Integer: 수치 비교
- Float op Float: 부동소수점 비교
- Integer op Float: Integer를 Float로 프로모션 후 비교
- String op String: 사전순 비교 (Eq/Ne/Lt/Lte/Gt/Gte)
- 타입 불일치: `ExecutionError("type mismatch in comparison")`
- Null op anything: 항상 false (Cypher null 시맨틱)

이 로직은 `executor/eval.rs`에 단일 권위적 비교 구현으로 존재한다.

### 7.3 Two-Representation Value System

스토리지 레이어의 `PropertyValue`와 쿼리 실행기의 `Value` 두 가지 표현이 존재한다:

- `Value`는 `PropertyValue`에 `Node(NodeId)`, `Edge(EdgeId)`, `List(Vec<Value>)` 변형을 추가
- `From<PropertyValue> for Value` 및 `TryFrom<Value> for PropertyValue` 변환 구현
- 프로퍼티 쓰기 경로(SET)는 `TryFrom` 사용
- 7개 타입 변형(Array 포함) 모두에 대한 테스트 커버

---

## 8. Phase 1 Crate Modifications (Phase 2 범위)

### cypherlite-core 변경 사항

1. `traits.rs`에 `LabelRegistry` trait 추가
2. `error.rs`에 `ParseError`, `SemanticError`, `ExecutionError`, `UnsupportedSyntax` 변형 추가

### cypherlite-storage 변경 사항

1. `catalog/` 모듈 추가 (Catalog struct, BiMap, save/load)
2. `StorageEngine`에 Catalog 필드 및 `impl LabelRegistry` 추가
3. `scan_nodes()`, `scan_nodes_by_label()`, `scan_edges_by_type()` 공개 메서드 추가
4. `StorageEngine::open()`에서 카탈로그 페이지(page 2) 로드 로직 추가
