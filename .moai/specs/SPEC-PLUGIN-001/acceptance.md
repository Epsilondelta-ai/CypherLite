---
id: SPEC-PLUGIN-001
type: acceptance
version: "1.0.0"
status: draft
created: "2026-03-13"
updated: "2026-03-13"
author: epsilondelta
tags: [plugin, extensibility, acceptance-criteria, gherkin]
traceability:
  spec: spec.md
  plan: plan.md
---

# SPEC-PLUGIN-001 수락 기준: Plugin System

## AC-001: Plugin 기본 트레이트 정의 및 Object Safety

**요구사항**: REQ-PLUGIN-001, REQ-PLUGIN-005

```gherkin
Scenario: Plugin 트레이트 object safety 및 Send+Sync 검증
  Given cypherlite-core 크레이트에 plugin 모듈이 존재한다
  And plugin feature flag가 활성화되어 있다
  When Plugin 트레이트를 구현하는 구조체를 정의한다
  Then Box<dyn Plugin>으로 변환할 수 있어야 한다 (object-safe)
  And dyn Plugin이 Send + Sync 바운드를 충족해야 한다
  And name()과 version() 메서드가 올바른 값을 반환해야 한다
```

---

## AC-002: PluginRegistry 등록 및 조회

**요구사항**: REQ-PLUGIN-002

```gherkin
Scenario: 플러그인 등록 및 이름 기반 조회
  Given 빈 PluginRegistry<dyn ScalarFunction>가 존재한다
  When "my_upper" 이름의 ScalarFunction 플러그인을 등록한다
  Then registry.get("my_upper")이 Some을 반환해야 한다
  And registry.contains("my_upper")이 true를 반환해야 한다
  And registry.list()에 ("my_upper", "1.0.0") 쌍이 포함되어야 한다

Scenario: 미등록 플러그인 조회
  Given 빈 PluginRegistry가 존재한다
  When 등록되지 않은 이름 "unknown"으로 조회한다
  Then registry.get("unknown")이 None을 반환해야 한다
  And registry.contains("unknown")이 false를 반환해야 한다
```

---

## AC-003: 중복 플러그인 등록 방지

**요구사항**: REQ-PLUGIN-003

```gherkin
Scenario: 동일 이름 플러그인 중복 등록 시 에러
  Given PluginRegistry에 "my_func" 이름의 플러그인이 등록되어 있다
  When 동일한 이름 "my_func"으로 다른 플러그인을 등록한다
  Then CypherLiteError::PluginError가 반환되어야 한다
  And 에러 메시지에 "my_func"이 포함되어야 한다
  And 기존 등록된 플러그인은 변경되지 않아야 한다
```

---

## AC-004: ScalarFunction 등록 및 쿼리 실행

**요구사항**: REQ-PLUGIN-010, REQ-PLUGIN-011

```gherkin
Scenario: 커스텀 스칼라 함수를 쿼리에서 호출
  Given CypherLite 인스턴스에 "double" 이름의 ScalarFunction이 등록되어 있다
  And "double" 함수는 정수 인자를 받아 2배를 반환한다
  When "RETURN double(21)" 쿼리를 실행한다
  Then 결과 행에 42(i64)가 반환되어야 한다

Scenario: 커스텀 스칼라 함수를 노드 프로퍼티와 함께 호출
  Given CypherLite 인스턴스에 "my_upper" 이름의 ScalarFunction이 등록되어 있다
  And "my_upper" 함수는 문자열을 대문자로 변환한다
  And (n:Person {name: "alice"}) 노드가 존재한다
  When "MATCH (n:Person) RETURN my_upper(n.name)" 쿼리를 실행한다
  Then 결과 행에 "ALICE"(String)가 반환되어야 한다
```

---

## AC-005: 내장 함수 동작 불변 검증

**요구사항**: REQ-PLUGIN-012

```gherkin
Scenario: plugin 활성화 후 내장 함수 정상 동작
  Given plugin feature flag가 활성화되어 있다
  And CypherLite 인스턴스에 어떤 플러그인도 등록되어 있지 않다
  When 내장 함수 id(), type(), labels(), toUpper(), size() 등을 포함하는 쿼리를 실행한다
  Then 모든 내장 함수가 plugin 비활성화 시와 동일한 결과를 반환해야 한다
```

---

## AC-006: 미등록 함수 호출 시 에러

**요구사항**: REQ-PLUGIN-013

```gherkin
Scenario: 내장/등록 함수 모두에 없는 함수 호출
  Given plugin feature flag가 활성화되어 있다
  And "nonexistent_func" 이름의 함수가 내장도 아니고 등록도 되어 있지 않다
  When "RETURN nonexistent_func(1)" 쿼리를 실행한다
  Then CypherLiteError::FunctionNotFound가 반환되어야 한다
  And 에러 메시지에 "nonexistent_func"이 포함되어야 한다
```

---

## AC-007: IndexPlugin 등록 및 인덱스 조작

**요구사항**: REQ-PLUGIN-020, REQ-PLUGIN-021

```gherkin
Scenario: 커스텀 인덱스 플러그인 등록 및 lookup
  Given CypherLite 인스턴스에 "fulltext" 타입의 IndexPlugin이 등록되어 있다
  And "fulltext" 인덱스에 PropertyValue::String("hello world") -> NodeId(1)이 삽입되어 있다
  When "fulltext" 인덱스에서 PropertyValue::String("hello world")를 lookup 한다
  Then 결과에 NodeId(1)이 포함되어야 한다

Scenario: 커스텀 인덱스에서 항목 제거
  Given "fulltext" 인덱스에 PropertyValue::String("test") -> NodeId(2)이 삽입되어 있다
  When PropertyValue::String("test"), NodeId(2)를 remove 한다
  Then 이후 lookup에서 NodeId(2)가 포함되지 않아야 한다
```

---

## AC-008: 미지원 인덱스 타입 에러

**요구사항**: REQ-PLUGIN-022

```gherkin
Scenario: 등록되지 않은 인덱스 타입 요청
  Given "spatial" 타입의 IndexPlugin이 등록되어 있지 않다
  When "spatial" 타입의 인덱스 생성을 요청한다
  Then CypherLiteError::UnsupportedIndexType가 반환되어야 한다
  And 에러 메시지에 "spatial"이 포함되어야 한다
```

---

## AC-009: Serializer 등록 및 export/import 라운드트립

**요구사항**: REQ-PLUGIN-030, REQ-PLUGIN-031

```gherkin
Scenario: JSON Serializer로 export/import 라운드트립
  Given CypherLite 인스턴스에 "json" 포맷의 Serializer가 등록되어 있다
  And 데이터베이스에 (a:Person {name: "Alice", age: 30}) 노드가 존재한다
  When "MATCH (n:Person) RETURN n.name, n.age" 쿼리 결과를 "json" 포맷으로 export 한다
  Then 유효한 JSON 바이트 배열이 반환되어야 한다
  When 해당 JSON 바이트를 "json" 포맷으로 import 한다
  Then import된 Row의 데이터가 원본과 일치해야 한다

Scenario: CSV Serializer로 export
  Given CypherLite 인스턴스에 "csv" 포맷의 Serializer가 등록되어 있다
  And 쿼리 결과 Row 데이터가 존재한다
  When "csv" 포맷으로 export 한다
  Then CSV 형식의 바이트 배열이 반환되어야 한다
```

---

## AC-010: 미지원 포맷 에러

**요구사항**: REQ-PLUGIN-032

```gherkin
Scenario: 등록되지 않은 포맷으로 export 요청
  Given "xml" 포맷의 Serializer가 등록되어 있지 않다
  When "xml" 포맷으로 export를 요청한다
  Then CypherLiteError::UnsupportedFormat이 반환되어야 한다
  And 에러 메시지에 "xml"이 포함되어야 한다
```

---

## AC-011: Trigger before/after 훅 호출

**요구사항**: REQ-PLUGIN-040, REQ-PLUGIN-041

```gherkin
Scenario: CREATE 연산 시 Trigger before/after 호출
  Given CypherLite 인스턴스에 "audit" 이름의 Trigger가 등록되어 있다
  And "audit" 트리거는 호출 기록을 내부 벡터에 저장한다
  When "CREATE (n:Person {name: 'Alice'})" 쿼리를 실행한다
  Then "audit" 트리거의 on_before_create가 먼저 호출되어야 한다
  And 그 다음 노드 생성이 수행되어야 한다
  And 마지막으로 on_after_create가 호출되어야 한다
  And TriggerContext에 entity_type: Node, label: "Person", operation: Create이 포함되어야 한다

Scenario: SET 연산 시 Trigger before/after 호출
  Given "audit" 트리거가 등록되어 있다
  And (n:Person {name: 'Alice'}) 노드가 존재한다
  When "MATCH (n:Person {name: 'Alice'}) SET n.age = 30" 쿼리를 실행한다
  Then on_before_update와 on_after_update가 순서대로 호출되어야 한다

Scenario: DELETE 연산 시 Trigger before/after 호출
  Given "audit" 트리거가 등록되어 있다
  And (n:Temp) 노드가 존재한다
  When "MATCH (n:Temp) DELETE n" 쿼리를 실행한다
  Then on_before_delete와 on_after_delete가 순서대로 호출되어야 한다
```

---

## AC-012: TriggerContext 데이터 정확성

**요구사항**: REQ-PLUGIN-043

```gherkin
Scenario: TriggerContext에 엔티티 정보가 정확히 전달됨
  Given "inspector" 트리거가 등록되어 있다
  And "inspector"는 전달받은 TriggerContext를 캡처한다
  When "CREATE (n:User {email: 'test@example.com'})" 쿼리를 실행한다
  Then on_before_create에 전달된 TriggerContext.entity_type이 EntityType::Node이어야 한다
  And TriggerContext.label_or_type이 Some("User")이어야 한다
  And TriggerContext.properties에 ("email", "test@example.com") 쌍이 포함되어야 한다
  And TriggerContext.operation이 TriggerOperation::Create이어야 한다
```

---

## AC-013: before 트리거 실패 시 연산 중단

**요구사항**: REQ-PLUGIN-042

```gherkin
Scenario: on_before_create 실패 시 노드 생성 중단
  Given "validator" 트리거가 등록되어 있다
  And "validator"의 on_before_create는 항상 에러를 반환한다
  When "CREATE (n:Blocked {name: 'should_not_exist'})" 쿼리를 실행한다
  Then CypherLiteError::TriggerError가 반환되어야 한다
  And 데이터베이스에 :Blocked 레이블의 노드가 존재하지 않아야 한다
  And on_after_create는 호출되지 않아야 한다

Scenario: on_before_delete 실패 시 삭제 중단
  Given "protector" 트리거가 등록되어 있다
  And "protector"의 on_before_delete는 항상 에러를 반환한다
  And (n:Protected {name: 'important'}) 노드가 존재한다
  When "MATCH (n:Protected) DELETE n" 쿼리를 실행한다
  Then CypherLiteError::TriggerError가 반환되어야 한다
  And (n:Protected {name: 'important'}) 노드가 여전히 존재해야 한다
```

---

## AC-014: 플러그인 에러 타입 및 전파

**요구사항**: REQ-PLUGIN-050, REQ-PLUGIN-051

```gherkin
Scenario: 플러그인 에러가 CypherLiteError로 래핑되어 전파됨
  Given "failing_func" ScalarFunction이 등록되어 있다
  And "failing_func"은 호출 시 커스텀 에러를 반환한다
  When "RETURN failing_func()" 쿼리를 실행한다
  Then CypherLiteError::PluginError가 반환되어야 한다
  And 에러 메시지에 원본 에러 정보가 포함되어야 한다
```

---

## AC-015: CypherLite API register/list 메서드

**요구사항**: REQ-PLUGIN-060, REQ-PLUGIN-061

```gherkin
Scenario: CypherLite API를 통한 플러그인 등록 및 목록 조회
  Given plugin feature가 활성화된 CypherLite 인스턴스가 있다
  When register_scalar_function으로 "my_func" 함수를 등록한다
  And register_index_plugin으로 "fulltext" 인덱스를 등록한다
  And register_serializer로 "json" 직렬화기를 등록한다
  And register_trigger로 "audit" 트리거를 등록한다
  Then list_functions()에 "my_func"이 포함되어야 한다
  And 각 레지스트리에서 등록된 플러그인이 올바르게 조회되어야 한다
```

---

## AC-016: Feature Flag 격리

**요구사항**: REQ-PLUGIN-004, REQ-PLUGIN-070

```gherkin
Scenario: plugin feature 비활성화 시 기존 동작 불변
  Given plugin feature flag가 비활성화되어 있다
  When cargo test --workspace (plugin 없이)를 실행한다
  Then 기존 모든 테스트(1,256개)가 통과해야 한다
  And 플러그인 관련 코드가 컴파일에 포함되지 않아야 한다

Scenario: plugin feature 활성화 시 전체 테스트 통과
  Given plugin feature flag가 활성화되어 있다
  When cargo test --workspace --all-features를 실행한다
  Then 기존 테스트와 신규 플러그인 테스트 모두 통과해야 한다
```

---

## AC-017: 커버리지 목표 달성

**요구사항**: REQ-PLUGIN-071

```gherkin
Scenario: 플러그인 코드 커버리지 85% 이상
  Given 모든 플러그인 Phase(10a~10e)가 완료되었다
  When cargo llvm-cov --workspace --features plugin 를 실행한다
  Then 플러그인 관련 코드의 라인 커버리지가 85% 이상이어야 한다
```

---

## AC-018: 성능 회귀 없음 및 오버헤드 제한

**요구사항**: REQ-PLUGIN-072, REQ-PLUGIN-073

```gherkin
Scenario: plugin 비활성화 시 성능 회귀 없음
  Given plugin feature flag가 비활성화되어 있다
  When 기존 벤치마크(내장 함수 호출, CREATE/MATCH 연산)를 실행한다
  Then 성능이 v0.9.0 대비 회귀되지 않아야 한다

Scenario: plugin 활성화 + 미등록 시 오버헤드 5% 이내
  Given plugin feature flag가 활성화되어 있다
  And 어떤 플러그인도 등록되어 있지 않다
  When 내장 함수 호출 벤치마크를 실행한다
  Then plugin 비활성화 대비 성능 오버헤드가 5% 이내여야 한다
```

---

## Definition of Done

- [ ] `cypherlite-core/src/plugin/mod.rs` 모듈이 생성되었다
- [ ] Plugin, ScalarFunction, IndexPlugin, Serializer, Trigger 트레이트가 정의되었다
- [ ] 모든 트레이트가 object-safe이고 Send + Sync이다
- [ ] PluginRegistry<T>가 등록/조회/중복방지를 지원한다
- [ ] CypherLiteError에 5개 플러그인 에러 변형이 추가되었다
- [ ] 3개 crate에 `plugin` feature flag가 설정되었다
- [ ] eval.rs에서 ScalarFunction 레지스트리 폴백이 동작한다
- [ ] 42개 내장 함수의 동작이 변경되지 않았다
- [ ] IndexManager에 IndexPlugin이 통합되었다
- [ ] export.rs에 Serializer 통합이 구현되었다
- [ ] Executor에 Trigger before/after 훅이 삽입되었다
- [ ] before 트리거 실패 시 변이 연산이 중단된다
- [ ] CypherLite API에 4개 register_* 메서드가 추가되었다
- [ ] plugin 비활성화 시 기존 1,256개 테스트 통과
- [ ] plugin 활성화 시 기존 + 신규 테스트 모두 통과
- [ ] 플러그인 코드 커버리지 85% 이상
- [ ] 성능 회귀 없음 (plugin 비활성화 시)
- [ ] 성능 오버헤드 5% 이내 (plugin 활성화, 미등록 시)
- [ ] Proptest 프로퍼티 기반 테스트 추가
- [ ] Criterion 벤치마크 추가
- [ ] 버전 범프 0.9.0 -> 1.0.0

---

## 검증 방법

### 자동 검증

1. **단위 테스트**: `cargo test --workspace --features plugin` -- Plugin trait, PluginRegistry 검증
2. **통합 테스트**: ScalarFunction/IndexPlugin/Serializer/Trigger 각각의 end-to-end 테스트
3. **프로퍼티 테스트**: `cargo test --workspace --features plugin` (proptest 포함)
4. **전체 테스트**: `cargo test --workspace --all-features` -- 기존 + 신규 모두
5. **기본 피처 테스트**: `cargo test --workspace` -- plugin 비활성화 시 기존 호환
6. **벤치마크**: `cargo bench --workspace --features plugin` -- 성능 검증
7. **커버리지**: `cargo llvm-cov --workspace --features plugin --fail-under-lines 85`

### 수동 검증

1. **Object Safety**: `Box<dyn ScalarFunction>` 변환 컴파일 확인
2. **Feature Flag**: `plugin` 비활성화 시 플러그인 코드 미포함 확인 (`cargo build` 사이즈 비교)
3. **API 사용성**: 샘플 코드로 플러그인 등록-쿼리 실행 시나리오 검증
