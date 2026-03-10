---
id: SPEC-DB-003
version: "1.0.0"
status: draft
created: "2026-03-10"
updated: "2026-03-10"
author: epsilondelta
priority: P1
tags: [advanced-query, with, merge, optional-match, unwind, variable-length-paths, indexing, optimization]
lifecycle: spec-anchored
---

# SPEC-DB-003: CypherLite Phase 3 - Advanced Query Features (v0.3)

> CypherLite의 고급 쿼리 기능 레이어. Phase 2 쿼리 엔진 위에 WITH, UNWIND, OPTIONAL MATCH, MERGE 절 실행, 가변 길이 경로 순회, 프로퍼티 인덱스 시스템, 쿼리 최적화 규칙을 구축하여 openCypher v1.0 서브셋의 실행 범위를 완성한다.

---

## HISTORY

| Version | Date | Changes |
|---------|------|---------|
| 1.0.0 | 2026-03-10 | Initial SPEC creation based on research, plan, acceptance criteria drafts |

---

## 1. Environment (환경)

### 1.1 시스템 환경

- **언어**: Rust 1.84+ (Edition 2021)
- **MSRV**: 1.84 (Phase 1/2와 동일)
- **대상 플랫폼**: Linux (x86_64), macOS (x86_64, aarch64), Windows (x86_64)
- **실행 모델**: 동기, 단일 스레드, WASM 호환 (`std::thread` 사용 금지)
- **WASM 빌드 타겟**: `wasm32-unknown-unknown`

### 1.2 크레이트 구조

- **확장 크레이트**: `cypherlite-query` (`crates/cypherlite-query/`) — Phase 2에서 생성된 크레이트 확장
- **확장 크레이트**: `cypherlite-storage` (`crates/cypherlite-storage/`) — 인덱스 모듈 추가
- **의존 크레이트**: `cypherlite-core` — 에러 타입 확장
- **신규 외부 의존성**: 없음 (Phase 2 의존성 유지)

### 1.3 의존성 그래프 (Phase 3 추가분)

```
cypherlite-query
  +-- cypherlite-core    (PropertyValue, error types)
  +-- cypherlite-storage (StorageEngine, Catalog, scan APIs, IndexManager)
  +-- logos = "0.14"     (Phase 2에서 도입, 유지)

cypherlite-storage
  +-- cypherlite-core    (NodeId, EdgeId, PropertyValue, CypherLiteError)
  +-- 신규: index/ 모듈  (PropertyIndex, IndexManager — 내부 구현, 외부 의존성 없음)

cypherlite-core
  (no new dependencies)
```

### 1.4 Phase 3 크레이트 변경 범위

```
crates/
  cypherlite-storage/
    src/
      index/                    [NEW] 프로퍼티 인덱스 모듈
        mod.rs                  PropertyIndex trait, IndexManager struct
        btree_index.rs          B+Tree 기반 프로퍼티 인덱스 구현
      catalog/
        mod.rs                  + get_label_id(), get_type_id() 읽기 전용 조회
                                + 인덱스 정의 저장
      lib.rs                    + find_node(), find_edge(), scan_nodes_by_property(),
                                  scan_nodes_by_range() 메서드 추가

  cypherlite-query/
    src/
      lexer/
        mod.rs                  + UNWIND, INDEX, ON 키워드 토큰
      parser/
        ast.rs                  + UnwindClause, MergeClause ON MATCH/ON CREATE 확장
                                + RelationshipPattern.min_hops/max_hops
        clause.rs               + parse_unwind_clause(), parse_merge_clause() 확장
                                + CREATE INDEX / DROP INDEX DDL 파싱
        pattern.rs              + 가변 길이 경로 구문 (*N..M)
      semantic/
        mod.rs                  + WITH 스코프 리셋, UNWIND 변수 바인딩
                                + OPTIONAL MATCH null 변수 처리
                                + MERGE 패턴 검증
      planner/
        mod.rs                  + LogicalPlan::With, Unwind, MergeOp,
                                  OptionalExpand, VarLengthExpand, IndexScan
        optimize.rs             + 인덱스 스캔 선택, LIMIT pushdown,
                                  constant folding, projection pruning
      executor/
        operators/
          with.rs               [NEW] WithOp (projection + scope barrier)
          unwind.rs             [NEW] UnwindOp (리스트 확장)
          optional_expand.rs    [NEW] OptionalExpandOp (left join)
          merge.rs              [NEW] MergeOp (match-or-create)
          var_length_expand.rs  [NEW] VarLengthExpandOp (BFS 순회)
          index_scan.rs         [NEW] IndexScanOp (프로퍼티 인덱스 조회)
      api/
        mod.rs                  + CREATE INDEX / DROP INDEX DDL 실행 지원
```

---

## 2. Assumptions (가정)

### 2.1 Phase 2 완료 가정

- **A-010**: Phase 2의 ORDER BY, SKIP, LIMIT는 RETURN 절 내에서 이미 완전히 구현되어 있다. Phase 3에서 추가 작업이 필요하지 않다.
- **A-011**: Phase 2에서 WITH, MERGE, OPTIONAL MATCH에 대한 파서 지원(AST 노드)이 이미 존재한다. 그러나 시맨틱 분석, 플래너, 실행기 파이프라인은 구현되어 있지 않다.
- **A-012**: UNWIND와 가변 길이 경로는 Phase 2에서 파싱되지 않으며, 렉서부터 실행기까지 모든 레이어에서 변경이 필요하다.

### 2.2 스토리지 엔진 가정

- **A-013**: Phase 1의 B+Tree 구현을 프로퍼티 인덱스의 백엔드 저장소로 재사용한다. 외부 인덱스 라이브러리는 불필요하다.
- **A-014**: StorageEngine은 단일 writer 모델을 사용한다 (쓰기 트랜잭션 직렬화). MERGE의 match-then-create 원자성은 단일 쓰기 트랜잭션 내에서 보장된다.
- **A-015**: 인덱스 업데이트는 데이터 변경과 동일한 쓰기 트랜잭션 내에서 수행된다. WAL을 통한 크래시 일관성이 보장된다.
- **A-016**: `find_node()`, `find_edge()` API가 Phase 2에는 존재하지 않으며 Phase 3에서 추가해야 한다.

### 2.3 쿼리 엔진 가정

- **A-017**: Phase 3에서도 비용 기반 옵티마이저는 도입하지 않는다. 규칙 기반 플래너를 인덱스 인식 규칙으로 확장한다.
- **A-018**: 복합 인덱스(composite index)는 Phase 3 범위 밖이다. 단일 프로퍼티 인덱스만 지원한다.
- **A-019**: Phase 2에서 P2 절 실행 시 반환하던 `UnsupportedSyntax` 에러를 Phase 3에서 제거하고 실제 실행 로직으로 대체한다.

---

## 3. Requirements (요구사항)

### 모듈 1: WITH Clause (REQ-QE3-001 ~ REQ-QE3-007)

#### REQ-QE3-001 [Ubiquitous]
WITH 절은 **항상** 파이프라인 배리어로 동작하며, 지정된 컬럼만 투영(projection)하고 변수 스코프를 리셋한다. WITH 이후에는 명시적으로 나열된 변수만 접근 가능하다.

#### REQ-QE3-002 [Event-Driven]
**WHEN** WITH 절 이후에 WITH에서 투영되지 않은 변수를 참조하면 **THEN** 시맨틱 분석기는 `SemanticError`를 반환하며, 해당 변수가 WITH 스코프에 포함되지 않았음을 명시한다.

#### REQ-QE3-003 [State-Driven]
**IF** WITH 절에 WHERE 조건이 포함되면 **THEN** 실행기는 투영(projection) 이후에 필터를 적용한다. 이는 MATCH WHERE와 다른 평가 순서이다.

#### REQ-QE3-004 [State-Driven]
**IF** WITH 절에 집계 함수(count, sum 등)가 포함되면 **THEN** 실행기는 AggregateOp과 결합하여 그룹별 집계를 수행한 후 스코프 배리어를 적용한다.

#### REQ-QE3-005 [State-Driven]
**IF** WITH DISTINCT가 사용되면 **THEN** 실행기는 다음 절로 전달하기 전에 중복을 제거한다.

#### REQ-QE3-006 [State-Driven]
**IF** WITH 절에 ORDER BY, SKIP, LIMIT가 포함되면 **THEN** Phase 2에서 구현된 정렬/페이지네이션 로직을 WITH 스코프 내에서 재사용한다.

#### REQ-QE3-007 [Complex]
**IF** 여러 WITH 절이 연쇄(chain)되면 **AND WHEN** 각 WITH 절이 실행되면 **THEN** 변수 스코프는 점진적으로 축소되며, 각 WITH는 이전 WITH의 출력만을 입력으로 받는다.

---

### 모듈 2: UNWIND Clause (REQ-QE3-008 ~ REQ-QE3-014)

#### REQ-QE3-008 [Ubiquitous]
UNWIND 절은 **항상** 리스트 표현식을 개별 행으로 확장한다. 각 소스 레코드에 대해 리스트의 각 요소가 별도의 행으로 생성된다.

#### REQ-QE3-009 [Event-Driven]
**WHEN** UNWIND의 표현식이 빈 리스트(`[]`)로 평가되면 **THEN** 해당 소스 레코드에 대해 0개의 행이 생성된다 (에러 없음).

#### REQ-QE3-010 [Event-Driven]
**WHEN** UNWIND의 표현식이 NULL로 평가되면 **THEN** 해당 소스 레코드에 대해 0개의 행이 생성된다 (Neo4j 호환 동작).

#### REQ-QE3-011 [Event-Driven]
**WHEN** UNWIND의 표현식이 리스트가 아닌 값(정수, 문자열 등)으로 평가되면 **THEN** 실행기는 실제 타입을 명시하는 `ExecutionError`를 반환한다.

#### REQ-QE3-012 [Ubiquitous]
파서는 **항상** `UNWIND expr AS variable` 구문을 지원하며, 렉서에 `UNWIND` 키워드 토큰을 추가한다.

#### REQ-QE3-013 [Ubiquitous]
시맨틱 분석기는 **항상** UNWIND의 AS 변수를 현재 스코프에 등록한다.

#### REQ-QE3-014 [State-Driven]
**IF** UNWIND의 리스트가 중첩 리스트를 포함하면 **THEN** 각 내부 리스트는 단일 요소로 취급된다 (1단계 확장만 수행).

---

### 모듈 3: OPTIONAL MATCH (REQ-QE3-015 ~ REQ-QE3-021)

#### REQ-QE3-015 [Ubiquitous]
OPTIONAL MATCH는 **항상** left outer join 의미론으로 동작한다. 매칭되는 패턴이 없으면 해당 변수를 NULL로 바인딩하고 소스 레코드를 보존한다.

#### REQ-QE3-016 [Event-Driven]
**WHEN** OPTIONAL MATCH의 내부 패턴이 소스 레코드에 대해 0개의 결과를 반환하면 **THEN** 실행기는 소스 레코드를 유지하고 OPTIONAL MATCH에서 바인딩할 모든 변수를 NULL로 설정한 레코드 1개를 생성한다.

#### REQ-QE3-017 [Event-Driven]
**WHEN** OPTIONAL MATCH의 내부 패턴이 N개의 결과를 반환하면 **THEN** 일반 MATCH와 동일하게 N개의 레코드를 생성한다.

#### REQ-QE3-018 [Ubiquitous]
실행기는 **항상** OPTIONAL MATCH에서 발생한 NULL 값의 전파를 올바르게 처리한다. NULL에 대한 프로퍼티 접근, 비교 연산, 집계 함수 모두 삼값 논리(three-valued logic)를 따른다.

#### REQ-QE3-019 [State-Driven]
**IF** OPTIONAL MATCH에 WHERE 조건이 포함되면 **THEN** 필터는 OPTIONAL 부분에만 적용된다. 필터를 통과하지 못해도 소스 레코드는 NULL 패딩으로 보존된다.

#### REQ-QE3-020 [Complex]
**IF** 여러 OPTIONAL MATCH가 연쇄되면 **AND WHEN** 각 OPTIONAL MATCH가 실행되면 **THEN** 각 레벨이 독립적으로 NULL이 될 수 있다.

#### REQ-QE3-021 [Ubiquitous]
OPTIONAL MATCH에서 NULL 바인딩된 변수에 대한 `count()` 집계는 **항상** NULL 행을 제외한다 (`count(b)`는 NULL인 b를 세지 않는다).

---

### 모듈 4: MERGE Clause (REQ-QE3-022 ~ REQ-QE3-031)

#### REQ-QE3-022 [Ubiquitous]
MERGE 절은 **항상** match-or-create 원자 연산으로 동작한다. 패턴과 일치하는 엔티티가 있으면 매칭하고, 없으면 생성한다.

#### REQ-QE3-023 [Event-Driven]
**WHEN** MERGE 패턴과 일치하는 노드가 존재하면 **THEN** 실행기는 기존 노드를 매칭하고 새 노드를 생성하지 않는다.

#### REQ-QE3-024 [Event-Driven]
**WHEN** MERGE 패턴과 일치하는 노드가 존재하지 않으면 **THEN** 실행기는 패턴에 지정된 레이블과 프로퍼티를 가진 새 노드를 생성한다.

#### REQ-QE3-025 [Ubiquitous]
MERGE는 **항상** 멱등(idempotent)이어야 한다. 동일한 MERGE를 여러 번 실행해도 결과가 변하지 않는다.

#### REQ-QE3-026 [State-Driven]
**IF** MERGE에 `ON CREATE SET` 절이 포함되면 **THEN** 지정된 프로퍼티 설정은 새 노드가 생성된 경우에만 적용된다.

#### REQ-QE3-027 [State-Driven]
**IF** MERGE에 `ON MATCH SET` 절이 포함되면 **THEN** 지정된 프로퍼티 설정은 기존 노드가 매칭된 경우에만 적용된다.

#### REQ-QE3-028 [Complex]
**IF** MERGE에 `ON MATCH SET`과 `ON CREATE SET`이 모두 포함되면 **AND WHEN** 매칭 또는 생성이 결정되면 **THEN** 해당하는 액션만 실행된다 (두 액션이 동시에 적용되지 않는다).

#### REQ-QE3-029 [Ubiquitous]
MERGE 관계 패턴은 **항상** 시작 노드, 끝 노드, 관계 타입을 기준으로 기존 관계를 검색한다. 일치하지 않으면 새 관계를 생성한다.

#### REQ-QE3-030 [Ubiquitous]
MERGE의 match-then-create 시퀀스는 **항상** 단일 쓰기 트랜잭션 내에서 원자적으로 실행된다.

#### REQ-QE3-031 [Ubiquitous]
파서는 **항상** `MergeClause` AST에 `on_match: Vec<SetItem>`과 `on_create: Vec<SetItem>` 필드를 포함하도록 확장한다.

---

### 모듈 5: Variable-Length Paths (REQ-QE3-032 ~ REQ-QE3-041)

#### REQ-QE3-032 [Ubiquitous]
파서는 **항상** 관계 패턴에서 가변 길이 경로 구문을 지원한다: `[*]`, `[*N]`, `[*N..M]`, `[:TYPE*N..M]`.

#### REQ-QE3-033 [Ubiquitous]
AST의 `RelationshipPattern`은 **항상** `min_hops: Option<u32>`와 `max_hops: Option<u32>` 필드를 포함한다.

#### REQ-QE3-034 [Ubiquitous]
실행기는 **항상** BFS(Breadth-First Search) 기반 순회를 사용하여 가변 길이 경로를 탐색한다. BFS는 최단 경로 우선 순서를 보장한다.

#### REQ-QE3-035 [Ubiquitous]
실행기는 **항상** 방문한 엣지를 추적(`HashSet<EdgeId>`)하여 순환 그래프에서 무한 루프를 방지한다.

#### REQ-QE3-036 [State-Driven]
**IF** 가변 길이 경로가 범위 지정(`[*N..M]`)이면 **THEN** 실행기는 min_hops 이상 max_hops 이하의 깊이에 있는 노드만 결과에 포함한다.

#### REQ-QE3-037 [State-Driven]
**IF** 가변 길이 경로가 비제한(`[*]`)이면 **THEN** 플래너는 설정 가능한 기본 최대 홉 수(기본값: 10)를 적용하여 무제한 순회를 방지한다.

#### REQ-QE3-038 [State-Driven]
**IF** 정확한 홉 수가 지정되면(`[*N]`, min_hops == max_hops) **THEN** 실행기는 정확히 N 홉 거리의 노드만 결과에 포함한다.

#### REQ-QE3-039 [State-Driven]
**IF** 가변 길이 경로에 관계 타입이 지정되면(`[:KNOWS*1..3]`) **THEN** 순회는 지정된 타입의 엣지만 따른다.

#### REQ-QE3-040 [Event-Driven]
**WHEN** 가변 길이 경로 순회가 매칭되는 경로를 찾지 못하면 **THEN** 에러 없이 빈 결과를 반환한다.

#### REQ-QE3-041 [Unwanted]
시스템은 `[*1..0]`과 같이 min_hops > max_hops인 무효한 범위를 **허용하지 않아야 한다**. 파서 또는 시맨틱 분석기에서 에러를 반환한다.

---

### 모듈 6: Property Index System (REQ-QE3-042 ~ REQ-QE3-053)

#### REQ-QE3-042 [Ubiquitous]
인덱스 시스템은 **항상** 단일 프로퍼티 B+Tree 인덱스를 지원한다. 키는 `(label_id, prop_key_id, PropertyValue)`이며 값은 `Vec<NodeId>`이다.

#### REQ-QE3-043 [Ubiquitous]
인덱스는 **항상** (label, property) 쌍별로 독립적인 B+Tree로 저장된다.

#### REQ-QE3-044 [Event-Driven]
**WHEN** `CREATE INDEX [name] ON :Label(property)` DDL이 실행되면 **THEN** 인덱스가 생성되고 정의가 Catalog에 저장된다.

#### REQ-QE3-045 [Event-Driven]
**WHEN** `DROP INDEX name` DDL이 실행되면 **THEN** 인덱스가 제거되고 Catalog에서 정의가 삭제된다. 후속 쿼리는 전체 레이블 스캔으로 폴백한다.

#### REQ-QE3-046 [Ubiquitous]
인덱스는 **항상** CREATE 노드, SET 프로퍼티, DELETE 노드 연산 시 자동으로 업데이트된다. 인덱스 업데이트는 데이터 변경과 동일한 쓰기 트랜잭션 내에서 수행된다.

#### REQ-QE3-047 [Ubiquitous]
`StorageEngine`은 **항상** `scan_nodes_by_property(label_id, prop_key, value) -> Vec<NodeId>` API를 제공하여 인덱스 기반 동등(equality) 조회를 지원한다.

#### REQ-QE3-048 [Ubiquitous]
`StorageEngine`은 **항상** `scan_nodes_by_range(label_id, prop_key, min, max) -> Vec<NodeId>` API를 제공하여 인덱스 기반 범위(range) 조회를 지원한다.

#### REQ-QE3-049 [Ubiquitous]
렉서는 **항상** `INDEX`, `ON` 키워드 토큰을 지원하여 DDL 파싱을 가능하게 한다.

#### REQ-QE3-050 [Ubiquitous]
Catalog는 **항상** `get_label_id(name) -> Option<u32>`와 `get_type_id(name) -> Option<u32>` 읽기 전용 조회 메서드를 제공한다 (자동 생성 없이 조회만 수행).

#### REQ-QE3-051 [Ubiquitous]
`StorageEngine`은 **항상** `find_node(label_ids, properties) -> Option<NodeId>` API를 제공하여 레이블과 프로퍼티 기반 노드 검색을 지원한다. 이는 MERGE의 매칭 단계에 필수적이다.

#### REQ-QE3-052 [Ubiquitous]
`StorageEngine`은 **항상** `find_edge(start, end, type_id) -> Option<EdgeId>` API를 제공하여 엔드포인트와 타입 기반 엣지 검색을 지원한다. 이는 MERGE 관계의 매칭 단계에 필수적이다.

#### REQ-QE3-053 [Optional]
**가능하면** 인덱스 정의는 Catalog 영속화를 통해 checkpoint/recovery 이후에도 보존된다.

---

### 모듈 7: Query Optimization Rules (REQ-QE3-054 ~ REQ-QE3-060)

#### REQ-QE3-054 [State-Driven]
**IF** 쿼리의 WHERE 조건에 프로퍼티 동등 비교가 포함되고 **AND** 해당 (label, property)에 인덱스가 존재하면 **THEN** 플래너는 NodeScan+Filter 대신 IndexScan을 선택한다.

#### REQ-QE3-055 [State-Driven]
**IF** 쿼리에 LIMIT 절이 포함되면 **THEN** 플래너는 LIMIT를 NodeScan/Expand 연산자로 푸시다운하여 전체 결과 수집(materialization)을 방지한다.

#### REQ-QE3-056 [Ubiquitous]
플래너는 **항상** 상수 표현식을 계획 시점에 평가한다 (constant folding). 예: `WHERE n.age > 10 + 20`은 `WHERE n.age > 30`으로 변환된다.

#### REQ-QE3-057 [Ubiquitous]
플래너는 **항상** 후속 절에서 사용되지 않는 컬럼을 파이프라인 초기에 제거한다 (projection pruning).

#### REQ-QE3-058 [State-Driven]
**IF** MERGE 실행 시 매칭 대상 (label, property)에 인덱스가 존재하면 **THEN** 전체 레이블 스캔 대신 인덱스 조회를 사용하여 매칭을 수행한다 (MERGE short-circuit).

#### REQ-QE3-059 [Ubiquitous]
플래너는 **항상** `LogicalPlan::IndexScan { label_id, prop_key, value }` 논리 계획 변형을 지원한다.

#### REQ-QE3-060 [Ubiquitous]
인덱스 스캔 선택, LIMIT pushdown, constant folding, projection pruning 규칙은 **항상** `planner/optimize.rs`에 독립적인 최적화 패스로 구현한다.

---

### 모듈 8: API Layer Updates (REQ-QE3-061 ~ REQ-QE3-064)

#### REQ-QE3-061 [Ubiquitous]
`CypherLite::execute()` API는 **항상** Phase 3의 모든 새 절(WITH, UNWIND, OPTIONAL MATCH, MERGE)과 DDL(CREATE INDEX, DROP INDEX)을 지원한다.

#### REQ-QE3-062 [Event-Driven]
**WHEN** Phase 2에서 `UnsupportedSyntax`를 반환하던 P2 절이 실행되면 **THEN** Phase 3에서는 실제 실행 로직이 동작한다.

#### REQ-QE3-063 [Ubiquitous]
실행기의 `dispatch` 로직은 **항상** 6개의 새 연산자 타입(WithOp, UnwindOp, OptionalExpandOp, MergeOp, VarLengthExpandOp, IndexScanOp)을 라우팅한다.

#### REQ-QE3-064 [Ubiquitous]
버전을 **항상** 0.3.0으로 업데이트하고, 공개 API 문서를 새 기능에 맞게 갱신한다.

---

## 4. Non-Functional Requirements (비기능 요구사항)

### 4.1 성능 요구사항

| Metric | Target | 측정 방법 |
|--------|--------|-----------|
| 인덱스 동등 조회 (p99) | < 1ms | Criterion benchmark, 100K 노드, 1K iterations |
| 인덱스 범위 스캔 (p99) | < 5ms | Criterion benchmark, 100K 노드, 데이터 ~1% 반환 |
| 가변 길이 경로 3-hop (p99) | < 20ms | Criterion benchmark, 1K 노드, 5K 엣지, 100 iterations |
| MERGE (p99) | < 5ms | Criterion benchmark, 기존 노드 매칭 시나리오 |
| WITH pipeline (p99) | < 15ms | Criterion benchmark, MATCH + WITH + RETURN, 10K 노드 |
| OPTIONAL MATCH (p99) | < 20ms | Criterion benchmark, 1K 노드, 부분 매칭 |

### 4.2 품질 게이트

| Gate | Criteria | Command |
|------|----------|---------|
| 모든 테스트 통과 | 100% pass | `cargo test --workspace` |
| Clippy 경고 제로 | zero warnings | `cargo clippy -- -D warnings` |
| 코드 커버리지 | 85% 이상 | `cargo tarpaulin` (Linux) 또는 동등 도구 |
| 포맷 검사 | pass | `cargo fmt --check` |
| 바이너리 크기 (release) | < 50MB | `cargo build --release` |

### 4.3 TRUST 5 검증

| Pillar | 검증 항목 |
|--------|-----------|
| **Tested** | 85%+ 커버리지; 모든 AC에 대응하는 테스트; proptest (가변 길이 경로, OPTIONAL MATCH) |
| **Readable** | 명확한 네이밍; 영문 코드 주석; 새 연산자와 인덱스 모듈에 MX 태그 |
| **Unified** | `cargo fmt` 통과; `cargo clippy` 경고 제로 |
| **Secured** | UNWIND/가변 길이 경로 제한 입력 검증; 트랜잭션 경계 내 인덱스 연산 |
| **Trackable** | Conventional commits; SPEC-DB-003 참조; changelog 항목 |

---

## 5. openCypher Subset Scope Update

### Phase 3 실행 범위 확장

| Priority | Clause / Feature | Phase 2 상태 | Phase 3 상태 | 설명 |
|----------|-----------------|-------------|-------------|------|
| **P2→실행** | `WITH` | 파싱만 | 파싱 + 실행 | 스코프 배리어, 투영, 필터링, 집계, DISTINCT |
| **P2→실행** | `MERGE` | 파싱만 | 파싱 + 실행 | match-or-create 원자 연산, ON MATCH/ON CREATE SET |
| **P2→실행** | `OPTIONAL MATCH` | 파싱만 | 파싱 + 실행 | Left join 의미론, NULL 전파 |
| **신규** | `UNWIND` | 미구현 | 파싱 + 실행 | 리스트 확장 |
| **신규** | Variable-length paths `*N..M` | 미구현 | 파싱 + 실행 | BFS 순회, 사이클 감지 |
| **신규** | Property indexes | 미구현 | 구현 | B+Tree 인덱스, CREATE/DROP INDEX DDL |
| **신규** | Query optimization | 부분 구현 | 확장 | 인덱스 스캔, LIMIT pushdown, constant folding, projection pruning |
| **완료** | `ORDER BY` / `SKIP` / `LIMIT` | 파싱 + 실행 | 변경 없음 | Phase 2에서 완전히 구현됨 |

### v2.0 이후 연기 유지 기능

| Feature | 연기 사유 |
|---------|-----------|
| `CALL` subqueries | 서브쿼리 실행 컨텍스트 필요 |
| `UNION` / `UNION ALL` | 브랜치 간 스키마 호환성 검증 필요 |
| `FOREACH` | 표현식 내 mutation; MVCC와의 복잡한 상호작용 |
| `CASE WHEN` | WHERE + OPTIONAL MATCH로 에뮬레이션 가능 |
| Composite indexes | 단일 프로퍼티 인덱스로 Phase 3 충분 |
| Join order optimization | 규칙 기반 플래너로 Phase 3 충분 |

---

## 6. Architecture Overview (아키텍처 개요)

### 6.1 Phase 3 쿼리 파이프라인 확장

```
Cypher 문자열
   |
   v
[Lexer] -- + UNWIND, INDEX, ON 키워드 토큰
   |
   v
Token Stream
   |
   v
[Parser] -- + parse_unwind_clause(), 가변 길이 경로 구문, DDL 파싱
   |
   v
AST -- + UnwindClause, MergeClause 확장, RelationshipPattern.hops
   |
   v
[Semantic Analysis] -- + WITH 스코프 리셋, UNWIND 변수 바인딩,
   |                      OPTIONAL MATCH null 처리, MERGE 패턴 검증
   v
Annotated AST
   |
   v
[Logical Planner] -- + With, Unwind, MergeOp, OptionalExpand,
   |                    VarLengthExpand, IndexScan 논리 계획 변형
   v
[Optimizer] -- + 인덱스 스캔 선택, LIMIT pushdown,
   |             constant folding, projection pruning
   v
Logical Plan (optimized)
   |
   v
[Physical Planner] -- Volcano Iterator 연산자 트리로 변환
   |
   v
Physical Plan (operator tree) -- + 6개 신규 연산자
   |
   v
[Executor] -- + WithOp, UnwindOp, OptionalExpandOp,
   |            MergeOp, VarLengthExpandOp, IndexScanOp
   v
QueryResult (rows)
```

### 6.2 프로퍼티 인덱스 아키텍처

```
IndexManager
  +-- PropertyIndex(Person, name) -- B+Tree: PropertyValue -> Vec<NodeId>
  +-- PropertyIndex(Person, age)  -- B+Tree: PropertyValue -> Vec<NodeId>
  +-- ...

StorageEngine
  +-- IndexManager (인덱스 관리)
  +-- Catalog (인덱스 정의 저장)
  +-- WAL (인덱스 + 데이터 원자적 기록)
```

### 6.3 MERGE 실행 흐름

```
MERGE (n:Person {name: "Alice"})
  |
  v
[1. Match Phase] -- find_node([:Person], {name: "Alice"})
  |                  (인덱스 존재 시 인덱스 조회)
  |
  +-- 매칭됨 --> [3a. ON MATCH SET 적용] --> 변수 바인딩
  |
  +-- 미매칭 --> [2. Create Phase] -- CREATE (:Person {name: "Alice"})
                  |
                  v
                [3b. ON CREATE SET 적용] --> 변수 바인딩
```

### 6.4 가변 길이 경로 순회 (BFS)

```
(A)-[:KNOWS*1..3]->(x)
  |
  v
[BFS Traversal]
  Depth 0: {A} (시작 노드, 결과에 미포함 -- min_hops=1)
  Depth 1: {B} -- A→B (결과에 포함)
  Depth 2: {C} -- B→C (결과에 포함)
  Depth 3: {D} -- C→D (결과에 포함)
  Depth 4: 중단 (max_hops=3 초과)

  사이클 감지: visited_edges HashSet으로 동일 엣지 재방문 방지
```

---

## 7. Error Types (에러 타입 확장)

`CypherLiteError`에 추가될 Phase 3 변형:

| 변형 | 설명 |
|------|------|
| `IndexError(String)` | 인덱스 생성/삭제/조회 오류 |
| `MergeConflict(String)` | MERGE 패턴 충돌 (선택적, 진단용) |

기존 Phase 2 에러 타입 재사용:

| 변형 | Phase 3 사용 |
|------|-------------|
| `ExecutionError(String)` | UNWIND 타입 불일치, 가변 길이 경로 제한 초과 |
| `SemanticError(String)` | WITH 스코프 위반, 무효한 경로 범위 |
| `UnsupportedSyntax(String)` | Phase 3 완료 후 제거 대상 (P2 절 모두 실행 가능) |

---

## 8. Traceability (추적성)

| 요구사항 | 구현 대상 | 테스트 대상 |
|----------|-----------|-------------|
| REQ-QE3-001 ~ 007 (WITH) | `cypherlite-query::executor::operators::with` | AC-020 ~ AC-023, WITH-T001 ~ T007 |
| REQ-QE3-008 ~ 014 (UNWIND) | `cypherlite-query::executor::operators::unwind`, `parser::clause`, `lexer` | AC-024 ~ AC-028, UNWIND-T001 ~ T007 |
| REQ-QE3-015 ~ 021 (OPTIONAL MATCH) | `cypherlite-query::executor::operators::optional_expand` | AC-029 ~ AC-032, OPT-T001 ~ T007 |
| REQ-QE3-022 ~ 031 (MERGE) | `cypherlite-query::executor::operators::merge`, `parser::clause` | AC-033 ~ AC-039, MERGE-T001 ~ T010 |
| REQ-QE3-032 ~ 041 (Variable-Length Paths) | `cypherlite-query::executor::operators::var_length_expand`, `parser::pattern` | AC-044 ~ AC-048, VLP-T001 ~ T010 |
| REQ-QE3-042 ~ 053 (Indexing) | `cypherlite-storage::index`, `catalog`, `lib` | AC-040 ~ AC-043, IDX-T001 ~ T010 |
| REQ-QE3-054 ~ 060 (Optimization) | `cypherlite-query::planner::optimize` | AC-049 ~ AC-051, OPT-T001 ~ T005 |
| REQ-QE3-061 ~ 064 (API) | `cypherlite-query::api`, `executor::mod` | INT-T010 ~ T016 |
