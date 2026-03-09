---
id: SPEC-DB-001
document: acceptance
version: "1.0.0"
status: draft
created: "2026-03-10"
updated: "2026-03-10"
tags: [storage-engine, acid, wal, btree, buffer-pool, transaction]
---

# SPEC-DB-001: 인수 기준 - Storage Engine (v0.1)

> CypherLite Phase 1 스토리지 엔진의 인수 테스트 시나리오, 성능 게이트, 품질 게이트를 정의한다.

---

## 1. 테스트 시나리오 (Given-When-Then)

### 시나리오 1: 기본 노드 생성 및 조회

**[REQ-STORE-001, REQ-STORE-002]**

```gherkin
Scenario: 노드를 생성하고 ID로 조회한다
  Given 빈 CypherLite 데이터베이스가 열려 있다
  And 쓰기 트랜잭션이 시작되었다
  When 레이블 "Person"과 프로퍼티 {name: "Alice", age: 30}을 가진 노드를 생성한다
  Then 고유한 NodeId가 반환된다
  And 해당 NodeId로 노드를 조회하면 레이블 "Person"과 프로퍼티 {name: "Alice", age: 30}이 포함된 NodeRecord가 반환된다

Scenario: 존재하지 않는 NodeId로 조회하면 None을 반환한다
  Given 빈 CypherLite 데이터베이스가 열려 있다
  When NodeId(999999)로 노드를 조회한다
  Then None이 반환된다

Scenario: 노드의 프로퍼티를 수정한다
  Given NodeId(1)에 프로퍼티 {name: "Alice"}를 가진 노드가 존재한다
  And 쓰기 트랜잭션이 시작되었다
  When NodeId(1)의 프로퍼티를 {name: "Alice", email: "alice@example.com"}으로 업데이트한다
  And 트랜잭션을 커밋한다
  Then NodeId(1)을 조회하면 프로퍼티 {name: "Alice", email: "alice@example.com"}이 반환된다

Scenario: 노드를 삭제하면 연결된 엣지도 함께 삭제된다
  Given NodeId(1)과 NodeId(2)가 존재한다
  And EdgeId(1)이 NodeId(1) -> NodeId(2) 관계로 연결되어 있다
  When NodeId(1)을 삭제한다
  Then NodeId(1)을 조회하면 None이 반환된다
  And EdgeId(1)을 조회하면 None이 반환된다
```

---

### 시나리오 2: 엣지 생성 및 인접 체인 탐색

**[REQ-STORE-005, REQ-STORE-006, REQ-STORE-007, REQ-STORE-008]**

```gherkin
Scenario: 엣지를 생성하고 인접 체인을 통해 탐색한다
  Given NodeId(1) "Alice"와 NodeId(2) "Bob"이 존재한다
  And 쓰기 트랜잭션이 시작되었다
  When "Alice" -> "Bob" 관계 타입 "KNOWS"로 엣지를 생성한다
  And 트랜잭션을 커밋한다
  Then 고유한 EdgeId가 반환된다
  And EdgeId로 엣지를 조회하면 start_node=NodeId(1), end_node=NodeId(2), rel_type="KNOWS"가 반환된다
  And NodeId(1)의 인접 엣지 목록에 해당 엣지가 포함된다
  And NodeId(2)의 인접 엣지 목록에 해당 엣지가 포함된다

Scenario: 한 노드에 여러 엣지를 생성하고 인접 체인으로 모두 조회한다
  Given NodeId(1) "Alice"가 존재한다
  And NodeId(2) "Bob", NodeId(3) "Carol", NodeId(4) "Dave"가 존재한다
  When "Alice" -> "Bob" 관계 "KNOWS" 엣지를 생성한다
  And "Alice" -> "Carol" 관계 "KNOWS" 엣지를 생성한다
  And "Alice" -> "Dave" 관계 "WORKS_WITH" 엣지를 생성한다
  And 트랜잭션을 커밋한다
  Then NodeId(1)의 인접 엣지 목록을 조회하면 3개의 엣지가 반환된다
  And 모든 엣지의 start_node이 NodeId(1)이다

Scenario: 엣지 삭제 시 인접 체인 포인터가 올바르게 갱신된다
  Given NodeId(1) -> NodeId(2) 엣지 EdgeId(1)이 존재한다
  And NodeId(1) -> NodeId(3) 엣지 EdgeId(2)가 존재한다
  When EdgeId(1)을 삭제한다
  Then NodeId(1)의 인접 엣지 목록에 EdgeId(2)만 포함된다
  And EdgeId(1) 조회 시 None이 반환된다
```

---

### 시나리오 3: ACID 트랜잭션 Atomicity (롤백 테스트)

**[REQ-TX-003, REQ-TX-004, REQ-TX-011]**

```gherkin
Scenario: 트랜잭션 롤백 시 모든 변경이 폐기된다
  Given 빈 CypherLite 데이터베이스가 열려 있다
  And 쓰기 트랜잭션 TX1이 시작되었다
  When TX1에서 노드 A를 생성한다
  And TX1에서 노드 B를 생성한다
  And TX1을 롤백한다
  Then 새로운 읽기 트랜잭션에서 노드 A를 조회하면 None이 반환된다
  And 새로운 읽기 트랜잭션에서 노드 B를 조회하면 None이 반환된다

Scenario: 커밋되지 않은 트랜잭션의 변경은 다른 트랜잭션에서 보이지 않는다
  Given 노드 A가 존재하는 CypherLite 데이터베이스가 열려 있다
  And 쓰기 트랜잭션 TX1이 시작되었다
  And TX1에서 노드 B를 생성한다 (아직 커밋하지 않음)
  When 읽기 트랜잭션 TX2를 시작한다
  Then TX2에서 노드 B를 조회하면 None이 반환된다 (TX1 미커밋)
  And TX2에서 노드 A를 조회하면 정상적으로 반환된다

Scenario: 커밋 후 변경이 영속된다
  Given 빈 CypherLite 데이터베이스가 열려 있다
  And 쓰기 트랜잭션 TX1이 시작되었다
  When TX1에서 노드 C를 생성한다
  And TX1을 커밋한다
  Then 새로운 읽기 트랜잭션에서 노드 C를 조회하면 정상적으로 반환된다
```

---

### 시나리오 4: WAL 크래시 복구

**[REQ-TX-007, REQ-TX-008, REQ-WAL-001, REQ-WAL-007]**

```gherkin
Scenario: 커밋 후 크래시 발생 시 WAL 재생으로 데이터가 복원된다
  Given CypherLite 데이터베이스에 노드 A를 생성하고 커밋하였다
  And 체크포인트를 수행하지 않은 상태이다 (WAL에만 커밋 프레임 존재)
  When 프로세스가 비정상 종료된다 (시뮬레이션: 파일 핸들만 닫음)
  And 데이터베이스를 다시 연다
  Then 자동으로 WAL 복구가 수행된다
  And 노드 A를 조회하면 정상적으로 반환된다

Scenario: 미커밋 상태에서 크래시 발생 시 미커밋 데이터가 폐기된다
  Given CypherLite 데이터베이스에 노드 X를 생성하고 커밋하였다
  And 이후 쓰기 트랜잭션에서 노드 Y를 생성하였으나 커밋하지 않았다
  When 프로세스가 비정상 종료된다
  And 데이터베이스를 다시 연다
  Then 노드 X를 조회하면 정상적으로 반환된다
  And 노드 Y를 조회하면 None이 반환된다

Scenario: checksum 불일치 WAL 프레임은 무시된다
  Given WAL 파일에 손상된 프레임(checksum 불일치)이 포함되어 있다
  When 데이터베이스를 연다
  Then 복구 과정에서 손상된 프레임은 무시된다
  And checksum이 유효한 커밋 프레임만 재생된다
  And 에러 로그에 손상된 프레임 정보가 기록된다
```

---

### 시나리오 5: 동시 Reader + Writer

**[REQ-TX-006, REQ-TX-009, REQ-TX-010]**

```gherkin
Scenario: 읽기 트랜잭션은 쓰기 트랜잭션에 의해 차단되지 않는다
  Given 노드 A가 존재하는 CypherLite 데이터베이스가 열려 있다
  And 쓰기 트랜잭션 Writer가 시작되었다
  When 읽기 트랜잭션 Reader를 시작한다
  Then Reader에서 노드 A를 즉시 조회할 수 있다 (Writer에 의한 차단 없음)
  And Reader가 보는 데이터는 Writer 시작 이전의 일관된 스냅샷이다

Scenario: 동시에 두 개의 쓰기 트랜잭션을 시작하면 경합 에러가 발생한다
  Given CypherLite 데이터베이스가 열려 있다
  And 쓰기 트랜잭션 Writer1이 시작되었다
  When 쓰기 트랜잭션 Writer2를 시작하려고 시도한다
  Then TransactionConflict 에러가 반환된다
  And Writer1은 영향을 받지 않고 정상적으로 계속 동작한다

Scenario: 멀티 스레드 동시 읽기가 안전하게 수행된다
  Given 1000개의 노드가 존재하는 CypherLite 데이터베이스가 열려 있다
  When 4개의 스레드가 동시에 임의의 NodeId로 노드를 조회한다
  Then 모든 스레드가 올바른 노드 데이터를 반환받는다
  And race condition이나 데이터 손상이 발생하지 않는다
```

---

### 시나리오 6: 성능 - 노드 쓰기 처리량

**[spec.md 4.2 성능 목표]**

```gherkin
Scenario: 순차 노드 쓰기 처리량이 1000 노드/초를 초과한다
  Given 빈 CypherLite 데이터베이스가 열려 있다 (release 빌드)
  When 단일 쓰기 트랜잭션에서 10,000개의 노드를 순차적으로 생성한다
  And 각 노드에 프로퍼티 3개 (String, Int64, Bool)를 포함한다
  And 트랜잭션을 커밋한다
  Then 총 소요 시간이 10초 미만이다 (> 1,000 노드/초)
  And 커밋 후 임의의 NodeId로 조회하면 올바른 데이터가 반환된다

Scenario: 캐시 히트 노드 읽기 지연이 1ms 미만이다
  Given 1000개의 노드가 존재하고 모두 버퍼 풀에 캐시되어 있다
  When 임의의 NodeId로 1000회 조회를 수행한다
  Then 평균 조회 지연이 1ms 미만이다

Scenario: 캐시 미스 노드 읽기 지연이 10ms 미만이다
  Given 10,000개의 노드가 존재한다 (버퍼 풀 용량 초과)
  When 버퍼 풀에 존재하지 않는 NodeId로 조회한다
  Then 조회 지연이 10ms 미만이다 (디스크 접근 포함)
```

---

## 2. 성능 게이트 기준

### 필수 성능 목표 (MUST PASS)

| 지표 | 목표값 | 측정 방법 | 판정 |
|------|--------|-----------|------|
| 순차 노드 쓰기 속도 | > 1,000 노드/초 | `cargo bench --bench storage_bench` (release) | 3회 연속 측정 중 최소값 기준 |
| 단일 노드 읽기 (캐시 히트) | < 1ms (p95) | criterion 벤치마크 | 통계적 유의 수준 95% |
| 단일 노드 읽기 (캐시 미스) | < 10ms (p95) | criterion 벤치마크 | 통계적 유의 수준 95% |
| WAL 프레임 쓰기 + fsync | < 5ms (p95) | criterion 벤치마크 | SSD 기준 |
| 크래시 복구 시간 (1000 프레임) | < 1초 | 통합 테스트 시간 측정 | 3회 평균 |

### 권장 성능 목표 (SHOULD PASS)

| 지표 | 목표값 | 비고 |
|------|--------|------|
| 체크포인트 (1000 프레임) | < 500ms | SSD 기준 |
| 동시 읽기 처리량 (4 스레드) | 읽기 성능 저하 < 10% | 단일 Writer 동시 실행 중 |
| 메모리 사용량 (10,000 노드) | < 50MB | 기본 버퍼 풀 설정 |

---

## 3. 품질 게이트 기준

### 필수 품질 기준 (MUST PASS)

| 기준 | 목표 | 검증 명령 |
|------|------|-----------|
| **전체 테스트 통과** | 0 failures | `cargo test --release` |
| **Clippy 경고 없음** | 0 warnings | `cargo clippy -- -D warnings` |
| **포맷 준수** | 차이 없음 | `cargo fmt --check` |
| **코드 커버리지** | >= 85% | `cargo tarpaulin` 또는 `cargo llvm-cov` |
| **크리티컬 경로 커버리지** | >= 95% | WAL, Transaction, Recovery 모듈 |
| **보안 취약점** | 0 critical/high | `cargo audit` |

### 권장 품질 기준 (SHOULD PASS)

| 기준 | 목표 | 비고 |
|------|------|------|
| 속성 기반 테스트 | proptest 10,000 케이스 통과 | B-tree, 직렬화, WAL |
| Thread Sanitizer | race condition 0건 | `RUSTFLAGS="-Z sanitizer=thread"` |
| 문서 커버리지 | 공개 API 100% doc comment | `#![warn(missing_docs)]` |
| `unsafe` 사용 | 최소화 (0 또는 명시적 safety 주석) | 필요 시 `// SAFETY:` 주석 필수 |

---

## 4. 인수 체크리스트 (Definition of Done)

### 기능 완성도

- [ ] 노드 CRUD (생성, 조회, 수정, 삭제) 정상 동작
- [ ] 엣지 CRUD (생성, 조회, 삭제) 정상 동작
- [ ] Index-Free Adjacency 인접 체인 탐색 정상 동작
- [ ] 프로퍼티 인라인 저장 (7개 타입 모두 지원)
- [ ] Overflow 프로퍼티 저장/읽기 정상 동작
- [ ] 파일 형식 (.cyl) 생성/오픈/검증 정상 동작
- [ ] Free Space Map 페이지 할당/해제 정상 동작
- [ ] 버퍼 풀 LRU 캐시 정상 동작 (pin/unpin/evict/flush)
- [ ] WAL 쓰기/읽기/체크포인트 정상 동작
- [ ] 크래시 복구 (WAL 재생) 정상 동작
- [ ] 트랜잭션 begin/commit/rollback 정상 동작
- [ ] Snapshot Isolation (읽기 일관성) 정상 동작
- [ ] Single-Writer 배타적 잠금 정상 동작

### ACID 준수

- [ ] Atomicity: 롤백 시 모든 변경 폐기 확인
- [ ] Consistency: 노드 삭제 시 연결 엣지 자동 삭제 확인
- [ ] Isolation: 미커밋 데이터 타 트랜잭션 비가시 확인
- [ ] Durability: 커밋 후 프로세스 재시작 시 데이터 보존 확인

### 코드 품질

- [ ] `cargo test --release` 전체 통과
- [ ] `cargo clippy -- -D warnings` 경고 0건
- [ ] `cargo fmt --check` 포맷 준수
- [ ] 코드 커버리지 85% 이상
- [ ] WAL/Transaction/Recovery 모듈 커버리지 95% 이상
- [ ] `cargo audit` critical/high 취약점 0건
- [ ] 공개 API에 doc comment 작성 완료

### 성능 검증

- [ ] 순차 노드 쓰기 > 1,000 노드/초 (release 빌드)
- [ ] 캐시 히트 읽기 < 1ms (p95)
- [ ] 캐시 미스 읽기 < 10ms (p95)
- [ ] 크래시 복구 < 1초 (1000 WAL 프레임)
- [ ] criterion 벤치마크 기준선(baseline) 저장 완료
