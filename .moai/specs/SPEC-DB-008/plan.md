---
id: SPEC-DB-008
type: plan
version: "0.8.0"
status: approved
created: "2026-03-13"
updated: "2026-03-13"
author: epsilondelta
tags: [inline-property-filter, pattern-matching, query-engine, bug-fix]
---

# SPEC-DB-008 Implementation Plan: Inline Property Filters

## 1. Overview

CypherLite의 MATCH 패턴에서 인라인 프로퍼티 필터(`{key: value}`)가 파싱은 되지만 실행 시 무시되는 버그를 수정한다. 플래너(`plan_pattern_chain()`)에서 NodeScan 및 Expand 경로에 `Filter` 논리 계획 노드를 삽입하는 국소적 수정이다.

## 2. Reference Implementation

### 2.1 동작하는 코드 (Subgraph 경로)

`crates/cypherlite-query/src/planner/mod.rs` line 531-554:

```rust
// Subgraph 경로 -- 인라인 프로퍼티를 Filter로 변환 (정상 동작)
if let Some(ref props) = first_node.properties {
    let predicates: Vec<Expression> = props
        .iter()
        .map(|(key, val_expr)| {
            Expression::BinaryOp(
                BinaryOp::Eq,
                Box::new(Expression::Property(
                    Box::new(Expression::Variable(variable.clone())),
                    key.clone(),
                )),
                Box::new(val_expr.clone()),
            )
        })
        .collect();
    let predicate = predicates.into_iter().reduce(|acc, p| {
        Expression::BinaryOp(BinaryOp::And, Box::new(acc), Box::new(p))
    });
    if let Some(pred) = predicate {
        plan = LogicalPlan::Filter {
            source: Box::new(plan),
            predicate: pred,
        };
    }
}
```

### 2.2 버그가 있는 코드 (NodeScan 경로)

`crates/cypherlite-query/src/planner/mod.rs` line 600-665:

```rust
// NodeScan 경로 -- first_node.properties를 완전히 무시 (버그)
let mut plan = LogicalPlan::NodeScan { variable, label_id, limit: None };

// 바로 relationship 처리로 넘어감 -- 인라인 프로퍼티 처리 없음
while let Some(rel_elem) = elements.next() {
    // ... Expand 생성 시에도 rel.properties, target_node.properties 무시
}
```

## 3. Task Decomposition

### Phase 8a: Node Inline Property Filters (Primary Goal)

**Task 8a-1: 유틸리티 함수 추출**
- `planner/mod.rs`에 `build_inline_property_predicate(variable, properties) -> Option<Expression>` 헬퍼 함수 생성
- Subgraph 경로(line 531-548)의 기존 inline 코드를 이 함수 호출로 리팩토링
- 기존 Subgraph 테스트가 여전히 통과하는지 확인

**Task 8a-2: NodeScan 경로에 Filter 삽입**
- line ~605 (`LogicalPlan::NodeScan` 생성 직후)에 `first_node.properties` 체크 추가
- `build_inline_property_predicate()` 호출 후 `LogicalPlan::Filter`로 plan 감싸기
- 빈 프로퍼티 맵(`Some(vec![])`)은 필터 없이 통과

**Task 8a-3: 노드 인라인 프로퍼티 테스트 작성**
- 단일 프로퍼티: `MATCH (n:Person {name: "Alice"})` -> Alice만 반환
- 복수 프로퍼티: `MATCH (n:Person {name: "Alice", age: 30})` -> 정확히 매칭
- null 프로퍼티: `MATCH (n:Person {email: null})` -> email 없는 노드
- WHERE 조합: `MATCH (n:Person {name: "Alice"}) WHERE n.age > 25` -> 양쪽 모두 적용
- 빈 맵: `MATCH (n:Person {})` -> 모든 Person (필터 없음)
- 매칭 없음: `MATCH (n:Person {name: "NonExistent"})` -> 빈 결과

### Phase 8b: Relationship Inline Property Filters (Secondary Goal)

**Task 8b-1: Expand 경로에 관계 프로퍼티 Filter 삽입**
- `Expand` 생성 직후(line ~652-660) `rel.properties` 체크 추가
- `build_inline_property_predicate(rel_var, rel.properties)` 호출
- Filter로 plan 감싸기

**Task 8b-2: Expand 경로에 대상 노드 프로퍼티 Filter 삽입**
- `Expand` 생성 직후 `target_node.properties` 체크 추가
- `build_inline_property_predicate(target_var, target_node.properties)` 호출
- Filter로 plan 감싸기 (관계 Filter 위에 추가)

**Task 8b-3: VarLengthExpand 경로 동일 처리**
- `VarLengthExpand` 생성 직후(line ~640-650)에도 동일한 프로퍼티 필터 삽입
- 관계 프로퍼티와 대상 노드 프로퍼티 모두 처리

**Task 8b-4: 관계 인라인 프로퍼티 테스트 작성**
- 관계 프로퍼티: `MATCH ()-[r:KNOWS {since: 2020}]->()` -> 필터링된 관계
- 대상 노드: `MATCH (a)-[:KNOWS]->(b:Person {name: "Bob"})` -> Bob만 대상
- 양쪽 모두: `MATCH (a:Person {name: "Alice"})-[r:KNOWS {since: 2020}]->(b)` -> 소스 + 관계 필터

### Phase 8c: Quality (Final Goal)

**Task 8c-1: Proptest**
- 랜덤 프로퍼티 맵 생성 -> 인라인 필터 결과 = full scan + manual filter 결과 검증

**Task 8c-2: 성능 벤치마크**
- 인라인 프로퍼티 없는 MATCH 쿼리의 수정 전후 성능 비교
- 인라인 프로퍼티 있는 MATCH 쿼리의 성능 측정

**Task 8c-3: 버전 범프**
- 모든 `Cargo.toml`: `version = "0.7.0"` -> `version = "0.8.0"`

## 4. Files to Modify

| File | Phase | Change Type | 예상 변경량 |
|------|-------|------------|-----------|
| `crates/cypherlite-query/src/planner/mod.rs` | 8a, 8b | 수정 | ~50-70 lines 추가, ~20 lines 리팩토링 |
| `crates/cypherlite-query/tests/inline_property_filter.rs` | 8a, 8b | 신규 | ~200 lines |
| `crates/cypherlite-query/tests/proptest_inline_filter.rs` | 8c | 신규 | ~80 lines |
| `crates/cypherlite-query/benches/inline_filter.rs` | 8c | 신규 | ~60 lines |
| `Cargo.toml` (3 crates) | 8c | 수정 | 각 1 line |

**총 예상 변경량**: ~100-150 lines 코드 + ~340 lines 테스트

## 5. Technical Approach

### 5.1 수정 순서

1. **추출 먼저**: Subgraph 경로의 인라인 코드를 헬퍼 함수로 추출 (동작 변경 없음)
2. **Subgraph 테스트 확인**: 기존 테스트 통과 확인
3. **NodeScan 적용**: 헬퍼 함수를 NodeScan 경로에 적용
4. **Expand 적용**: 헬퍼 함수를 Expand/VarLengthExpand 경로에 적용
5. **통합 테스트**: 모든 시나리오 테스트 작성 및 실행

### 5.2 Filter 삽입 순서 (Expand 경로)

```
Expand(source, src_var, rel_var, target_var, ...)
  |
  +-- rel.properties Filter (rel_var 기준)
  |
  +-- target_node.properties Filter (target_var 기준)
```

관계 프로퍼티 필터가 먼저, 대상 노드 프로퍼티 필터가 그 위에 적용된다.

### 5.3 WHERE 절과의 상호작용

인라인 프로퍼티 필터는 `plan_pattern_chain()` 내부에서 적용되고, WHERE 절은 `plan_match_clause()`에서 별도로 적용된다. 따라서 자연스럽게 인라인 필터 -> WHERE 필터 순서로 중첩된다.

```
Filter(WHERE predicate,
  source: Filter(inline predicate,
    source: NodeScan(...)
  )
)
```

## 6. Risk & Mitigation

| Risk | Mitigation |
|------|------------|
| Subgraph 리팩토링 부작용 | 로직 추출만 수행, 기존 Subgraph 테스트로 즉시 검증 |
| null 프로퍼티 비교 | 기존 `eval_expression()`의 null 비교 로직 활용 (`Value::Null == Value::Null` -> true) |
| 빈 MapLiteral 처리 | `reduce()`가 빈 Vec에 대해 `None` 반환 -> Filter 미삽입 (안전) |

## 7. Scope Estimate

- **코드 변경 범위**: `planner/mod.rs` 1개 파일 (핵심 수정)
- **코드량**: ~100-150 lines 추가/수정
- **테스트**: ~340 lines 신규
- **위험도**: LOW (국소적 플래너 변경, 검증된 패턴 재사용)
