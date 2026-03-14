---
id: SPEC-PLUGIN-001
title: "Plugin System for CypherLite"
version: "1.0.0"
status: draft
created: "2026-03-13"
updated: "2026-03-13"
author: epsilondelta
priority: P1
tags: [plugin, extensibility, trait-object, query-function, index, serializer, trigger]
lifecycle: spec-anchored
depends_on: []
traceability:
  plan: plan.md
  acceptance: acceptance.md
  research: research.md
---

# SPEC-PLUGIN-001: CypherLite Plugin System (v1.0)

> CypherLite에 트레이트 기반 플러그인 시스템을 도입하여 사용자 정의 쿼리 함수, 인덱스, 직렬화 포맷, 트리거를 등록할 수 있게 한다. 기존 코드베이스의 `dyn LabelRegistry`, `IndexManager` HashMap 레지스트리, `match func_name` 디스패치 등 확립된 패턴을 활용하여 최소한의 침습으로 확장성을 확보한다.

---

## HISTORY

| Version | Date | Changes |
|---------|------|---------|
| 1.0.0 | 2026-03-13 | 딥 리서치 기반 초기 SPEC 작성. 4가지 플러그인 타입 정의 |

---

## 1. 환경 (Environment)

### 1.1 Rust Version

- Rust 1.84+ (MSRV), 2021 edition
- CypherLite v0.9.0 -> v1.0.0

### 1.2 Crate 구조

| Crate | 역할 | 플러그인 관련 변경 |
|-------|------|-------------------|
| `cypherlite-core` | 타입, 트레이트, 설정, 에러 | Plugin 기본 트레이트 정의, ScalarFunction/IndexPlugin/Serializer/Trigger 트레이트 정의 |
| `cypherlite-storage` | 스토리지 엔진, 인덱스, 트랜잭션 | IndexPlugin 통합, Trigger 훅 포인트 |
| `cypherlite-query` | 렉서, 파서, 플래너, 실행기, API | ScalarFunction 레지스트리, eval.rs 리팩토링, Serializer 통합, PluginRegistry 노출 |

### 1.3 Feature Flag

```toml
[features]
plugin = []  # 기본 플러그인 인프라 (temporal chain과 독립)
```

`plugin` 피처는 기존 `temporal-core -> temporal-edge -> subgraph -> hypergraph -> full-temporal` 체인과 독립적으로 동작한다. 플러그인은 직교적(orthogonal) 관심사이다.

### 1.4 의존성

신규 외부 의존성 추가 없음. 기존 의존성 활용:
- `serde` -- 플러그인 설정 직렬화
- `parking_lot` -- 스레드 안전 레지스트리 잠금
- `dashmap` -- 동시 플러그인 접근

---

## 2. 가정 (Assumptions)

- A1: 플러그인은 동기(synchronous) 전용이다. CypherLite에 async/await가 없으므로 비동기 플러그인 인터페이스는 불필요하다.
- A2: 플러그인은 런타임에 Rust 코드로 등록된다 (동적 라이브러리 로딩 없음). 컴파일 타임에 정적으로 링크된다.
- A3: 기존 `IndexManager`의 `HashMap<String, (IndexDefinition, PropertyIndex)>` 레지스트리 패턴이 플러그인 레지스트리의 설계 기반으로 적합하다.
- A4: Object-safe 트레이트만 사용하여 `Box<dyn Plugin>` 형태의 동적 디스패치를 지원한다.
- A5: 플러그인 시스템은 기존 테스트(1,256개)에 영향을 주지 않는다 (feature-gated).
- A6: 사용자는 4가지 타입의 플러그인(Query Function, Index, Serializer, Trigger)으로 대부분의 확장 요구를 충족할 수 있다.

---

## 3. 요구사항 (Requirements) -- EARS Format

### Group AA: Core Plugin Infrastructure

#### REQ-PLUGIN-001: Plugin 기본 트레이트 (Ubiquitous)

시스템은 **항상** 모든 플러그인이 구현해야 하는 기본 `Plugin` 트레이트를 제공해야 한다. 이 트레이트는 `name()`, `version()` 메서드를 포함하며, `Send + Sync`이고 object-safe여야 한다.

```rust
pub trait Plugin: Send + Sync {
    fn name(&self) -> &str;
    fn version(&self) -> &str;
}
```

#### REQ-PLUGIN-002: PluginRegistry 제네릭 레지스트리 (Ubiquitous)

시스템은 **항상** `PluginRegistry<T>` 제네릭 구조체를 통해 플러그인을 이름 기반으로 등록하고 조회할 수 있어야 한다. 레지스트리는 `HashMap<String, Box<T>>`를 내부 저장소로 사용한다.

#### REQ-PLUGIN-003: 중복 등록 방지 (Unwanted)

시스템은 동일한 이름의 플러그인을 중복 등록하는 것을 **허용하지 않아야 한다**. 중복 시 `CypherLiteError::PluginError`를 반환해야 한다.

#### REQ-PLUGIN-004: Feature Flag 격리 (State-Driven)

**IF** `plugin` 피처 플래그가 비활성화 상태이면, **THEN** 플러그인 관련 모든 코드가 컴파일에서 제외되어야 하며, 기존 동작에 영향이 없어야 한다.

#### REQ-PLUGIN-005: Thread Safety (Ubiquitous)

시스템은 **항상** 모든 플러그인 트레이트와 레지스트리가 `Send + Sync` 바운드를 충족하여 멀티스레드 환경에서 안전하게 사용될 수 있어야 한다.

### Group BB: Query Function Plugin

#### REQ-PLUGIN-010: ScalarFunction 트레이트 (Ubiquitous)

시스템은 **항상** 사용자 정의 스칼라 함수를 등록할 수 있는 `ScalarFunction` 트레이트를 제공해야 한다. 이 트레이트는 `call(&self, args: &[Value]) -> Result<Value, CypherLiteError>` 메서드를 포함한다.

```rust
pub trait ScalarFunction: Plugin {
    fn call(&self, args: &[Value]) -> Result<Value, CypherLiteError>;
}
```

#### REQ-PLUGIN-011: 함수 레지스트리 통합 (Event-Driven)

**WHEN** 쿼리 실행 중 함수 호출(`RETURN myFunc(n.name)`)을 만나면, **THEN** 시스템은 먼저 내장 함수 목록에서 검색하고, 없으면 ScalarFunction 레지스트리에서 검색해야 한다.

#### REQ-PLUGIN-012: eval.rs 디스패치 리팩토링 (Event-Driven)

**WHEN** `plugin` 피처가 활성화되어 있으면, **THEN** `eval.rs`의 `match func_name.as_str()` 하드코딩 디스패치를 내장 함수 HashMap + ScalarFunction 레지스트리 폴백 구조로 리팩토링해야 한다.

#### REQ-PLUGIN-013: 미등록 함수 에러 (Event-Driven)

**WHEN** 내장 함수 목록과 ScalarFunction 레지스트리 모두에서 함수를 찾을 수 없으면, **THEN** 시스템은 `CypherLiteError::FunctionNotFound` 에러를 반환해야 한다.

### Group CC: Index Plugin

#### REQ-PLUGIN-020: IndexPlugin 트레이트 (Ubiquitous)

시스템은 **항상** 사용자 정의 인덱스 구현을 등록할 수 있는 `IndexPlugin` 트레이트를 제공해야 한다. 이 트레이트는 `index_type()`, `insert()`, `remove()`, `lookup()` 메서드를 포함한다.

```rust
pub trait IndexPlugin: Plugin {
    fn index_type(&self) -> &str;
    fn insert(&mut self, key: &PropertyValue, node_id: NodeId) -> Result<(), CypherLiteError>;
    fn remove(&mut self, key: &PropertyValue, node_id: NodeId) -> Result<(), CypherLiteError>;
    fn lookup(&self, key: &PropertyValue) -> Result<Vec<NodeId>, CypherLiteError>;
}
```

#### REQ-PLUGIN-021: IndexManager 통합 (Event-Driven)

**WHEN** 사용자가 커스텀 인덱스 타입으로 인덱스 생성을 요청하면, **THEN** `IndexManager`는 IndexPlugin 레지스트리에서 해당 타입의 플러그인을 찾아 인덱스를 생성해야 한다.

#### REQ-PLUGIN-022: 미등록 인덱스 타입 에러 (Event-Driven)

**WHEN** 요청된 인덱스 타입이 내장 타입도 아니고 IndexPlugin 레지스트리에도 없으면, **THEN** 시스템은 `CypherLiteError::UnsupportedIndexType` 에러를 반환해야 한다.

### Group DD: Serializer Plugin

#### REQ-PLUGIN-030: Serializer 트레이트 (Ubiquitous)

시스템은 **항상** 사용자 정의 가져오기/내보내기 형식을 등록할 수 있는 `Serializer` 트레이트를 제공해야 한다. 이 트레이트는 `format()`, `export()`, `import()` 메서드를 포함한다.

```rust
pub trait Serializer: Plugin {
    fn format(&self) -> &str;
    fn export(&self, data: &[Row]) -> Result<Vec<u8>, CypherLiteError>;
    fn import(&self, bytes: &[u8]) -> Result<Vec<Row>, CypherLiteError>;
}
```

#### REQ-PLUGIN-031: 포맷 기반 직렬화기 검색 (Event-Driven)

**WHEN** 사용자가 특정 포맷(예: "json", "csv", "graphml")으로 데이터 내보내기/가져오기를 요청하면, **THEN** 시스템은 Serializer 레지스트리에서 해당 포맷의 플러그인을 찾아 사용해야 한다.

#### REQ-PLUGIN-032: 미지원 포맷 에러 (Event-Driven)

**WHEN** 요청된 포맷이 Serializer 레지스트리에 없으면, **THEN** 시스템은 `CypherLiteError::UnsupportedFormat` 에러를 반환해야 한다.

### Group EE: Trigger Plugin

#### REQ-PLUGIN-040: Trigger 트레이트 (Ubiquitous)

시스템은 **항상** 변이 연산 전후에 실행되는 트리거를 등록할 수 있는 `Trigger` 트레이트를 제공해야 한다. 이 트레이트는 `on_before_create()`, `on_after_create()`, `on_before_update()`, `on_after_update()`, `on_before_delete()`, `on_after_delete()` 메서드를 포함한다.

```rust
pub trait Trigger: Plugin {
    fn on_before_create(&self, entity: &TriggerContext) -> Result<(), CypherLiteError>;
    fn on_after_create(&self, entity: &TriggerContext) -> Result<(), CypherLiteError>;
    fn on_before_update(&self, entity: &TriggerContext) -> Result<(), CypherLiteError>;
    fn on_after_update(&self, entity: &TriggerContext) -> Result<(), CypherLiteError>;
    fn on_before_delete(&self, entity: &TriggerContext) -> Result<(), CypherLiteError>;
    fn on_after_delete(&self, entity: &TriggerContext) -> Result<(), CypherLiteError>;
}
```

#### REQ-PLUGIN-041: Executor 훅 통합 (Event-Driven)

**WHEN** CREATE, SET, 또는 DELETE 연산이 실행되면, **THEN** 시스템은 등록된 모든 Trigger 플러그인의 해당 before/after 훅을 순서대로 호출해야 한다.

#### REQ-PLUGIN-042: Trigger 실패 시 롤백 (Event-Driven + Unwanted)

**WHEN** `on_before_*` 트리거가 에러를 반환하면, **THEN** 시스템은 해당 변이 연산을 실행하지 않아야 하며, 에러를 호출자에게 전파해야 한다. 시스템은 before 트리거 실패 후 변이 연산을 실행하는 것을 **허용하지 않아야 한다**.

#### REQ-PLUGIN-043: TriggerContext 구조체 (Ubiquitous)

시스템은 **항상** 트리거에 전달되는 컨텍스트 정보를 담는 `TriggerContext` 구조체를 제공해야 한다. 이 구조체는 엔티티 타입(노드/엣지), 엔티티 ID, 레이블/타입, 프로퍼티를 포함한다.

### Group FF: Error Handling

#### REQ-PLUGIN-050: 플러그인 에러 타입 (Ubiquitous)

시스템은 **항상** `CypherLiteError`에 플러그인 관련 에러 변형을 추가해야 한다:
- `PluginError(String)` -- 일반 플러그인 에러
- `FunctionNotFound(String)` -- 미등록 함수
- `UnsupportedIndexType(String)` -- 미지원 인덱스 타입
- `UnsupportedFormat(String)` -- 미지원 직렬화 포맷
- `TriggerError(String)` -- 트리거 실행 에러

#### REQ-PLUGIN-051: 플러그인 에러 전파 (Event-Driven)

**WHEN** 플러그인 실행 중 에러가 발생하면, **THEN** 시스템은 에러를 `CypherLiteError`로 래핑하여 호출자에게 전파해야 한다. 플러그인 에러가 조용히 무시되어서는 안 된다.

### Group GG: Public API

#### REQ-PLUGIN-060: CypherLite API 확장 (Event-Driven)

**WHEN** `plugin` 피처가 활성화되어 있으면, **THEN** `CypherLite` 퍼사드 구조체에 플러그인 등록 메서드가 추가되어야 한다:
- `register_scalar_function(func: Box<dyn ScalarFunction>)`
- `register_index_plugin(plugin: Box<dyn IndexPlugin>)`
- `register_serializer(serializer: Box<dyn Serializer>)`
- `register_trigger(trigger: Box<dyn Trigger>)`

#### REQ-PLUGIN-061: 등록된 플러그인 조회 (Event-Driven)

**WHEN** 사용자가 등록된 플러그인 목록을 요청하면, **THEN** 시스템은 각 레지스트리별 등록된 플러그인의 이름과 버전 목록을 반환해야 한다.

### Group HH: Quality

#### REQ-PLUGIN-070: 기존 테스트 호환 (Ubiquitous)

시스템은 **항상** 기존의 모든 테스트(1,256개)가 `plugin` 피처 활성화/비활성화 모두에서 통과해야 한다.

#### REQ-PLUGIN-071: 커버리지 목표 (State-Driven)

**IF** 플러그인 관련 코드가 추가되면, **THEN** 해당 코드의 테스트 커버리지가 85% 이상이어야 한다.

#### REQ-PLUGIN-072: 성능 회귀 방지 (Unwanted)

시스템은 `plugin` 피처가 비활성화된 상태에서 기존 쿼리 실행 성능이 회귀되는 것을 **허용하지 않아야 한다**.

#### REQ-PLUGIN-073: 성능 오버헤드 제한 (State-Driven)

**IF** `plugin` 피처가 활성화된 상태이면, **THEN** 플러그인이 등록되지 않은 경우 기존 내장 함수 호출의 성능 오버헤드가 5% 이내여야 한다.

---

## 4. Non-Goals

- 동적 라이브러리(`.so`/`.dll`) 기반 런타임 플러그인 로딩
- 비동기(async) 플러그인 인터페이스
- Storage Backend 플러그인 (StorageEngine 결합도가 높아 v2.0으로 연기)
- Event/Lifecycle 플러그인 (크로스커팅 이벤트 버스가 필요하여 v2.0으로 연기)
- 플러그인 의존성 관리 (플러그인 간 의존성 선언)
- 플러그인 핫 리로드/언로드
- 쿼리 언어 수준의 `CALL` 프로시저 지원

---

## 5. 명세 (Specifications)

### 5.1 Core Plugin 아키텍처

```
cypherlite-core (트레이트 정의)
  |
  +-- plugin/mod.rs
  |     +-- Plugin trait (base)
  |     +-- ScalarFunction trait
  |     +-- IndexPlugin trait
  |     +-- Serializer trait
  |     +-- Trigger trait
  |     +-- TriggerContext struct
  |     +-- PluginRegistry<T> struct
  |
  +-- error.rs (에러 변형 추가)

cypherlite-storage (인덱스/트리거 통합)
  |
  +-- index/mod.rs (IndexPlugin 통합)
  +-- (executor 훅을 위한 트리거 포인트)

cypherlite-query (함수/직렬화/API 통합)
  |
  +-- executor/eval.rs (ScalarFunction 레지스트리 폴백)
  +-- api/mod.rs (register_* 메서드)
  +-- api/export.rs (Serializer 통합, 신규)
```

### 5.2 PluginRegistry 설계

```rust
#[cfg(feature = "plugin")]
pub struct PluginRegistry<T: Plugin + ?Sized> {
    plugins: HashMap<String, Box<T>>,
}

impl<T: Plugin + ?Sized> PluginRegistry<T> {
    pub fn new() -> Self { ... }
    pub fn register(&mut self, plugin: Box<T>) -> Result<(), CypherLiteError> { ... }
    pub fn get(&self, name: &str) -> Option<&T> { ... }
    pub fn get_mut(&mut self, name: &str) -> Option<&mut T> { ... }
    pub fn list(&self) -> Vec<(&str, &str)> { ... }  // (name, version) pairs
    pub fn contains(&self, name: &str) -> bool { ... }
}
```

### 5.3 eval.rs 리팩토링 전략

```
현재 (하드코딩 dispatch):
  match func_name.as_str() {
      "id" => { ... }
      "type" => { ... }
      "labels" => { ... }
      // ... 42개 내장 함수
      _ => Err(UnsupportedFunction)
  }

목표 (#[cfg(feature = "plugin")]):
  // 1. 내장 함수 HashMap에서 검색
  if let Some(builtin) = BUILTINS.get(func_name) {
      return builtin(args);
  }
  // 2. ScalarFunction 레지스트리에서 검색
  if let Some(func) = registry.get(func_name) {
      return func.call(args);
  }
  // 3. 함수 없음
  Err(FunctionNotFound(func_name))

목표 (#[cfg(not(feature = "plugin"))]):
  // 기존 match 디스패치 유지 (제로 오버헤드)
```

### 5.4 TriggerContext 설계

```rust
#[cfg(feature = "plugin")]
pub struct TriggerContext {
    pub entity_type: EntityType,  // Node or Edge
    pub entity_id: u64,
    pub label_or_type: Option<String>,
    pub properties: HashMap<String, PropertyValue>,
    pub operation: TriggerOperation,  // Create, Update, Delete
}

#[cfg(feature = "plugin")]
pub enum EntityType {
    Node,
    Edge,
}

#[cfg(feature = "plugin")]
pub enum TriggerOperation {
    Create,
    Update,
    Delete,
}
```

---

## 6. 제약사항 (Constraints)

- C1: CypherLite는 동기(synchronous)만 지원 -- 플러그인도 동기여야 한다
- C2: 단일 파일 데이터베이스 -- 플러그인이 파일 포맷을 변경할 수 없다 (헤더 버전 범프 없이)
- C3: MSRV 1.84 -- Rust 1.84 이후 기능만 사용 가능
- C4: Object Safety 필수 -- `Self: Sized` 제약 없이 `dyn Trait` 사용 가능해야 한다
- C5: Thread Safety 필수 -- `Send + Sync` 바운드 (parking_lot/dashmap 사용)
- C6: Feature-gated -- `#[cfg(feature = "plugin")]`으로 모든 플러그인 코드를 격리
- C7: 기존 42개 내장 함수의 동작을 변경하지 않아야 한다

---

## 7. 추적성 (Traceability)

| 요구사항 | Phase | 구현 파일 | 수락 기준 |
|----------|-------|-----------|-----------|
| REQ-PLUGIN-001~005 | 10a | cypherlite-core/src/plugin/mod.rs | AC-001~003 |
| REQ-PLUGIN-010~013 | 10b | cypherlite-query/src/executor/eval.rs, api/mod.rs | AC-004~006 |
| REQ-PLUGIN-020~022 | 10c | cypherlite-storage/src/index/mod.rs | AC-007~008 |
| REQ-PLUGIN-030~032 | 10d | cypherlite-query/src/api/export.rs | AC-009~010 |
| REQ-PLUGIN-040~043 | 10e | cypherlite-query/src/executor/mod.rs | AC-011~013 |
| REQ-PLUGIN-050~051 | 10a | cypherlite-core/src/error.rs | AC-014 |
| REQ-PLUGIN-060~061 | 10b | cypherlite-query/src/api/mod.rs | AC-015 |
| REQ-PLUGIN-070~073 | 10e | tests/, benches/ | AC-016~018 |
