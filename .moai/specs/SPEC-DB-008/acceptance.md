---
id: SPEC-DB-008
type: acceptance
version: "0.8.0"
status: approved
created: "2026-03-13"
updated: "2026-03-13"
author: epsilondelta
tags: [inline-property-filter, pattern-matching, query-engine, bug-fix]
---

# SPEC-DB-008 Acceptance Criteria: Inline Property Filters

## 1. Node Inline Property Filters

### Scenario 1: 단일 프로퍼티 필터 (QQ-001)

```gherkin
Given 다음 노드들이 생성되어 있다:
  | label  | name    | age |
  | Person | Alice   | 30  |
  | Person | Bob     | 25  |
  | Person | Charlie | 30  |
When 쿼리를 실행한다: MATCH (n:Person {name: "Alice"}) RETURN n.name, n.age
Then 결과는 정확히 1행이어야 한다
And 결과에 name="Alice", age=30이 포함되어야 한다
And 결과에 "Bob"이 포함되지 않아야 한다
And 결과에 "Charlie"가 포함되지 않아야 한다
```

### Scenario 2: 복수 프로퍼티 필터 -- AND 논리 (QQ-001, QQ-002)

```gherkin
Given 다음 노드들이 생성되어 있다:
  | label  | name  | age |
  | Person | Alice | 30  |
  | Person | Alice | 25  |
  | Person | Bob   | 30  |
When 쿼리를 실행한다: MATCH (n:Person {name: "Alice", age: 30}) RETURN n
Then 결과는 정확히 1행이어야 한다
And 결과의 name은 "Alice"이고 age는 30이어야 한다
```

### Scenario 3: null 프로퍼티 필터 (QQ-003)

```gherkin
Given 다음 노드들이 생성되어 있다:
  | label  | name  | email          |
  | Person | Alice | alice@test.com |
  | Person | Bob   | (없음)          |
When 쿼리를 실행한다: MATCH (n:Person {email: null}) RETURN n.name
Then 결과는 정확히 1행이어야 한다
And 결과에 name="Bob"이 포함되어야 한다
```

### Scenario 4: 매칭되는 노드 없음 (QQ-001)

```gherkin
Given Person 노드 "Alice"와 "Bob"이 생성되어 있다
When 쿼리를 실행한다: MATCH (n:Person {name: "NonExistent"}) RETURN n
Then 결과는 0행이어야 한다
```

### Scenario 5: 빈 프로퍼티 맵 -- 필터 없음 (TT-001)

```gherkin
Given Person 노드 "Alice"와 "Bob"이 생성되어 있다
When 쿼리를 실행한다: MATCH (n:Person {}) RETURN n.name
Then 결과는 2행이어야 한다
And "Alice"와 "Bob" 모두 포함되어야 한다
```

---

## 2. Relationship Inline Property Filters

### Scenario 6: 관계 프로퍼티 필터 (RR-001)

```gherkin
Given 다음 그래프가 생성되어 있다:
  Alice -[:KNOWS {since: 2020}]-> Bob
  Alice -[:KNOWS {since: 2023}]-> Charlie
When 쿼리를 실행한다: MATCH (a)-[r:KNOWS {since: 2020}]->(b) RETURN a.name, b.name
Then 결과는 정확히 1행이어야 한다
And 결과에 a.name="Alice", b.name="Bob"이 포함되어야 한다
```

### Scenario 7: 관계 복수 프로퍼티 필터 (RR-001, RR-002)

```gherkin
Given 다음 그래프가 생성되어 있다:
  Alice -[:KNOWS {since: 2020, strength: 5}]-> Bob
  Alice -[:KNOWS {since: 2020, strength: 3}]-> Charlie
When 쿼리를 실행한다: MATCH (a)-[r:KNOWS {since: 2020, strength: 5}]->(b) RETURN b.name
Then 결과는 정확히 1행이어야 한다
And 결과에 b.name="Bob"이 포함되어야 한다
```

---

## 3. Combined Filters

### Scenario 8: 인라인 + WHERE 조합 (TT-002)

```gherkin
Given 다음 노드들이 생성되어 있다:
  | label  | name  | age |
  | Person | Alice | 30  |
  | Person | Alice | 20  |
  | Person | Bob   | 35  |
When 쿼리를 실행한다: MATCH (n:Person {name: "Alice"}) WHERE n.age > 25 RETURN n.age
Then 결과는 정확히 1행이어야 한다
And 결과에 age=30이 포함되어야 한다
```

### Scenario 9: 소스 노드 + 관계 + 대상 노드 인라인 필터 조합

```gherkin
Given 다음 그래프가 생성되어 있다:
  Alice -[:KNOWS {since: 2020}]-> Bob
  Alice -[:KNOWS {since: 2023}]-> Charlie
  Dave  -[:KNOWS {since: 2020}]-> Bob
When 쿼리를 실행한다:
  MATCH (a:Person {name: "Alice"})-[r:KNOWS {since: 2020}]->(b)
  RETURN b.name
Then 결과는 정확히 1행이어야 한다
And 결과에 b.name="Bob"이 포함되어야 한다
```

---

## 4. Backward Compatibility

### Scenario 10: 인라인 프로퍼티 없는 쿼리 -- 동작 무변경 (TT-001)

```gherkin
Given Person 노드 "Alice"와 "Bob"이 생성되어 있다
When 쿼리를 실행한다: MATCH (n:Person) RETURN n.name
Then 결과는 2행이어야 한다
And 수정 전과 동일한 결과를 반환해야 한다
```

### Scenario 11: WHERE 절만 사용 -- 동작 무변경 (TT-002)

```gherkin
Given Person 노드 Alice(age=30), Bob(age=25)이 생성되어 있다
When 쿼리를 실행한다: MATCH (n:Person) WHERE n.age > 27 RETURN n.name
Then 결과는 정확히 1행이어야 한다
And 결과에 name="Alice"가 포함되어야 한다
```

---

## 5. Subgraph Path Regression

### Scenario 12: Subgraph 인라인 필터 -- 기존 동작 유지

```gherkin
Given Subgraph 노드가 생성되어 있다 (feature: subgraph 활성)
When Subgraph 패턴에 인라인 프로퍼티를 사용하는 쿼리를 실행한다
Then 결과는 수정 전과 동일해야 한다
```

---

## 6. Quality Gates

### Performance

| Metric | Criteria |
|--------|---------|
| 인라인 프로퍼티 없는 MATCH | 기존 대비 성능 회귀 없음 (< 5% variance) |
| 인라인 프로퍼티 있는 MATCH | 동등한 WHERE 절 쿼리와 유사한 성능 |

### Coverage

| Metric | Criteria |
|--------|---------|
| `planner/mod.rs` 커버리지 | >= 85% |
| 신규 테스트 파일 커버리지 | >= 90% |

### Test Suite

| Metric | Criteria |
|--------|---------|
| 기존 테스트 전체 통과 | 1,241 tests PASS |
| 신규 테스트 | >= 10 test cases |
| Proptest | 인라인 필터 = full scan + filter 등가성 검증 |

---

## 7. Definition of Done

- [ ] `build_inline_property_predicate()` 유틸리티 함수 추출 완료
- [ ] Subgraph 경로 리팩토링 (유틸리티 함수 사용) + 기존 테스트 통과
- [ ] NodeScan 경로에 인라인 노드 프로퍼티 Filter 삽입
- [ ] Expand 경로에 관계 프로퍼티 Filter 삽입
- [ ] Expand 경로에 대상 노드 프로퍼티 Filter 삽입
- [ ] VarLengthExpand 경로에 동일 처리
- [ ] 단일/복수 프로퍼티 필터 테스트 통과
- [ ] null 프로퍼티 테스트 통과
- [ ] 관계 프로퍼티 필터 테스트 통과
- [ ] 인라인 + WHERE 조합 테스트 통과
- [ ] 빈 프로퍼티 맵 테스트 통과
- [ ] 기존 전체 테스트 스위트 통과 (1,241 tests)
- [ ] 수정 대상 파일 커버리지 >= 85%
- [ ] 성능 회귀 없음 확인
- [ ] 버전 범프 0.7.0 -> 0.8.0
