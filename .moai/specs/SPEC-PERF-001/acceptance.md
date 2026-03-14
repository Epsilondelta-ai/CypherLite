---
id: SPEC-PERF-001
document: acceptance
version: 1.0.0
status: approved
created: 2026-03-15
updated: 2026-03-15
author: epsilondelta
---

# SPEC-PERF-001: Acceptance Criteria (인수 기준)

## 1. Module 1: Storage Performance (스토리지 성능)

### AC-S-001: LRU Touch O(1) 최적화 (REQ-S-001)

**Scenario 1: 캐시 히트 시 O(1) LRU 갱신**

```gherkin
Given BufferPool이 256개 페이지로 가득 찬 상태에서
When 임의의 캐시된 페이지에 get()을 호출하면
Then LRU 순서 갱신이 O(1) 시간 복잡도로 완료되어야 한다
And 기존 VecDeque::retain 대비 최소 10배 이상 빠른 touch 성능을 보여야 한다
```

**Scenario 2: LRU Eviction 정확성**

```gherkin
Given BufferPool이 최대 용량(256 페이지)에 도달한 상태에서
When 새로운 페이지를 insert()하면
Then 가장 오래 접근되지 않은 페이지가 정확하게 evict되어야 한다
And evict된 페이지가 dirty이면 디스크에 기록된 후 evict되어야 한다
```

**Scenario 3: LRU 동시성 안전성**

```gherkin
Given BufferPool이 다중 스레드에서 공유되는 상태에서
When 여러 스레드가 동시에 get()과 insert()를 호출하면
Then 데이터 레이스 없이 정확한 LRU 동작이 유지되어야 한다
And Miri 또는 ThreadSanitizer 검증을 통과해야 한다
```

### AC-S-002: 미사용 의존성 제거 (REQ-S-002)

**Scenario 1: 의존성 제거 후 빌드 성공**

```gherkin
Given crossbeam과 dashmap이 Cargo.toml에서 제거된 상태에서
When cargo build --workspace --all-features를 실행하면
Then 컴파일이 성공해야 한다
And 기존 1,309개 테스트가 전부 통과해야 한다
```

**Scenario 2: 바이너리 크기 감소**

```gherkin
Given 미사용 의존성이 제거된 상태에서
When release 빌드를 수행하면
Then 바이너리 크기가 최소 200KB 이상 감소해야 한다
```

### AC-S-003: FSM 페이지 할당 힌트 (REQ-S-003)

**Scenario 1: 힌트 기반 할당 성능**

```gherkin
Given 100개 페이지가 할당된 후 50개가 해제된 상태에서
When 새로운 페이지 할당을 요청하면
Then next_free_hint 위치부터 스캔을 시작하여 기존 대비 빠르게 빈 페이지를 찾아야 한다
And 힌트가 유효하지 않은 경우 전체 스캔으로 fallback하여 정확성을 보장해야 한다
```

**Scenario 2: 힌트 갱신 정확성**

```gherkin
Given next_free_hint가 페이지 50을 가리키는 상태에서
When 페이지 30이 해제되면
Then next_free_hint가 30으로 갱신되어야 한다
And 페이지 70이 해제되면 hint는 여전히 30을 가리켜야 한다
```

---

## 2. Module 2: Query Performance (쿼리 성능)

### AC-Q-001: Temporal 표현식 힙 할당 제거 (REQ-Q-001)

**Scenario 1: Temporal 프로퍼티 접근 최적화**

```gherkin
Given temporal 노드에 AT TIME 쿼리를 실행할 때
When eval()에서 temporal 프로퍼티 키를 생성하면
Then format!() 매크로를 통한 힙 할당이 발생하지 않아야 한다
And 사전 할당된 키 또는 Cow<str> 패턴을 사용해야 한다
```

**Scenario 2: Temporal 쿼리 성능 유지**

```gherkin
Given 1,000개 temporal 노드가 존재하는 데이터베이스에서
When AT TIME 쿼리를 실행하면
Then 기존 대비 동일하거나 더 나은 성능을 보여야 한다
And 기존 temporal 쿼리 테스트가 전부 통과해야 한다
```

### AC-Q-002: AND/OR Short-Circuit Evaluation (REQ-Q-002)

**Scenario 1: AND 단락 평가**

```gherkin
Given WHERE 절에 AND 조건이 있는 쿼리에서
When 왼쪽 조건이 false로 평가되면
Then 오른쪽 조건은 평가되지 않아야 한다
And 최종 결과는 false여야 한다
```

**Scenario 2: OR 단락 평가**

```gherkin
Given WHERE 절에 OR 조건이 있는 쿼리에서
When 왼쪽 조건이 true로 평가되면
Then 오른쪽 조건은 평가되지 않아야 한다
And 최종 결과는 true여야 한다
```

**Scenario 3: NULL과의 단락 평가 (3-valued logic)**

```gherkin
Given WHERE 절에 NULL 값이 포함된 AND/OR 조건이 있을 때
When NULL AND false를 평가하면
Then 결과는 false여야 한다 (openCypher 3-valued logic)
And NULL OR true를 평가하면 결과는 true여야 한다
And 기존 NULL 처리 테스트가 전부 통과해야 한다
```

**Edge Case: 부작용 없는 표현식 보장**

```gherkin
Given 순수 표현식(side-effect free)으로 구성된 WHERE 절에서
When 단락 평가가 적용되면
Then 쿼리 결과가 단락 평가 전과 동일해야 한다
And 기존 WHERE 절 테스트 전체가 통과해야 한다
```

### AC-Q-003: Expand 연산자 Record 공유 (REQ-Q-003)

**Scenario 1: Record Clone 감소**

```gherkin
Given 100개 노드와 500개 엣지를 가진 그래프에서
When 2-hop MATCH 패턴 쿼리를 실행하면
Then Record의 전체 HashMap clone 횟수가 기존 대비 50% 이상 감소해야 한다
And Cow::Borrowed 또는 Arc 참조를 통해 불필요한 복사를 회피해야 한다
```

**Scenario 2: 쿼리 결과 정확성 유지**

```gherkin
Given Cow/Arc 기반 Record 공유가 적용된 상태에서
When 다양한 MATCH 패턴 쿼리를 실행하면
Then 모든 결과가 기존 clone 기반 결과와 동일해야 한다
And 변수 바인딩이 필요한 경우에만 실제 복사가 발생해야 한다
```

**Edge Case: 대규모 Fanout**

```gherkin
Given 단일 노드에서 100개 이상의 엣지가 나가는 star graph에서
When MATCH (n)-[r]->(m) 패턴을 실행하면
Then 메모리 사용량이 기존 대비 유의미하게 감소해야 한다
And OOM 없이 정상 완료되어야 한다
```

---

## 3. Module 3: Benchmark Infrastructure (벤치마크 인프라)

### AC-B-001: 동시성 벤치마크 (REQ-B-001)

**Scenario 1: 다중 스레드 읽기 벤치마크**

```gherkin
Given 10,000개 노드가 로드된 데이터베이스에서
When 4개 스레드가 동시에 랜덤 노드 읽기를 수행하면
Then criterion 벤치마크가 처리량(reads/sec)을 정확히 측정해야 한다
And 벤치마크 결과가 재현 가능해야 한다 (통계적 유의성)
```

**Scenario 2: 읽기-쓰기 경합 벤치마크**

```gherkin
Given 3개 reader 스레드와 1개 writer 스레드가 동시에 동작할 때
When 10초간 지속적으로 읽기/쓰기를 수행하면
Then reader 처리량과 writer 처리량이 각각 측정되어야 한다
And write-lock 획득 평균 지연 시간이 기록되어야 한다
```

### AC-B-002: 메모리 프로파일링 벤치마크 (REQ-B-002)

**Scenario 1: 대규모 데이터셋 메모리 측정**

```gherkin
Given 빈 데이터베이스에서 시작하여
When 1K, 10K, 100K, 1M 노드를 순차적으로 삽입하면
Then 각 단계별 RSS 메모리 사용량(MB)이 측정되어야 한다
And 노드당 평균 바이트 비용이 계산되어야 한다
```

**Scenario 2: 메모리 목표 충족 여부**

```gherkin
Given 100만 노드가 로드된 데이터베이스에서
When 메모리 사용량을 측정하면
Then RSS가 500MB 미만이어야 한다 (PG-004)
```

### AC-B-003: 쿼리 스트리밍 벤치마크 (REQ-B-003)

**Scenario 1: 대규모 결과 셋 처리**

```gherkin
Given 10K 노드와 50K 엣지를 가진 그래프에서
When MATCH (n) RETURN n 쿼리를 실행하면
Then 전체 결과 반환까지의 처리 시간이 측정되어야 한다
And 피크 메모리 사용량이 기록되어야 한다
```

**Scenario 2: 2-hop 대규모 패턴 쿼리**

```gherkin
Given 10K 노드와 50K 엣지를 가진 그래프에서
When MATCH (a)-[r1]->(b)-[r2]->(c) RETURN a, b, c 쿼리를 실행하면
Then p99 지연 시간이 측정되어야 한다
And first-result 지연 시간이 별도로 기록되어야 한다
```

---

## 4. Performance Gate Criteria (성능 게이트 기준)

### 최종 성능 검증 기준

모든 최적화 완료 후 release 빌드에서 다음 기준을 충족해야 한다:

| Gate ID | 지표 | 기준 | 측정 방법 | 판정 |
|---------|------|------|----------|------|
| PG-001 | 단순 매치 쿼리 p99 | < 10ms | criterion `simple_match` bench | MUST PASS |
| PG-002 | 2홉 패턴 쿼리 p99 | < 50ms | criterion `two_hop_pattern` bench | MUST PASS |
| PG-003 | 바이너리 크기 | < 50MB | `ls -la target/release/` | MUST PASS |
| PG-004 | 메모리 (1M 노드) | < 500MB | `memory_bench` RSS 측정 | MUST PASS |
| PG-005 | 순차 쓰기 속도 | > 1,000 노드/sec | criterion `sequential_write` bench | MUST PASS |
| PG-006 | 동시 읽기 (4T) | > 50,000 reads/sec | criterion `concurrent_read_4t` bench | MUST PASS |

### 성능 게이트 판정 규칙

```gherkin
Given 모든 Tier 1/Tier 2 최적화가 적용된 release 빌드에서
When 6개 성능 게이트를 순차적으로 검증하면
Then 6개 모두 PASS여야 SPEC-PERF-001이 완료된다
And 하나라도 FAIL이면 해당 게이트 관련 Tier 3 최적화를 승격하여 추가 작업한다
```

---

## 5. Quality Gate Criteria (품질 게이트 기준)

### 5.1 테스트 커버리지

```gherkin
Given SPEC-PERF-001의 모든 코드 변경이 완료된 상태에서
When cargo llvm-cov --workspace --all-features를 실행하면
Then 전체 커버리지가 85% 이상이어야 한다
And 신규/수정 코드의 커버리지가 80% 이상이어야 한다
```

### 5.2 기존 테스트 회귀 방지

```gherkin
Given 모든 최적화가 적용된 상태에서
When cargo test --workspace --all-features를 실행하면
Then 기존 1,309개 테스트가 전부 통과해야 한다
And 새로 추가된 테스트도 모두 통과해야 한다
```

### 5.3 CI 파이프라인 통과

```gherkin
Given 모든 변경사항이 커밋된 상태에서
When GitHub Actions CI 파이프라인이 실행되면
Then 6개 Job(check, msrv, test, coverage, security, bench-check)이 모두 통과해야 한다
```

### 5.4 코드 품질

```gherkin
Given 모든 최적화 코드가 작성된 상태에서
When cargo clippy --workspace --all-targets --all-features -- -D warnings를 실행하면
Then 경고 0건이어야 한다
And cargo fmt --all --check이 통과해야 한다
```

---

## 6. Edge Cases (엣지 케이스)

### 6.1 LRU 엣지 케이스

| 케이스 | 기대 동작 |
|--------|----------|
| 빈 캐시에서 evict 요청 | None 반환 (패닉 없음) |
| 동일 페이지 연속 touch | 중복 없이 정상 동작 |
| 캐시 크기 1에서 insert | 기존 항목 evict 후 새 항목 삽입 |
| 모든 페이지가 dirty 상태에서 evict | 디스크 기록 후 evict |

### 6.2 Short-Circuit 엣지 케이스

| 케이스 | 기대 동작 |
|--------|----------|
| AND(error_expr, valid_expr) | 에러 전파 (단락 평가 미적용) |
| OR(error_expr, valid_expr) | 에러 전파 (단락 평가 미적용) |
| AND(false, error_expr) | false 반환 (단락 평가로 에러 회피) |
| OR(true, error_expr) | true 반환 (단락 평가로 에러 회피) |
| 중첩 AND/OR (a AND (b OR c)) | 각 레벨에서 올바른 단락 평가 |

### 6.3 FSM Hint 엣지 케이스

| 케이스 | 기대 동작 |
|--------|----------|
| hint 위치가 이미 할당된 페이지 | 다음 빈 페이지까지 스캔 계속 |
| 모든 페이지가 할당됨 | 새 페이지 확장 (기존 동작 유지) |
| 체크포인트 후 재시작 | hint 리셋, 첫 할당 시 재계산 |

### 6.4 Record Cow 엣지 케이스

| 케이스 | 기대 동작 |
|--------|----------|
| 읽기 전용 접근 (RETURN만) | Borrowed 유지, clone 없음 |
| SET으로 프로퍼티 변경 | Owned로 전환 후 수정 |
| 빈 Record (변수 없음) | 정상 동작 (빈 HashMap) |
| 대규모 프로퍼티 맵 (100+ 키) | Cow 패턴으로 복사 비용 절감 |

---

## 7. Definition of Done (완료 정의)

SPEC-PERF-001은 다음 조건을 **모두** 충족할 때 완료된다:

1. **기능 완료**: REQ-S-001~003, REQ-Q-001~003, REQ-B-001~003 모든 요구사항 구현
2. **성능 달성**: PG-001~006 6개 성능 게이트 전부 PASS
3. **테스트 통과**: 기존 1,309개 + 신규 테스트 전부 통과
4. **커버리지**: 전체 85% 이상, 신규/수정 코드 80% 이상
5. **CI 통과**: GitHub Actions 6개 Job 전부 PASS
6. **코드 품질**: clippy 경고 0건, fmt 통과
7. **벤치마크 인프라**: 동시성/메모리/스트리밍 3개 벤치마크 파일 추가됨
8. **문서화**: 성능 최적화 결과가 벤치마크 데이터와 함께 기록됨
