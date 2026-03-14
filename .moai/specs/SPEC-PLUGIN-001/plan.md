---
id: SPEC-PLUGIN-001
type: plan
version: "1.0.0"
status: draft
created: "2026-03-13"
updated: "2026-03-13"
author: epsilondelta
tags: [plugin, extensibility, trait-object, implementation-plan]
traceability:
  spec: spec.md
  acceptance: acceptance.md
  research: research.md
---

# SPEC-PLUGIN-001 구현 계획: Plugin System

## 1. 구현 단계

### Phase 10a: Core Plugin Infrastructure (Primary Goal)

**목표**: Plugin 기본 트레이트, PluginRegistry 제네릭 구조체, 에러 타입, feature flag 설정

**대상 파일**:

| File | Change Type | Description |
|------|------------|-------------|
| `crates/cypherlite-core/Cargo.toml` | 수정 | `plugin` feature flag 추가 |
| `crates/cypherlite-storage/Cargo.toml` | 수정 | `plugin` feature flag 추가 (core의 plugin 전파) |
| `crates/cypherlite-query/Cargo.toml` | 수정 | `plugin` feature flag 추가 (core + storage의 plugin 전파) |
| `crates/cypherlite-core/src/plugin/mod.rs` | 신규 | Plugin trait, ScalarFunction trait, IndexPlugin trait, Serializer trait, Trigger trait, TriggerContext, PluginRegistry<T> |
| `crates/cypherlite-core/src/lib.rs` | 수정 | `#[cfg(feature = "plugin")] pub mod plugin;` 추가 |
| `crates/cypherlite-core/src/error.rs` | 수정 | PluginError, FunctionNotFound, UnsupportedIndexType, UnsupportedFormat, TriggerError 변형 추가 |
| `crates/cypherlite-core/tests/plugin_test.rs` | 신규 | Plugin trait, PluginRegistry 단위 테스트 |

**작업 내용**:

1. **Feature Flag 설정**
   - 3개 crate의 `Cargo.toml`에 `plugin = []` feature 추가
   - storage: `plugin = ["cypherlite-core/plugin"]`
   - query: `plugin = ["cypherlite-core/plugin", "cypherlite-storage/plugin"]`

2. **Plugin 기본 트레이트 정의**
   - `Plugin` trait: `name()`, `version()`, `Send + Sync` 바운드
   - Object-safe 검증 (no `Self: Sized`)

3. **4가지 확장 트레이트 정의**
   - `ScalarFunction: Plugin` -- `call(&self, args: &[Value]) -> Result<Value>`
   - `IndexPlugin: Plugin` -- `index_type()`, `insert()`, `remove()`, `lookup()`
   - `Serializer: Plugin` -- `format()`, `export()`, `import()`
   - `Trigger: Plugin` -- `on_before_*()`, `on_after_*()` 6개 메서드

4. **TriggerContext 구조체 정의**
   - `EntityType` enum (Node, Edge)
   - `TriggerOperation` enum (Create, Update, Delete)
   - `TriggerContext` struct

5. **PluginRegistry<T> 구현**
   - `new()`, `register()`, `get()`, `get_mut()`, `list()`, `contains()`
   - 중복 이름 등록 시 `PluginError` 반환

6. **에러 타입 확장**
   - `CypherLiteError`에 5개 플러그인 에러 변형 추가
   - `#[cfg(feature = "plugin")]`으로 게이팅

7. **TDD: RED-GREEN-REFACTOR**
   - RED: PluginRegistry 등록/조회/중복 방지 테스트 작성
   - GREEN: 최소 구현
   - REFACTOR: 코드 정리

**검증 기준**:
- REQ-PLUGIN-001~005: Plugin trait 정의 및 Send+Sync 검증
- REQ-PLUGIN-050: 에러 타입 추가
- 기존 전체 테스트 통과 (plugin 피처 비활성화)
- plugin 피처 활성화 시 신규 테스트 통과

---

### Phase 10b: Query Function Plugin (Secondary Goal)

**목표**: ScalarFunction 레지스트리, eval.rs 리팩토링, CypherLite API 확장

**대상 파일**:

| File | Change Type | Description |
|------|------------|-------------|
| `crates/cypherlite-query/src/executor/eval.rs` | 수정 | 내장 함수 HashMap 추출, ScalarFunction 레지스트리 폴백 추가 |
| `crates/cypherlite-query/src/api/mod.rs` | 수정 | `register_scalar_function()` 메서드 추가, ScalarFunction 레지스트리 필드 추가 |
| `crates/cypherlite-query/tests/plugin_function_test.rs` | 신규 | ScalarFunction 등록 및 쿼리 실행 통합 테스트 |

**작업 내용**:

1. **eval.rs 리팩토링**
   - 기존 42개 내장 함수의 match dispatch를 `HashMap<&str, fn(&[Value]) -> Result<Value>>` 또는 유사한 정적 디스패치로 추출
   - `#[cfg(feature = "plugin")]` 블록에서 ScalarFunction 레지스트리 폴백 추가
   - `#[cfg(not(feature = "plugin"))]` 블록에서 기존 match 유지 (제로 오버헤드)

2. **CypherLite API 확장**
   - `CypherLite` 구조체에 `ScalarFunction` 레지스트리 필드 추가
   - `register_scalar_function(func: Box<dyn ScalarFunction>)` 메서드 추가
   - `list_functions()` 메서드 추가

3. **TDD: RED-GREEN-REFACTOR**
   - RED: 커스텀 함수 등록 후 Cypher 쿼리에서 호출하는 통합 테스트
   - GREEN: eval.rs 리팩토링 및 레지스트리 연동
   - REFACTOR: 중복 제거, 코드 정리

**검증 기준**:
- REQ-PLUGIN-010~013: ScalarFunction 등록, 디스패치, 에러 처리
- REQ-PLUGIN-060: API 확장
- 기존 42개 내장 함수 동작 불변
- 미등록 함수 호출 시 FunctionNotFound 에러

---

### Phase 10c: Index Plugin (Secondary Goal)

**목표**: IndexPlugin 레지스트리, IndexManager 통합

**대상 파일**:

| File | Change Type | Description |
|------|------------|-------------|
| `crates/cypherlite-storage/src/index/mod.rs` | 수정 | IndexPlugin 레지스트리 통합, 커스텀 인덱스 타입 지원 |
| `crates/cypherlite-query/src/api/mod.rs` | 수정 | `register_index_plugin()` 메서드 추가 |
| `crates/cypherlite-storage/tests/plugin_index_test.rs` | 신규 | IndexPlugin 등록 및 인덱스 조작 테스트 |

**작업 내용**:

1. **IndexManager 확장**
   - `IndexManager`에 `PluginRegistry<dyn IndexPlugin>` 필드 추가
   - 인덱스 생성 시 내장 타입 확인 -> IndexPlugin 레지스트리 폴백
   - `#[cfg(feature = "plugin")]`으로 게이팅

2. **CypherLite API 확장**
   - `register_index_plugin(plugin: Box<dyn IndexPlugin>)` 메서드 추가

3. **TDD: RED-GREEN-REFACTOR**
   - RED: 커스텀 인덱스 플러그인 등록, insert/lookup 테스트
   - GREEN: IndexManager 통합
   - REFACTOR: 기존 IndexManager 코드와의 일관성 확보

**검증 기준**:
- REQ-PLUGIN-020~022: IndexPlugin 등록, 통합, 에러 처리
- 기존 PropertyIndex 동작 불변
- 미지원 인덱스 타입 시 UnsupportedIndexType 에러

---

### Phase 10d: Serializer Plugin (Final Goal)

**목표**: Serializer 레지스트리, import/export API

**대상 파일**:

| File | Change Type | Description |
|------|------------|-------------|
| `crates/cypherlite-query/src/api/export.rs` | 신규 | Serializer 레지스트리 기반 export/import API |
| `crates/cypherlite-query/src/api/mod.rs` | 수정 | `register_serializer()`, `export()`, `import()` 메서드 추가 |
| `crates/cypherlite-query/tests/plugin_serializer_test.rs` | 신규 | Serializer 등록 및 export/import 테스트 |

**작업 내용**:

1. **export.rs 모듈 생성**
   - `ExportManager` 구조체 (Serializer 레지스트리 래퍼)
   - `export(format: &str, data: &[Row]) -> Result<Vec<u8>>` 메서드
   - `import(format: &str, bytes: &[u8]) -> Result<Vec<Row>>` 메서드

2. **CypherLite API 확장**
   - `register_serializer(serializer: Box<dyn Serializer>)` 메서드
   - `export(format: &str, query: &str) -> Result<Vec<u8>>` 메서드
   - `import(format: &str, bytes: &[u8]) -> Result<()>` 메서드

3. **TDD: RED-GREEN-REFACTOR**
   - RED: 커스텀 JSON Serializer 등록, export/import 라운드트립 테스트
   - GREEN: ExportManager 구현
   - REFACTOR: Row 변환 로직 최적화

**검증 기준**:
- REQ-PLUGIN-030~032: Serializer 등록, 포맷 검색, 에러 처리
- export -> import 라운드트립 데이터 무결성
- 미지원 포맷 시 UnsupportedFormat 에러

---

### Phase 10e: Trigger Plugin + Quality (Optional Goal)

**목표**: Trigger 레지스트리, Executor 훅 통합, 프로퍼티 테스트, 벤치마크, 버전 범프

**대상 파일**:

| File | Change Type | Description |
|------|------------|-------------|
| `crates/cypherlite-query/src/executor/mod.rs` | 수정 | CREATE/SET/DELETE 연산에 Trigger before/after 훅 삽입 |
| `crates/cypherlite-query/src/api/mod.rs` | 수정 | `register_trigger()` 메서드 추가 |
| `crates/cypherlite-query/tests/plugin_trigger_test.rs` | 신규 | Trigger 등록, before/after 호출, 롤백 테스트 |
| `crates/cypherlite-query/tests/plugin_proptest.rs` | 신규 | Proptest: 랜덤 플러그인 등록/호출 |
| `crates/cypherlite-query/benches/plugin_bench.rs` | 신규 | 플러그인 유/무 성능 비교 벤치마크 |
| `Cargo.toml` (all crates) | 수정 | 버전 범프 0.9.0 -> 1.0.0 |

**작업 내용**:

1. **Executor 훅 통합**
   - CREATE 연산: `on_before_create()` -> 노드/엣지 생성 -> `on_after_create()`
   - SET 연산: `on_before_update()` -> 프로퍼티 설정 -> `on_after_update()`
   - DELETE 연산: `on_before_delete()` -> 노드/엣지 삭제 -> `on_after_delete()`
   - before 훅 실패 시 연산 중단 및 에러 전파

2. **CypherLite API 확장**
   - `register_trigger(trigger: Box<dyn Trigger>)` 메서드

3. **Quality 작업**
   - Proptest: 랜덤 함수 이름/인자 생성 후 ScalarFunction 호출
   - Proptest: 랜덤 프로퍼티 생성 후 Trigger 호출
   - Criterion 벤치마크: 내장 함수 호출 (plugin 유/무 비교)
   - Criterion 벤치마크: 트리거 유/무 CREATE 성능 비교

4. **버전 범프**
   - 3개 crate 모두 0.9.0 -> 1.0.0

5. **TDD: RED-GREEN-REFACTOR**
   - RED: Trigger 등록, CREATE 시 before/after 호출 확인, before 실패 시 롤백
   - GREEN: Executor 훅 구현
   - REFACTOR: 훅 호출 로직 추출, 중복 제거

**검증 기준**:
- REQ-PLUGIN-040~043: Trigger 등록, 훅 호출, 롤백
- REQ-PLUGIN-070~073: 기존 테스트 호환, 커버리지 85%+, 성능 회귀 없음
- 버전 1.0.0 릴리스 준비 완료

---

## 2. 기술적 접근 방식

### 2.1 트레이트 설계 원칙

**Object Safety 준수**:
- 모든 플러그인 트레이트에 `Self: Sized` 제약 없음
- `Box<dyn ScalarFunction>`, `Box<dyn IndexPlugin>` 등 동적 디스패치 가능
- 기존 `Box<dyn TransactionView>`, `&mut dyn LabelRegistry` 패턴과 일관

**Send + Sync 바운드**:
- `parking_lot::RwLock`으로 보호되는 공유 상태에서 안전하게 사용
- 기존 `dashmap` 사용 패턴과 호환

### 2.2 레지스트리 패턴

**기존 패턴 활용**: `IndexManager`의 `HashMap<String, (IndexDefinition, PropertyIndex)>` 레지스트리 패턴을 제네릭화하여 `PluginRegistry<T>`로 추상화.

```
IndexManager (기존)           PluginRegistry<T> (신규)
HashMap<String, (Def, Idx)>   HashMap<String, Box<T>>
get(name) -> Option<&>        get(name) -> Option<&T>
insert(name, value)           register(plugin: Box<T>)
```

### 2.3 eval.rs 리팩토링 전략

**점진적 리팩토링**:
1. Phase 10b에서 `#[cfg(feature = "plugin")]` 블록 추가
2. plugin 비활성화 시 기존 match 유지 (제로 오버헤드)
3. plugin 활성화 시 내장 함수 HashMap + 레지스트리 폴백

**대안 고려**:
- A: 전체 match를 HashMap으로 교체 -- 비 plugin 사용자에게 미세한 성능 영향
- B: cfg 분기로 양쪽 유지 -- 코드 중복이지만 제로 오버헤드 (채택)
- C: 매크로로 디스패치 생성 -- 복잡성 증가, 디버깅 어려움

### 2.4 Trigger 통합 전략

**Executor 수정 최소화**:
- CREATE/SET/DELETE 각 연산 시작/종료 지점에 훅 호출 삽입
- `#[cfg(feature = "plugin")]`으로 게이팅하여 비활성화 시 제로 오버헤드
- before 훅 실패 시 조기 반환 (`?` 연산자)

---

## 3. Phase 의존 관계

```
Phase 10a (Core Infrastructure)
    |
    +---> Phase 10b (Query Function) -- Plugin trait, PluginRegistry에 의존
    |
    +---> Phase 10c (Index Plugin) -- Plugin trait, PluginRegistry에 의존
    |
    +---> Phase 10d (Serializer Plugin) -- Plugin trait, PluginRegistry에 의존
    |
    +---> Phase 10e (Trigger + Quality) -- 모든 Phase에 의존
```

Phase 10b, 10c, 10d는 10a 완료 후 독립적으로 진행 가능하다. Phase 10e는 모든 기능 구현 후 수행한다.

---

## 4. 생성/수정 파일 요약

| File | Phase | Action | Description |
|------|-------|--------|-------------|
| `crates/cypherlite-core/Cargo.toml` | 10a | 수정 | plugin feature flag |
| `crates/cypherlite-storage/Cargo.toml` | 10a | 수정 | plugin feature flag 전파 |
| `crates/cypherlite-query/Cargo.toml` | 10a | 수정 | plugin feature flag 전파 |
| `crates/cypherlite-core/src/lib.rs` | 10a | 수정 | plugin 모듈 선언 |
| `crates/cypherlite-core/src/plugin/mod.rs` | 10a | 신규 | 모든 트레이트, Registry, Context |
| `crates/cypherlite-core/src/error.rs` | 10a | 수정 | 5개 에러 변형 추가 |
| `crates/cypherlite-query/src/executor/eval.rs` | 10b | 수정 | 함수 디스패치 리팩토링 |
| `crates/cypherlite-query/src/api/mod.rs` | 10b~10e | 수정 | register_* API 메서드 |
| `crates/cypherlite-storage/src/index/mod.rs` | 10c | 수정 | IndexPlugin 통합 |
| `crates/cypherlite-query/src/api/export.rs` | 10d | 신규 | Serializer 통합 모듈 |
| `crates/cypherlite-query/src/executor/mod.rs` | 10e | 수정 | Trigger 훅 삽입 |
| `crates/cypherlite-core/tests/plugin_test.rs` | 10a | 신규 | Core 플러그인 단위 테스트 |
| `crates/cypherlite-query/tests/plugin_function_test.rs` | 10b | 신규 | ScalarFunction 통합 테스트 |
| `crates/cypherlite-storage/tests/plugin_index_test.rs` | 10c | 신규 | IndexPlugin 통합 테스트 |
| `crates/cypherlite-query/tests/plugin_serializer_test.rs` | 10d | 신규 | Serializer 통합 테스트 |
| `crates/cypherlite-query/tests/plugin_trigger_test.rs` | 10e | 신규 | Trigger 통합 테스트 |
| `crates/cypherlite-query/tests/plugin_proptest.rs` | 10e | 신규 | Proptest 프로퍼티 기반 테스트 |
| `crates/cypherlite-query/benches/plugin_bench.rs` | 10e | 신규 | Criterion 벤치마크 |
| `Cargo.toml` (all crates) | 10e | 수정 | 버전 범프 0.9.0 -> 1.0.0 |

---

## 5. 리스크 분석

### R1: eval.rs 리팩토링 복잡성

- **위험**: 42개 내장 함수의 match dispatch 리팩토링 시 기존 동작 변경
- **확률**: 중간
- **대응**: `#[cfg]` 분기로 기존 코드 유지, plugin 비활성화 시 원본 match 보존. 기존 테스트가 회귀 감지

### R2: Object Safety 위반

- **위험**: 트레이트 설계 시 object-safe 위반으로 `Box<dyn T>` 사용 불가
- **확률**: 낮음
- **대응**: 설계 단계에서 `Self: Sized` 제약 제거, `dyn Trait` 컴파일 테스트 추가

### R3: Executor Trigger 훅 성능

- **위험**: 모든 변이 연산마다 트리거 레지스트리 조회 오버헤드
- **확률**: 낮음
- **대응**: `#[cfg]` 게이팅으로 비활성화 시 제로 오버헤드. 등록된 트리거 없으면 조기 반환(early return)

### R4: IndexPlugin과 기존 IndexManager 충돌

- **위험**: 커스텀 인덱스가 기존 PropertyIndex와 충돌
- **확률**: 낮음
- **대응**: 내장 타입 우선 검색, IndexPlugin은 폴백으로만 사용. 타입 이름 충돌 시 에러 반환

### R5: Feature Flag 조합 복잡성

- **위험**: plugin + temporal chain 등 복수 피처 플래그 조합에서 컴파일 문제
- **확률**: 중간
- **대응**: CI에서 `--all-features` 테스트로 모든 조합 검증. plugin은 temporal chain과 독립

---

## 6. 전문가 상담 권장

### Backend 전문가 (expert-backend)

이 SPEC은 Rust 트레이트 설계, 동적 디스패치, 레지스트리 패턴에 관한 것으로, 구현 시 expert-backend 에이전트 상담을 권장한다:

- 트레이트 object safety 검증
- PluginRegistry 제네릭 설계 리뷰
- eval.rs 리팩토링 전략 검토
- Trigger 훅 통합 아키텍처 리뷰

---

## 7. 추적성

| Phase | 요구사항 | 수락 기준 |
|-------|----------|-----------|
| 10a | REQ-PLUGIN-001~005, REQ-PLUGIN-050 | AC-001~003, AC-014 |
| 10b | REQ-PLUGIN-010~013, REQ-PLUGIN-060~061 | AC-004~006, AC-015 |
| 10c | REQ-PLUGIN-020~022 | AC-007~008 |
| 10d | REQ-PLUGIN-030~032 | AC-009~010 |
| 10e | REQ-PLUGIN-040~043, REQ-PLUGIN-070~073 | AC-011~013, AC-016~018 |
