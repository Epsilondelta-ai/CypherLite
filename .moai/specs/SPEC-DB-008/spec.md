---
id: SPEC-DB-008
version: "0.8.0"
status: approved
created: "2026-03-13"
updated: "2026-03-13"
author: epsilondelta
priority: P1
tags: [inline-property-filter, pattern-matching, query-engine, bug-fix]
lifecycle: spec-anchored
depends_on: [SPEC-DB-002, SPEC-DB-003]
---

# SPEC-DB-008: CypherLite Phase 8 - Inline Property Filters (v0.8)

> MATCH 패턴의 인라인 프로퍼티 필터(`{key: value}`)가 파싱은 되지만 실행 시 무시되는 알려진 버그를 수정한다. 노드 패턴과 관계 패턴 모두에 대해 플래너 수준에서 Filter 논리 계획 노드를 삽입하여 정확한 필터링을 보장한다.

---

## HISTORY

| Version | Date | Changes |
|---------|------|---------|
| 0.8.0 | 2026-03-13 | 딥 리서치 기반 초기 SPEC 작성. 플래너 버그 분석 완료, Subgraph 경로 참조 구현 확인 |

---

## 1. Environment

### 1.1 Rust Version

- Rust 1.92+ (2024 edition)
- CypherLite v0.7.0 -> v0.8.0

### 1.2 Crate Structure

- **수정 대상 crate**: `cypherlite-query` -- 플래너의 `plan_pattern_chain()` 함수
- **영향 없음**: `cypherlite-core`, `cypherlite-storage` -- 타입 및 스토리지 변경 불필요

### 1.3 Feature Flags

- 기본 feature set으로 동작 (feature gating 불필요)
- 모든 feature 조합에서 동일하게 적용

---

## 2. Requirements (EARS Format)

### Group QQ: Inline Node Property Filters

**QQ-001**: **WHEN** MATCH 절에 인라인 프로퍼티를 가진 노드 패턴 `(n:Label {key: value})`이 포함되어 있으면, **THEN** 시스템은 지정된 모든 key-value 쌍과 프로퍼티가 일치하는 노드만 결과에 포함해야 한다(SHALL).

**QQ-002**: **WHEN** MATCH 절의 노드 패턴에 복수의 인라인 프로퍼티가 있으면, **THEN** 시스템은 모든 프로퍼티 조건을 AND 논리로 결합해야 한다(SHALL).

**QQ-003**: **WHEN** 인라인 프로퍼티 값이 null이면, **THEN** 시스템은 해당 프로퍼티가 존재하지 않거나 null인 노드를 매칭해야 한다(SHALL).

### Group RR: Inline Relationship Property Filters

**RR-001**: **WHEN** MATCH 절에 인라인 프로퍼티를 가진 관계 패턴 `-[r:TYPE {key: value}]->`이 포함되어 있으면, **THEN** 시스템은 지정된 모든 key-value 쌍과 프로퍼티가 일치하는 관계만 결과에 포함해야 한다(SHALL).

**RR-002**: **WHEN** MATCH 절의 관계 패턴에 복수의 인라인 프로퍼티가 있으면, **THEN** 시스템은 모든 프로퍼티 조건을 AND 논리로 결합해야 한다(SHALL).

### Group SS: Predicate Extraction

**SS-001**: 플래너는 인라인 프로퍼티 맵을 소스 스캔을 감싸는 `Filter` 논리 계획 노드로 변환해야 한다(SHALL).

**SS-002**: 플래너는 Subgraph 코드 경로(`planner/mod.rs:531-554`)에 이미 구현된 predicate 빌딩 로직을 재사용해야 한다(SHALL).
> Reference: `plan_pattern_chain()` 내 Subgraph 분기(line 531-554)에서 `NodePattern.properties`를 `Expression::BinaryOp(Eq, Property, Value)` 체인으로 변환 후 AND로 결합하여 `LogicalPlan::Filter`로 감싸는 패턴이 정확히 동일한 요구사항이다.

**SS-003**: 관계 패턴의 인라인 프로퍼티는 Expand 노드 직후에 `Filter` 논리 계획 노드를 삽입하여 적용해야 한다(SHALL).

### Group TT: Backward Compatibility

**TT-001**: 인라인 프로퍼티가 없는 쿼리는 기존과 동일하게 동작해야 한다(SHALL).

**TT-002**: WHERE 절 필터는 기존과 동일하게 동작해야 하며, 인라인 프로퍼티 필터와 올바르게 조합되어야 한다(SHALL).
> 인라인 프로퍼티 필터가 먼저 적용되고, WHERE 절은 그 위에 추가 Filter로 적용된다.

### Group UU: Quality

**UU-001**: 기존의 모든 테스트가 계속 통과해야 한다(SHALL).

**UU-002**: 새로운 테스트는 다음 시나리오를 커버해야 한다(SHALL):
- 노드 단일 프로퍼티 필터
- 노드 복수 프로퍼티 필터
- null 값 프로퍼티 필터
- 엣지 프로퍼티 필터
- 인라인 필터와 WHERE 절의 조합
- 빈 프로퍼티 맵 `{}`
- Subgraph 패턴의 인라인 필터 (기존 동작 검증)

**UU-003**: 수정 대상 파일의 코드 커버리지가 85% 이상이어야 한다(SHALL).

**UU-004**: 인라인 프로퍼티가 없는 쿼리에 대해 성능 회귀가 없어야 한다(SHALL).

---

## 3. Non-Goals

- 인라인 프로퍼티에 대한 인덱스 최적화 (현재 full scan + filter 방식으로 충분)
- 프로퍼티 값에 복잡한 표현식 지원 (예: `{age: 20 + 10}`) -- 리터럴 값만 지원
- MERGE 패턴의 인라인 프로퍼티 필터링 (CREATE/MERGE는 이미 프로퍼티를 설정 용도로 사용)
- Variable-length path 내 관계 프로퍼티 필터링

---

## 4. Architecture

### 4.1 버그 원인 분석

`plan_pattern_chain()` (`planner/mod.rs:502-665`)에서:

1. **Subgraph 경로 (line 526-598)**: `first_node.properties`를 확인하여 `Expression::BinaryOp(Eq, ...)` 체인을 만들고 `LogicalPlan::Filter`로 감싼다 -- **올바르게 동작함**.
2. **일반 NodeScan 경로 (line 600-665)**: `first_node.properties`를 **완전히 무시**하고 `LogicalPlan::NodeScan`만 생성한다 -- **버그**.
3. **Expand 경로 (line 608-661)**: `rel.properties`와 `target_node.properties`를 **완전히 무시**한다 -- **버그**.

### 4.2 수정 전략

**핵심 원칙**: Subgraph 경로의 predicate 빌딩 로직을 유틸리티 함수로 추출하여 세 곳에서 재사용.

```
수정 전:
  NodeScan(variable, label_id) -- 프로퍼티 무시

수정 후:
  Filter(
    predicate: n.name = "Alice" AND n.age = 30,
    source: NodeScan(variable, label_id)
  )
```

**적용 지점**:

| 위치 | 적용 대상 | 설명 |
|------|----------|------|
| line ~605 | `first_node.properties` | NodeScan 직후 Filter 삽입 |
| line ~652-660 | `rel.properties` | Expand 직후 관계 프로퍼티 Filter 삽입 |
| line ~652-660 | `target_node.properties` | Expand 직후 대상 노드 프로퍼티 Filter 삽입 |

### 4.3 유틸리티 함수 설계

```rust
/// 인라인 프로퍼티 맵을 AND-결합된 predicate Expression으로 변환한다.
/// Subgraph 경로(line 531-548)의 기존 로직을 추출한 것이다.
fn build_inline_property_predicate(
    variable: &str,
    properties: &MapLiteral,
) -> Option<Expression> {
    let predicates: Vec<Expression> = properties
        .iter()
        .map(|(key, val_expr)| {
            Expression::BinaryOp(
                BinaryOp::Eq,
                Box::new(Expression::Property(
                    Box::new(Expression::Variable(variable.to_string())),
                    key.clone(),
                )),
                Box::new(val_expr.clone()),
            )
        })
        .collect();
    predicates.into_iter().reduce(|acc, p| {
        Expression::BinaryOp(BinaryOp::And, Box::new(acc), Box::new(p))
    })
}
```

### 4.4 영향 범위

| 파일 | 변경 유형 | 설명 |
|------|----------|------|
| `crates/cypherlite-query/src/planner/mod.rs` | 수정 | `build_inline_property_predicate()` 추출, NodeScan/Expand 경로에 Filter 삽입. Subgraph 경로 리팩토링(유틸리티 함수 사용). |
| `crates/cypherlite-query/tests/` | 신규/확장 | 인라인 프로퍼티 필터 통합 테스트 |

**수정되지 않는 파일**:
- Parser (이미 올바르게 파싱)
- Executor (Filter operator와 eval 로직 이미 정상 동작)
- Core types (타입 변경 불필요)
- Storage (스토리지 변경 불필요)

---

## 5. Example Queries

### 5.1 수정 전후 동작 비교

**쿼리 1: 단일 노드 프로퍼티 필터**
```cypher
CREATE (a:Person {name: 'Alice', age: 30})
CREATE (b:Person {name: 'Bob', age: 25})
MATCH (n:Person {name: 'Alice'})
RETURN n.name, n.age
```
- 수정 전: Alice, Bob 모두 반환 (버그)
- 수정 후: Alice만 반환

**쿼리 2: 복수 프로퍼티 필터**
```cypher
MATCH (n:Person {name: 'Alice', age: 30})
RETURN n
```
- 수정 전: 모든 Person 반환 (버그)
- 수정 후: name='Alice' AND age=30인 노드만 반환

**쿼리 3: 관계 프로퍼티 필터**
```cypher
MATCH (a:Person)-[r:KNOWS {since: 2020}]->(b:Person)
RETURN a.name, b.name, r.since
```
- 수정 전: 모든 KNOWS 관계 반환 (버그)
- 수정 후: since=2020인 관계만 반환

**쿼리 4: 인라인 + WHERE 조합**
```cypher
MATCH (n:Person {name: 'Alice'})
WHERE n.age > 25
RETURN n
```
- 수정 전: 모든 Person 중 age > 25 반환 (인라인 필터 무시)
- 수정 후: name='Alice' AND age > 25인 노드만 반환

**쿼리 5: 빈 프로퍼티 맵**
```cypher
MATCH (n:Person {})
RETURN n
```
- 수정 전/후 동일: 모든 Person 반환 (빈 맵은 필터 없음)

---

## 6. Risk Assessment

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| 기존 테스트 실패 | Low | High | 기존 테스트 중 인라인 프로퍼티 의존이 없으므로 안전. 수정 전 전체 테스트 실행으로 검증 |
| Subgraph 경로 리팩토링 부작용 | Low | Medium | 로직 추출만 수행 (동작 변경 없음). 기존 Subgraph 테스트로 검증 |
| 성능 회귀 | Very Low | Low | 프로퍼티 없는 경우 추가 코드 경로 없음 (`if let Some` 가드). 벤치마크로 확인 |
| Expand 경로 복잡도 증가 | Low | Low | 유틸리티 함수로 깔끔하게 분리. 각 적용 지점은 3-5줄 추가 |

**전체 위험도**: LOW -- 플래너 내 국소적 변경이며, 이미 검증된 Subgraph 코드 경로의 로직을 재사용.

---

## 7. Implementation Phases

### Phase 8a: Node Inline Property Filters (Priority: Primary Goal)

**Scope**: 플래너의 NodeScan 경로에 인라인 프로퍼티 Filter 삽입 + 유틸리티 함수 추출

**Requirements covered**: QQ-001, QQ-002, QQ-003, SS-001, SS-002, TT-001, TT-002

**File impact**:

| File | Change Type | Description |
|------|------------|-------------|
| `crates/cypherlite-query/src/planner/mod.rs` | 수정 | `build_inline_property_predicate()` 함수 추출. Subgraph 경로(line 531-548)를 유틸리티 함수 호출로 리팩토링. NodeScan 생성 직후(line ~605) `first_node.properties`에 대한 Filter 삽입 |
| `crates/cypherlite-query/tests/` | 신규/확장 | 노드 인라인 프로퍼티 필터 테스트: 단일/복수 프로퍼티, null 값, WHERE 조합, 빈 맵 |

### Phase 8b: Relationship Inline Property Filters (Priority: Secondary Goal)

**Scope**: Expand 경로에 관계 및 대상 노드 인라인 프로퍼티 Filter 삽입

**Requirements covered**: RR-001, RR-002, SS-003

**File impact**:

| File | Change Type | Description |
|------|------------|-------------|
| `crates/cypherlite-query/src/planner/mod.rs` | 수정 | Expand 생성 직후(line ~652-660) `rel.properties`와 `target_node.properties`에 대한 Filter 삽입. VarLengthExpand 경로도 동일하게 처리 |
| `crates/cypherlite-query/tests/` | 확장 | 관계 인라인 프로퍼티 필터 테스트: 단일/복수 프로퍼티, 방향별 테스트 |

### Phase 8c: Quality (Priority: Final Goal)

**Scope**: Proptest, 엣지 케이스, 성능 벤치마크, 버전 범프

**Requirements covered**: UU-001, UU-002, UU-003, UU-004, PP (version bump)

**File impact**:

| File | Change Type | Description |
|------|------------|-------------|
| `crates/cypherlite-query/tests/` | 확장 | Proptest: 랜덤 프로퍼티 생성 후 인라인 필터 + full scan 비교 |
| `crates/cypherlite-query/benches/` | 확장 | 인라인 프로퍼티 유/무 성능 비교 벤치마크 |
| `Cargo.toml` (all crates) | 수정 | 버전 범프 0.7.0 -> 0.8.0 |

### Phase Dependencies

```
Phase 8a (Node Inline Filters)
    |
    +---> Phase 8b (Relationship Inline Filters) -- 유틸리티 함수에 의존
    |
    +---> Phase 8c (Quality) -- 모든 기능 구현 후 수행
```
