---
id: SPEC-DB-002
type: acceptance
version: "1.0.0"
status: draft
created: "2026-03-10"
updated: "2026-03-10"
tags: [query-engine, cypher, parser, executor, openCypher]
---

# SPEC-DB-002 Acceptance Criteria: CypherLite Query Engine

---

## 1. Functional Acceptance Criteria

### AC-001: Basic MATCH RETURN (기본 노드 조회)

```gherkin
Given: 3개의 Person 노드가 존재하며 name 프로퍼티가 각각 "Alice", "Bob", "Carol"인 데이터베이스
When: MATCH (n:Person) RETURN n.name 이 실행되면
Then: 결과는 정확히 3개의 행을 포함한다
  And: 각 행은 "n.name" 컬럼에 "Alice", "Bob", "Carol" 값을 가진다 (순서 무관)
  And: 쿼리는 10ms 이내에 완료된다 (p99)
```

### AC-002: CREATE Node and Verify Retrieval (노드 생성 및 검증)

```gherkin
Given: 빈 데이터베이스
When: CREATE (a:Person {name: "Alice"}) 이 실행되면
Then: 트랜잭션이 성공적으로 커밋된다
  And: 후속 MATCH (n:Person) RETURN count(n) 은 1을 반환한다
  And: 후속 MATCH (n:Person {name: "Alice"}) RETURN n.name 은 "Alice"를 반환한다
```

### AC-003: CREATE Relationship and Traverse (관계 생성 및 순회)

```gherkin
Given: 빈 데이터베이스
When: CREATE (a:Person {name: "Alice"})-[:KNOWS]->(b:Person {name: "Bob"}) 이 실행되면
Then: 트랜잭션이 성공적으로 커밋된다
  And: 후속 MATCH (n:Person) RETURN count(n) 은 2를 반환한다
  And: 후속 MATCH (a)-[:KNOWS]->(b) RETURN b.name 은 "Bob"을 반환한다
```

### AC-004: WHERE Equality Filter (WHERE 동등 필터)

```gherkin
Given: age 프로퍼티가 20, 25, 30, 35, 40인 5개의 Person 노드가 있는 데이터베이스
When: MATCH (n:Person) WHERE n.age > 28 RETURN n.age 이 실행되면
Then: 결과는 정확히 3개의 행을 포함한다 (값: 30, 35, 40)
  And: age <= 28인 행은 제외된다
```

### AC-005: Two-Hop Pattern Traversal (2홉 패턴 순회)

```gherkin
Given: (Alice)-[:KNOWS]->(Bob)-[:KNOWS]->(Carol) 그래프
When: MATCH (a:Person {name: "Alice"})-[:KNOWS]->(b)-[:KNOWS]->(c:Person) RETURN c.name 이 실행되면
Then: 결과는 정확히 1개의 행을 포함하며 값은 "Carol"이다
  And: "Bob"은 결과에 포함되지 않는다 (1홉이므로)
  And: 쿼리는 50ms 이내에 완료된다 (p99)
```

### AC-006: Syntax Error Detection (구문 에러 감지)

```gherkin
Given: 잘못된 쿼리 문자열 "MATCH n RETURN n" (괄호 누락)
When: 쿼리가 파싱되면
Then: ParseError가 반환된다 (패닉이 아님)
  And: 에러 메시지는 구문 에러의 line 번호와 column을 포함한다
  And: 에러 메시지는 문제가 된 토큰과 예상 구문을 식별한다
```

### AC-007: Type Mismatch Error (타입 불일치 에러)

```gherkin
Given: age = 30 (정수) 프로퍼티를 가진 Person 노드가 있는 데이터베이스
When: MATCH (n:Person) WHERE n.age + "foo" > 0 RETURN n 이 실행되면
Then: 행이 생성되기 전에 TypeError가 반환된다
  And: 에러 메시지는 호환되지 않는 타입을 식별한다
```

### AC-008: Transaction Isolation (트랜잭션 격리)

```gherkin
Given: 트랜잭션 T1이 Person 노드에 대한 읽기를 시작함
  And: 트랜잭션 T2가 새 Person 노드를 생성하고 커밋함
When: T1이 동일 트랜잭션 내에서 Person 노드를 다시 읽으면
Then: T1은 T2가 생성한 노드를 보지 못한다 (스냅샷 격리)
  And: T2의 커밋 이후 시작된 새 트랜잭션 T3는 새 노드를 볼 수 있다
```

### AC-009: Simple MATCH Performance (단순 MATCH 성능)

```gherkin
Given: 10,000개의 Person 노드가 있는 데이터베이스
When: MATCH (n:Person {name: "Alice"}) RETURN n 이 1,000회 실행되면
Then: p99 레이턴시가 10ms 미만이다
  And: 쿼리 실행당 할당되는 메모리가 1MB를 초과하지 않는다
```

### AC-010: NULL Handling (NULL 처리)

```gherkin
Given: 일부는 "email" 프로퍼티가 있고 일부는 없는 Person 노드가 있는 데이터베이스
When: MATCH (n:Person) WHERE n.email IS NOT NULL RETURN n.name 이 실행되면
Then: email 프로퍼티가 non-null인 노드만 반환된다
When: MATCH (n:Person) RETURN n.email 이 실행되면
Then: email 프로퍼티가 없는 노드는 "n.email" 컬럼에 NULL을 반환한다
  And: NULL이 에러를 유발하지 않는다
```

### AC-011: MERGE Idempotency (MERGE 멱등성)

```gherkin
Given: Person 노드 {name: "Alice"}가 있는 데이터베이스
When: MERGE (n:Person {name: "Alice"}) 가 2회 실행되면
Then: 두 번 실행 후에도 데이터베이스에는 "Alice"라는 이름의 Person 노드가 정확히 1개만 존재한다
When: MERGE (n:Person {name: "Dave"}) 가 실행되면
Then: 새로운 Person 노드 {name: "Dave"}가 생성된다
```

**참고**: MERGE는 P2 절이므로 Phase 2에서는 파싱만 지원하고 실행 시 `UnsupportedSyntax`를 반환한다. 이 AC는 Phase 3에서 실행기가 구현될 때 활성화된다.

### AC-012: Result Streaming with LIMIT (LIMIT 결과 스트리밍)

```gherkin
Given: 10,000개의 노드가 있는 데이터베이스
When: MATCH (n) RETURN n LIMIT 10 이 실행되면
Then: 정확히 10개의 행이 반환된다
  And: 10개의 일치 행을 찾은 후 스토리지 스캔이 종료된다 (전체 스캔 없음)
  And: 메모리 사용량이 처음 10개 결과 행에 한정된다
```

**참고**: LIMIT는 P2 절이므로 Phase 2에서는 파싱만 지원한다. Phase 3에서 LimitOp 실행기가 구현될 때 이 AC가 활성화된다.

---

## 2. Performance Gates (성능 게이트)

| 지표 | 목표값 | 측정 방법 |
|------|--------|-----------|
| 단순 MATCH (p99) | < 10ms | Criterion 벤치마크, 10K 노드, 1K 반복 |
| 2홉 패턴 (p99) | < 50ms | Criterion 벤치마크, 1K 노드, 5K 엣지, 100 반복 |
| Parse 레이턴시 (p99) | < 1ms | 렉서 + 파서, 일반 쿼리 (50-200자) |
| Plan 레이턴시 (p99) | < 2ms | 3절 쿼리에 대한 플래너 |
| 쿼리당 메모리 | < 10MB | 힙 프로파일러, 최악 케이스 집계 |

---

## 3. Quality Gates (품질 게이트)

### 3.1 빌드 및 테스트

| 게이트 | 기준 | 명령어 |
|--------|------|--------|
| 전체 테스트 통과 | 100% pass | `cargo test --workspace` |
| Clippy 경고 없음 | zero warnings | `cargo clippy -- -D warnings` |
| 코드 커버리지 | 85% 이상 | `cargo tarpaulin` (Linux) 또는 동등 도구 |
| 포맷 검사 | 통과 | `cargo fmt --check` |

### 3.2 바이너리 크기

| 지표 | 기준 |
|------|------|
| 전체 바이너리 크기 (릴리즈) | < 50MB |

### 3.3 TRUST 5 검증

| Pillar | 검증 항목 |
|--------|-----------|
| **Tested** | 85%+ 커버리지, 모든 AC에 대한 테스트 존재, proptest로 파서 fuzzing |
| **Readable** | 명확한 네이밍, 영어 코드 주석, MX 태그 적절한 배치 |
| **Unified** | `cargo fmt` 통과, `cargo clippy` 경고 제로 |
| **Secured** | 입력 검증 (쿼리 파싱에서 injection 방지), `Params`를 통한 매개변수화된 쿼리 |
| **Trackable** | Conventional commits, SPEC-DB-002 참조, 변경 로그 |

---

## 4. Definition of Done (완료 정의)

SPEC-DB-002는 다음 조건이 모두 충족될 때 완료된다:

### 필수 조건

- [ ] AC-001 ~ AC-010 모든 기능 수락 기준 테스트 통과 (AC-011, AC-012는 P2 절이므로 Phase 3에서 활성화)
- [ ] `cargo test --workspace` 100% 통과
- [ ] `cargo clippy -- -D warnings` 경고 제로
- [ ] 코드 커버리지 85% 이상
- [ ] P0 절 (MATCH+RETURN, CREATE) 완전 실행 가능
- [ ] P1 절 (WHERE, SET, REMOVE, DELETE) 완전 실행 가능
- [ ] P2 절 (MERGE, WITH, ORDER BY, LIMIT, SKIP) 파싱 지원 (실행 시 UnsupportedSyntax)
- [ ] `CypherLite::open()`, `execute()`, `execute_with_params()` 공개 API 동작
- [ ] `Catalog` 영속화 (StorageEngine open/save 주기에서 카탈로그 페이지 유지)
- [ ] Scan APIs (`scan_nodes`, `scan_nodes_by_label`, `scan_edges_by_type`) 동작
- [ ] 성능 게이트 충족: parse p99 < 1ms, plan p99 < 2ms, simple MATCH p99 < 10ms

### 선택 조건

- [ ] proptest: 무작위 토큰 시퀀스에서 파서 패닉 없음
- [ ] Criterion 벤치마크 스위트 구축
- [ ] 2홉 패턴 p99 < 50ms 벤치마크 통과

---

## 5. Test Scenarios (상세 테스트 시나리오)

### 5.1 Lexer Tests

| Test ID | 시나리오 | 예상 결과 |
|---------|---------|-----------|
| LEX-T001 | 빈 입력 | 빈 토큰 스트림 |
| LEX-T002 | `MATCH (n) RETURN n` | [Match, LParen, Ident("n"), RParen, Return, Ident("n")] |
| LEX-T003 | 대소문자 혼합 `mAtCh` | Match 키워드로 인식 |
| LEX-T004 | 문자열 `'hello\nworld'` | 이스케이프 처리된 String 리터럴 |
| LEX-T005 | 인식 불가 문자 `@` | LexError with byte offset |
| LEX-T006 | 정수 `42`, 부동소수점 `3.14` | Integer(42), Float(3.14) |
| LEX-T007 | 유니코드 식별자 | 올바른 Ident 토큰 |

### 5.2 Parser Tests

| Test ID | 시나리오 | 예상 결과 |
|---------|---------|-----------|
| PARSE-T001 | `MATCH (n:Person) RETURN n` | MatchClause + ReturnClause AST |
| PARSE-T002 | `CREATE (n:Person {name: "Alice"})` | CreateClause with NodePattern |
| PARSE-T003 | `(a)-[:KNOWS]->(b)` | 방향 있는 RelationshipPattern |
| PARSE-T004 | `(a)<-[:KNOWS]-(b)` | 역방향 RelationshipPattern |
| PARSE-T005 | `WHERE n.age > 30 AND n.name = "Alice"` | BinaryOp(And, BinaryOp(Gt), BinaryOp(Eq)) |
| PARSE-T006 | 괄호 누락 `MATCH n` | ParseError with position info |
| PARSE-T007 | `RETURN count(DISTINCT n)` | FunctionCall with distinct=true |
| PARSE-T008 | `MATCH (n)-[*1..3]->(m)` | UnsupportedSyntax (가변 길이, P2) |

### 5.3 Executor Tests

| Test ID | 시나리오 | 예상 결과 |
|---------|---------|-----------|
| EXEC-T001 | NodeScan with label filter | 레이블 일치 노드만 반환 |
| EXEC-T002 | ExpandOp directed traversal | 방향에 따른 이웃 노드 반환 |
| EXEC-T003 | FilterOp with inequality | 조건 충족 행만 통과 |
| EXEC-T004 | ProjectOp column rename | RETURN AS alias 적용 |
| EXEC-T005 | CreateOp node creation | 노드 생성 및 ID 바인딩 |
| EXEC-T006 | DeleteOp with relationships | ConstraintError (DETACH 없이) |
| EXEC-T007 | eval_cmp: Int64 vs Float64 | 프로모션 후 정상 비교 |
| EXEC-T008 | eval_cmp: Null vs anything | false 반환 |
| EXEC-T009 | eval_cmp: type mismatch | ExecutionError |
| EXEC-T010 | AggregateOp count(*) | 전체 행 수 정확히 반환 |

### 5.4 Integration Tests

| Test ID | 시나리오 | 예상 결과 |
|---------|---------|-----------|
| INT-T001 | `CypherLite::open()` -> `execute("CREATE ...")` -> `execute("MATCH ...")` | 생성된 데이터 조회 성공 |
| INT-T002 | 매개변수 바인딩 `$name` | Params에서 값 치환 |
| INT-T003 | 명시적 Transaction commit/rollback | 커밋 후 데이터 보존, 롤백 후 데이터 소실 |
| INT-T004 | 잘못된 Cypher -> ParseError | 패닉 없이 에러 반환 |
| INT-T005 | 존재하지 않는 레이블 MATCH | 빈 결과 (에러 아님) |
| INT-T006 | SET 후 MATCH로 변경 확인 | 프로퍼티 갱신 반영 |
| INT-T007 | DETACH DELETE | 노드와 연결된 모든 엣지 함께 삭제 |

---

## 6. Verification Methods (검증 방법)

| 검증 유형 | 도구 | 대상 |
|-----------|------|------|
| 단위 테스트 | `cargo test` | 렉서, 파서, 시맨틱 분석, 각 연산자 |
| 통합 테스트 | `cargo test --test '*'` | 엔드투엔드 쿼리 실행 |
| 속성 기반 테스트 | `proptest` | 파서 fuzzing (무작위 입력에 패닉 없음) |
| 벤치마크 | `criterion` | parse, plan, execute 레이턴시 |
| 정적 분석 | `cargo clippy` | 코드 품질, 잠재 버그 |
| 포맷 검사 | `cargo fmt --check` | 일관된 코드 스타일 |
| 커버리지 | `cargo tarpaulin` | 85% 이상 라인 커버리지 |
