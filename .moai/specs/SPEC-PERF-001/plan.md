---
id: SPEC-PERF-001
document: plan
version: 1.0.0
status: approved
created: 2026-03-15
updated: 2026-03-15
author: epsilondelta
---

# SPEC-PERF-001: Implementation Plan (구현 계획)

## 1. 마일스톤 개요

| 우선순위 | 마일스톤 | 모듈 | 요구사항 | 성능 게이트 |
|---------|---------|------|---------|------------|
| Primary Goal | M1: Storage Quick Wins | Storage | REQ-S-001, REQ-S-002 | PG-001~003, PG-005 |
| Primary Goal | M2: Query Quick Wins | Query | REQ-Q-001, REQ-Q-002 | PG-001, PG-002 |
| Secondary Goal | M3: Record Sharing | Query | REQ-Q-003 | PG-002, PG-004 |
| Secondary Goal | M4: FSM Hints | Storage | REQ-S-003 | PG-005 |
| Final Goal | M5: Benchmark Infrastructure | Benchmark | REQ-B-001~003 | PG-004, PG-006 |
| Final Goal | M6: Performance Validation | All | All | PG-001~006 |

**의존성 관계**:
```
M1 (Storage Quick Wins)  ──┐
                            ├──> M5 (Benchmarks) ──> M6 (Validation)
M2 (Query Quick Wins)   ──┤
                            │
M3 (Record Sharing)     ──┤
M4 (FSM Hints)          ──┘
```

- M1, M2는 독립적으로 병렬 진행 가능
- M3, M4는 M1/M2와 독립적이나 Secondary Goal
- M5는 M1~M4 완료 후 벤치마크 측정이 의미 있음
- M6는 모든 최적화 완료 후 최종 성능 검증

---

## 2. 마일스톤 상세

### M1: Storage Quick Wins (Primary Goal)

#### Task M1-1: LRU O(1) 자료구조 교체 (REQ-S-001)

**기술 접근**:
1. RED: BufferPool LRU touch가 O(1)임을 검증하는 벤치마크 기반 테스트 작성
2. GREEN: VecDeque<PageId> -> 이중 연결 리스트 + HashMap 구현
   - `LruCache` 내부 구조체: `HashMap<PageId, *mut LruNode>` + sentinel 노드 기반 이중 연결 리스트
   - 기존 `BufferPool` API 유지: `get()`, `get_mut()`, `insert()`, `evict_lru()`
3. REFACTOR: 코드 정리 및 내부 문서화

**영향 범위**:
- 수정: `crates/cypherlite-storage/src/page/buffer_pool.rs`
- 테스트: 기존 buffer_pool 단위 테스트 + 새 O(1) 검증 테스트
- 벤치마크: `storage_bench.rs`의 node_read 계열 성능 변화 측정

**위험 요소**:
- 이중 연결 리스트 구현에 unsafe 포인터 사용 필요 가능성
- 완화: `NonNull<T>` + 명확한 소유권 관리, `#[cfg(test)]` 에서 invariant 검증

#### Task M1-2: 미사용 의존성 제거 (REQ-S-002)

**기술 접근**:
1. RED: `cargo build` 후 바이너리 크기 baseline 기록
2. GREEN: Cargo.toml에서 crossbeam, dashmap 제거
3. REFACTOR: 컴파일 확인, 기존 테스트 전부 통과 확인

**영향 범위**:
- 수정: `crates/cypherlite-storage/Cargo.toml` (crossbeam, dashmap 제거)
- 수정: 필요시 `crates/cypherlite-query/Cargo.toml`
- 검증: `cargo build --workspace --all-features` 성공, 1,309 테스트 통과

**위험 요소**:
- tech.md에 crossbeam이 스토리지 의존성으로 기재됨 - 실제 코드 미사용 확인 필요
- 완화: 제거 전 `cargo tree -i crossbeam` 및 소스 grep으로 최종 확인

---

### M2: Query Quick Wins (Primary Goal)

#### Task M2-1: eval() 힙 할당 제거 (REQ-Q-001)

**기술 접근**:
1. RED: temporal property access에서 힙 할당이 없음을 검증하는 테스트 작성
2. GREEN: `format!("__temporal_props__{}", var_name)` 패턴을 다음 중 하나로 교체:
   - Option A: 사전 할당된 `SmallString` 또는 정적 prefix + var_name 연결
   - Option B: `Cow<'static, str>` 패턴으로 상수 키는 할당 없이 사용
   - Option C: 호출자에서 한 번 생성 후 참조 전달
3. REFACTOR: 코드 정리

**영향 범위**:
- 수정: `crates/cypherlite-query/src/executor/eval.rs`
- 테스트: temporal 쿼리 관련 기존 테스트 + 새 성능 특성 테스트

**위험 요소**:
- 라이프타임 제약으로 Cow 패턴 적용이 복잡할 수 있음
- 완화: 가장 단순한 접근(사전 할당 String을 함수 진입 시 1회 생성)부터 시도

#### Task M2-2: AND/OR Short-Circuit (REQ-Q-002)

**기술 접근**:
1. RED: AND(false, expensive_expr)이 expensive_expr을 평가하지 않음을 검증하는 테스트
2. GREEN: eval.rs의 AND/OR 분기에서 단락 평가 로직 구현
   - AND: `eval_left()` -> false이면 즉시 `Value::Bool(false)` 반환
   - OR: `eval_left()` -> true이면 즉시 `Value::Bool(true)` 반환
3. REFACTOR: 코드 정리, 에러 처리 경로 검증

**영향 범위**:
- 수정: `crates/cypherlite-query/src/executor/eval.rs` (4줄 수정)
- 테스트: WHERE 절 관련 기존 테스트 + 새 단락 평가 테스트

**위험 요소**:
- NULL 처리 의미론 변경 가능성 (3-valued logic)
- 완화: openCypher NULL 의미론 확인 후 구현 (NULL AND false = false, NULL OR true = true)

---

### M3: Record Sharing (Secondary Goal)

#### Task M3-1: Expand 연산자 Record Cow/Arc (REQ-Q-003)

**기술 접근**:
1. RED: Expand 연산에서 불필요한 clone이 발생하지 않음을 검증하는 테스트
2. GREEN: Record 타입에 Cow 패턴 도입
   - `expand.rs`의 `record.clone()` -> `Cow::Borrowed(&record)` 로 시작
   - 변수 바인딩이 필요한 경우에만 `Cow::Owned(record.clone())` 수행
   - VarLengthExpand에도 동일 패턴 적용
3. REFACTOR: 공통 패턴 추출, 코드 정리

**영향 범위**:
- 수정: `crates/cypherlite-query/src/executor/operators/expand.rs`
- 수정: `crates/cypherlite-query/src/executor/operators/var_length_expand.rs` (필요시)
- 테스트: expand 관련 기존 테스트 + 대규모 fanout 테스트

**위험 요소**:
- Record의 라이프타임 전파가 executor 전체에 영향 줄 수 있음
- 완화: 먼저 expand.rs만 지역적으로 적용, 컴파일 및 테스트 확인 후 확장
- 대안: 라이프타임이 복잡하면 Arc<HashMap> 사용 (약간의 atomic 오버헤드)

---

### M4: FSM Allocation Hints (Secondary Goal)

#### Task M4-1: FSM Next-Free 힌트 구현 (REQ-S-003)

**기술 접근**:
1. RED: 연속 할당 시 선형 스캔이 아닌 힌트 기반 할당임을 검증하는 테스트
2. GREEN:
   - `PageManager` 구조체에 `next_free_hint: PageId` 필드 추가
   - `allocate_page()`: hint 위치부터 FSM 스캔 시작
   - `free_page()`: 해제된 page ID < hint이면 hint 갱신
   - 체크포인트 시 hint 리셋 (다음 기동 시 재계산)
3. REFACTOR: 코드 정리

**영향 범위**:
- 수정: `crates/cypherlite-storage/src/page/page_manager.rs`
- 테스트: 페이지 할당 단위 테스트 + 순차 할당 성능 테스트

**위험 요소**:
- hint가 실제 빈 페이지를 가리키지 않을 경우 (해제 후 재할당 경합)
- 완화: hint는 최적화 힌트이며 fallback으로 전체 스캔 유지

---

### M5: Benchmark Infrastructure (Final Goal)

#### Task M5-1: 동시성 벤치마크 (REQ-B-001)

**기술 접근**:
- 4스레드 동시 읽기 벤치마크 (1,000/10,000 노드 셋에서)
- 읽기-쓰기 경합 벤치마크 (3 reader + 1 writer)
- write-lock 획득 지연 측정

**대상 파일**: 신규 `crates/cypherlite-storage/benches/concurrent_bench.rs`

#### Task M5-2: 메모리 프로파일링 벤치마크 (REQ-B-002)

**기술 접근**:
- 노드 수 증가에 따른 메모리 사용량 측정 (1K, 10K, 100K, 1M)
- peak RSS 측정 (platform-specific: `/proc/self/status` 또는 `mach_task_info`)
- 노드당 바이트 비용 계산

**대상 파일**: 신규 `crates/cypherlite-storage/benches/memory_bench.rs`

#### Task M5-3: 쿼리 스트리밍 벤치마크 (REQ-B-003)

**기술 접근**:
- 대규모 결과 셋(10K, 100K records) MATCH 쿼리 벤치마크
- 2-hop 패턴 쿼리 대규모 그래프(10K 노드, 50K 엣지) 벤치마크
- first-result 지연 시간 측정

**대상 파일**: 신규 `crates/cypherlite-query/benches/streaming_bench.rs`

---

### M6: Performance Validation (Final Goal)

#### Task M6-1: 성능 게이트 최종 검증

**기술 접근**:
1. 모든 최적화 적용 후 release 빌드로 벤치마크 실행
2. 6개 성능 게이트(PG-001~006) 충족 여부 확인
3. 미달 항목에 대한 추가 최적화 또는 Tier 3 승격 판단

**성능 게이트 검증 매트릭스**:

| 게이트 | 측정 방법 | 판단 기준 |
|-------|----------|----------|
| PG-001 | criterion simple_match 벤치마크 p99 | < 10ms |
| PG-002 | criterion 2-hop pattern 벤치마크 p99 | < 50ms |
| PG-003 | `ls -la target/release/` 바이너리 크기 | < 50MB |
| PG-004 | memory_bench 1M 노드 RSS | < 500MB |
| PG-005 | storage_bench sequential write 처리량 | > 1,000 노드/sec |
| PG-006 | concurrent_bench 4-thread read 처리량 | > 50,000 reads/sec |

---

## 3. 기술 접근 요약

### 3.1 Storage Layer 최적화 전략

| 최적화 | 접근 방식 | 대안 |
|-------|----------|------|
| LRU O(1) | 이중 연결 리스트 + HashMap | `lru` 크레이트 사용 |
| Unused deps | Cargo.toml에서 제거 | feature-gate로 조건부 포함 |
| FSM hints | next_free_hint 필드 + fallback 전체 스캔 | in-memory 비트맵 풀 캐시 |

### 3.2 Query Layer 최적화 전략

| 최적화 | 접근 방식 | 대안 |
|-------|----------|------|
| eval() 할당 | 함수 진입 시 1회 사전 할당 | SmallString/Cow<str> |
| Short-circuit | 단락 평가 + NULL 3-valued logic | 별도 compile-time 최적화 패스 |
| Record sharing | Cow<Record> 패턴 | Arc<Record> + copy-on-write |

### 3.3 TDD 사이클 적용

모든 최적화 작업은 TDD RED-GREEN-REFACTOR 사이클을 따름:
- **RED**: 최적화 전 성능 특성을 검증하는 실패 테스트 작성 (또는 벤치마크 baseline)
- **GREEN**: 최소한의 변경으로 최적화 적용
- **REFACTOR**: 코드 정리, 문서화, 벤치마크 비교

---

## 4. 위험 분석

### 4.1 기술적 위험

| 위험 | 확률 | 영향 | 완화 전략 |
|------|------|------|----------|
| LRU unsafe 포인터 사용으로 메모리 안전성 문제 | MEDIUM | HIGH | NonNull 사용, Miri로 검증, #[cfg(test)] invariant 체크 |
| Record Cow 라이프타임 전파가 executor 전체에 영향 | MEDIUM | MEDIUM | 지역적 적용 후 확장, Arc 대안 준비 |
| NULL 의미론 변경으로 기존 쿼리 결과 달라짐 | LOW | HIGH | openCypher 3-valued logic 표준 준수, 기존 테스트로 회귀 검증 |
| FSM hint와 동시 접근 경합 | LOW | LOW | hint는 최적화용, 정확성 불요, fallback 스캔 유지 |
| crossbeam 제거 시 간접 의존성 깨짐 | LOW | MEDIUM | cargo tree 확인, CI 파이프라인에서 즉시 탐지 |

### 4.2 성능 위험

| 위험 | 시나리오 | 완화 전략 |
|------|---------|----------|
| PG-004 미달 | 1M 노드 시 500MB 초과 | Tier 3의 Value enum 크기 최적화 승격 |
| PG-005 미달 | FSM 힌트만으로 불충분 | WAL 그룹 커밋 승격 검토 |

---

## 5. 수정 대상 파일 요약

| 파일 | 수정 유형 | 마일스톤 |
|------|----------|---------|
| `crates/cypherlite-storage/src/page/buffer_pool.rs` | 주요 수정 | M1 |
| `crates/cypherlite-storage/Cargo.toml` | 의존성 제거 | M1 |
| `crates/cypherlite-query/Cargo.toml` | 의존성 확인/제거 | M1 |
| `crates/cypherlite-query/src/executor/eval.rs` | 수정 | M2 |
| `crates/cypherlite-query/src/executor/operators/expand.rs` | 주요 수정 | M3 |
| `crates/cypherlite-query/src/executor/operators/var_length_expand.rs` | 수정 (필요시) | M3 |
| `crates/cypherlite-storage/src/page/page_manager.rs` | 수정 | M4 |
| `crates/cypherlite-storage/benches/concurrent_bench.rs` | 신규 | M5 |
| `crates/cypherlite-storage/benches/memory_bench.rs` | 신규 | M5 |
| `crates/cypherlite-query/benches/streaming_bench.rs` | 신규 | M5 |
