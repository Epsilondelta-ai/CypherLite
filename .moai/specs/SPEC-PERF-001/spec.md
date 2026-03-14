---
id: SPEC-PERF-001
title: Performance Optimization
version: 1.0.0
status: completed
created: 2026-03-15
updated: 2026-03-15
author: epsilondelta
priority: P1
tags: [performance, storage, query, benchmark]
modules: [storage-performance, query-performance, benchmark-infrastructure]
related_specs: [SPEC-DB-001, SPEC-DB-002, SPEC-DB-008]
---

# SPEC-PERF-001: Performance Optimization

## 1. Environment (환경)

### 1.1 시스템 컨텍스트

- **프로젝트**: CypherLite v1.0.0 - Rust 임베디드 단일 파일 그래프 데이터베이스
- **크레이트 구조**: cypherlite-core / cypherlite-storage / cypherlite-query
- **현재 테스트**: 1,309개 통과
- **개발 모드**: TDD (RED-GREEN-REFACTOR)
- **MSRV**: Rust 1.84+

### 1.2 성능 목표 (v1.0 제품 요구사항)

| 지표 | 목표값 | ID |
|------|--------|----|
| 단순 매치 쿼리 (p99) | < 10ms | PG-001 |
| 2홉 패턴 쿼리 (p99) | < 50ms | PG-002 |
| 바이너리 크기 | < 50MB | PG-003 |
| 메모리 (100만 노드) | < 500MB | PG-004 |
| 순차 쓰기 속도 | > 1,000 노드/초 | PG-005 |
| 동시 읽기 (4스레드) | > 50,000 읽기/초 | PG-006 |

### 1.3 현재 벤치마크 커버리지

- Storage: 8개 criterion 테스트 (storage_bench.rs, 194줄)
- Query: 15개 criterion 테스트 (query_bench.rs, 362줄)
- Feature-specific: 15개 (hypergraph, subgraph, temporal_edge, inline_filter)
- **부재**: 동시성 스트레스, 메모리 프로파일링, 스트리밍 벤치마크

### 1.4 식별된 핫 패스 병목

| 위치 | 문제 | 영향 |
|------|------|------|
| buffer_pool.rs:195 | LRU touch O(n) - VecDeque::retain per access | 캐시 히트마다 O(256) 스캔 |
| eval.rs:24 | format!() 힙 할당 per temporal property access | temporal 쿼리 성능 저하 |
| eval.rs:33-36 | AND/OR 양쪽 항상 평가 | WHERE 절 불필요한 연산 |
| expand.rs:57,82,88 | Record full clone per edge | 100노드x5엣지 = 500회 HashMap 복사 |
| Cargo.toml | crossbeam + dashmap 미사용 (-280KB) | 바이너리 크기 초과 위험 |

---

## 2. Assumptions (가정)

### 2.1 기술적 가정

- **A-001**: LRU O(1) 수정은 `LinkedHashMap` 또는 이중 연결 리스트 + HashMap 조합으로 구현 가능하다
  - 신뢰도: HIGH
  - 검증: Rust 생태계에 `lru` 크레이트 및 표준 패턴 존재

- **A-002**: crossbeam과 dashmap은 코드베이스 어디에서도 사용되지 않는다
  - 신뢰도: HIGH
  - 검증: research.md에서 grep 결과 미사용 확인됨

- **A-003**: Record에 Cow/Arc 적용 시 기존 테스트에 영향이 최소하다
  - 신뢰도: MEDIUM
  - 검증: expand.rs의 clone 패턴이 지역적이며, Record 타입의 공개 API는 변경 불필요

- **A-004**: AND/OR short-circuit 도입은 기존 쿼리 결과에 의미론적 변화를 주지 않는다
  - 신뢰도: HIGH
  - 검증: 부작용 없는 순수 표현식 평가이므로 단락 평가가 동치

### 2.2 제약 조건

- **C-001**: 공개 API 변경 불가 - v1.0.0 릴리즈 후 호환성 유지 필요
- **C-002**: 기존 1,309개 테스트 전부 통과 필수
- **C-003**: unsafe 코드 추가 최소화 - 기존 MVCC transmute 외 추가 unsafe 지양
- **C-004**: feature flag 변경 없음 - 기존 `hypergraph = ["subgraph"]` 유지

---

## 3. Requirements (요구사항)

### Module 1: Storage Performance (스토리지 성능)

#### REQ-S-001: LRU Touch O(1) 최적화

> **WHEN** BufferPool에서 캐시 히트가 발생하면, **THEN** 시스템은 O(1) 시간 복잡도로 LRU 순서를 갱신해야 한다.

- **현재 상태**: VecDeque::retain으로 O(n) 스캔 (buffer_pool.rs:195)
- **대상 파일**: `crates/cypherlite-storage/src/page/buffer_pool.rs`
- **관련 성능 게이트**: PG-001, PG-002, PG-005

#### REQ-S-002: 미사용 의존성 제거

> 시스템은 **항상** 실제로 사용되는 의존성만 포함해야 한다. 미사용 크레이트(crossbeam, dashmap)를 **제거해야 한다**.

- **대상 파일**: `crates/cypherlite-storage/Cargo.toml`, `crates/cypherlite-query/Cargo.toml`
- **예상 효과**: 바이너리 크기 -280KB
- **관련 성능 게이트**: PG-003

#### REQ-S-003: FSM 페이지 할당 힌트

> **WHEN** 새로운 페이지 할당이 요청되면, **THEN** 시스템은 in-memory FSM 비트맵과 next_free_page 힌트를 사용하여 선형 스캔을 회피해야 한다.

- **현재 상태**: page_manager.rs:106-144에서 매 할당마다 바이트 0부터 선형 스캔
- **대상 파일**: `crates/cypherlite-storage/src/page/page_manager.rs`
- **관련 성능 게이트**: PG-005

### Module 2: Query Performance (쿼리 성능)

#### REQ-Q-001: Temporal 표현식 힙 할당 제거

> **WHEN** temporal 프로퍼티에 접근할 때, **THEN** 시스템은 format!() 대신 사전 할당된 키 또는 Cow<str>를 사용하여 핫 루프 내 힙 할당을 제거해야 한다.

- **현재 상태**: `format!("__temporal_props__{}", var_name)` per access (eval.rs:24)
- **대상 파일**: `crates/cypherlite-query/src/executor/eval.rs`
- **관련 성능 게이트**: PG-001, PG-002

#### REQ-Q-002: AND/OR 단락 평가 (Short-Circuit Evaluation)

> **WHEN** AND 표현식을 평가할 때 왼쪽 항이 false이면, **THEN** 시스템은 오른쪽 항을 평가하지 않아야 한다. **WHEN** OR 표현식을 평가할 때 왼쪽 항이 true이면, **THEN** 시스템은 오른쪽 항을 평가하지 않아야 한다.

- **현재 상태**: 양쪽 항상 평가 (eval.rs:33-36)
- **대상 파일**: `crates/cypherlite-query/src/executor/eval.rs`
- **관련 성능 게이트**: PG-001, PG-002

#### REQ-Q-003: Expand 연산자 Record 공유

> **WHEN** Expand 연산자가 엣지를 순회하며 Record를 확장할 때, **THEN** 시스템은 Cow<Record> 또는 Arc<Record> 패턴을 사용하여 불필요한 전체 HashMap clone을 제거해야 한다.

- **현재 상태**: `record.clone()` per edge (expand.rs:57,82,88)
- **대상 파일**: `crates/cypherlite-query/src/executor/operators/expand.rs`
- **관련 성능 게이트**: PG-002, PG-004

### Module 3: Benchmark Infrastructure (벤치마크 인프라)

#### REQ-B-001: 동시성 벤치마크

> 시스템은 **항상** 다중 스레드(4스레드) 환경에서의 동시 읽기 처리량과 읽기-쓰기 경합 벤치마크를 포함해야 한다.

- **측정 항목**: 4스레드 동시 읽기 처리량 (reads/sec), write-lock 획득 지연
- **대상 파일**: 신규 `crates/cypherlite-storage/benches/concurrent_bench.rs`
- **관련 성능 게이트**: PG-006

#### REQ-B-002: 메모리 프로파일링 벤치마크

> 시스템은 **항상** 대규모 데이터셋(100만 노드)에서의 RSS/heap 메모리 사용량을 측정하는 벤치마크를 포함해야 한다.

- **측정 항목**: 1M 노드 로드 후 메모리 사용량 (MB), 노드당 메모리 비용
- **대상 파일**: 신규 `crates/cypherlite-storage/benches/memory_bench.rs`
- **관련 성능 게이트**: PG-004

#### REQ-B-003: 쿼리 스트리밍 벤치마크

> 시스템은 **항상** 대규모 결과 셋에 대한 쿼리 처리 시간과 메모리 사용량을 측정하는 벤치마크를 포함해야 한다.

- **측정 항목**: 10K/100K 결과 반환 시 처리 시간, 피크 메모리
- **대상 파일**: 신규 `crates/cypherlite-query/benches/streaming_bench.rs`
- **관련 성능 게이트**: PG-001, PG-002, PG-004

---

## 4. Specifications (세부 명세)

### 4.1 LRU 자료구조 교체 명세

**현재 구조**:
```
lru_order: VecDeque<PageId>  // retain + push_back per touch
```

**목표 구조**:
```
lru_map: HashMap<PageId, NonNull<LruNode>>  // O(1) lookup
lru_list: DoublyLinkedList<PageId>           // O(1) move-to-front
```

**대안**: `lru` 크레이트 (v0.12+) 또는 직접 구현
- 직접 구현 권장: 외부 의존성 최소화 원칙에 부합
- 기존 `BufferPool` 공개 API 유지: `get()`, `get_mut()`, `insert()`, `evict_lru()`

### 4.2 AND/OR 단락 평가 명세

**현재 코드 패턴** (eval.rs:33-36):
```
// AND: left와 right 모두 평가 후 결합
// OR: left와 right 모두 평가 후 결합
```

**목표 코드 패턴**:
```
// AND: left 평가 -> false이면 즉시 false 반환 -> 아니면 right 평가
// OR: left 평가 -> true이면 즉시 true 반환 -> 아니면 right 평가
```

### 4.3 Record 공유 명세

**현재 패턴**: `let mut new_record = record.clone();`
**목표 패턴**: `Cow<'_, Record>` 또는 `Arc<Record>` + copy-on-write

**선택 기준**:
- Cow: 단일 스레드 환경 (현재 executor 모델)에 적합, 오버헤드 최소
- Arc: 향후 병렬 executor 확장 고려 시 적합

### 4.4 미사용 의존성 목록

| 크레이트 | 위치 | 크기 영향 |
|---------|------|----------|
| crossbeam 0.8 | cypherlite-storage/Cargo.toml | ~200KB |
| dashmap 6 | cypherlite-storage/Cargo.toml | ~80KB |
| **합계** | | **~280KB 절감** |

### 4.5 FSM 힌트 명세

**현재 동작**: `allocate_page()` 호출 시 FSM 바이트 0부터 선형 스캔
**목표 동작**:
- In-memory `next_free_hint: PageId` 유지
- 할당 시 hint 위치부터 스캔 시작
- 해제 시 hint를 해제된 페이지로 갱신 (더 작은 ID이면)

---

## 5. Scope Boundary (범위 경계)

### 포함 (In-Scope)

- Tier 1 최적화: LRU O(1), 미사용 dep 제거, 동시성 벤치마크
- Tier 2 최적화: eval() 힙 할당, AND/OR 단락, Record Cow/Arc
- FSM 할당 힌트 (Tier 3에서 범위 내로 승격 - 쓰기 성능 목표 PG-005 달성에 필요)
- 벤치마크 인프라: 동시성, 메모리, 스트리밍

### 제외 (Out-of-Scope) - 향후 과제

- **스트리밍 쿼리 결과** (Tier 3): Vec<Record> -> Iterator 모델 전환 (20-30h, 아키텍처 변경)
- **비용 기반 옵티마이저 통계** (Tier 3): 행 카운트 통계 수집 및 비용 모델 (15-20h)
- **WAL 그룹 커밋/LZ4 압축** (Tier 3): 현재 PG-005 달성 가능성 HIGH로 보류
- **BTreeMap 전체 직렬화 최적화**: 선택적 필드 업데이트, delta 직렬화
- **Value enum 크기 최적화**: Box large variants, repr(u32) discriminant
- **bincode zero-copy 역직렬화**: Cow<[u8]> 기반 참조형 포맷

---

## 6. Traceability (추적성)

| 요구사항 | 성능 게이트 | 모듈 | 대상 파일 |
|---------|------------|------|----------|
| REQ-S-001 | PG-001, PG-002, PG-005 | Storage | buffer_pool.rs |
| REQ-S-002 | PG-003 | Storage | Cargo.toml (2개) |
| REQ-S-003 | PG-005 | Storage | page_manager.rs |
| REQ-Q-001 | PG-001, PG-002 | Query | eval.rs |
| REQ-Q-002 | PG-001, PG-002 | Query | eval.rs |
| REQ-Q-003 | PG-002, PG-004 | Query | expand.rs |
| REQ-B-001 | PG-006 | Benchmark | concurrent_bench.rs (신규) |
| REQ-B-002 | PG-004 | Benchmark | memory_bench.rs (신규) |
| REQ-B-003 | PG-001, PG-002, PG-004 | Benchmark | streaming_bench.rs (신규) |
