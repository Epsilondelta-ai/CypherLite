---
id: SPEC-DB-001
document: plan
version: "1.0.0"
status: draft
created: "2026-03-10"
updated: "2026-03-10"
tags: [storage-engine, acid, wal, btree, buffer-pool, transaction]
---

# SPEC-DB-001: 구현 계획 - Storage Engine (v0.1)

> CypherLite Phase 1 스토리지 엔진의 구현 전략, 태스크 분해, 기술적 접근 방식을 정의한다.

---

## 1. 구현 순서 및 의존성

### 마일스톤 구조

구현은 **상향식(bottom-up)** 접근으로, 하위 레이어부터 순차적으로 구축한다.

```
M1: Core Types & Config
 |
 v
M2: Page Manager & File Format
 |
 v
M3: Buffer Pool (LRU Cache)
 |
 v
M4: WAL Writer & Reader ──────> M6: Crash Recovery
 |                                 |
 v                                 v
M5: B-tree (Node/Edge Store) ──> M7: Transaction Manager (MVCC)
                                   |
                                   v
                                 M8: ACID Integration & Validation
```

---

### M1: 핵심 타입 및 설정 (Primary Goal)

**크레이트**: `cypherlite-core`

**태스크**:
- [ ] `types.rs`: `NodeId(u64)`, `EdgeId(u64)`, `PageId(u32)`, `PropertyValue` enum 정의
- [ ] `types.rs`: `NodeRecord`, `RelationshipRecord`, `PropertyRecord` 구조체 정의
- [ ] `error.rs`: `CypherLiteError` enum 정의 (thiserror 기반)
- [ ] `config.rs`: `DatabaseConfig` 구조체 (page_size, cache_capacity, wal_sync_mode)
- [ ] `traits.rs`: `Transaction` trait (begin, commit, rollback, is_active)
- [ ] `lib.rs`: 모듈 export 구성

**TDD 전략**: 각 타입에 대해 생성, 직렬화/역직렬화, 경계값 테스트를 먼저 작성한다.

**의존성**: 없음 (첫 번째 구현 대상)

---

### M2: 페이지 관리자 및 파일 형식 (Primary Goal)

**크레이트**: `cypherlite-storage`

**태스크**:
- [ ] `page/mod.rs`: `PageType` enum (Header, Data, BTreeInterior, BTreeLeaf, Overflow, FreeSpaceMap, WALFrame)
- [ ] `page/mod.rs`: `PageHeader` 구조체 (32바이트 고정 레이아웃)
- [ ] `page/page_manager.rs`: `PageManager` 구조체
  - `create_database()`: 새 `.cyl` 파일 생성 (Header Page + Free Space Map 초기화)
  - `open_database()`: 기존 파일 오픈 (매직 넘버, 버전 검증)
  - `allocate_page()`: Free Space Map에서 빈 페이지 할당
  - `deallocate_page()`: 페이지 해제 (비트맵 업데이트)
  - `read_page()`: 디스크에서 페이지 읽기
  - `write_page()`: 디스크에 페이지 쓰기

**TDD 전략**:
- 파일 생성 후 매직 넘버/버전 검증 테스트
- 페이지 할당/해제 왕복 테스트
- Free Space Map 비트맵 정확성 테스트
- 잘못된 매직 넘버로 오픈 시 에러 반환 테스트

**의존성**: M1

---

### M3: 버퍼 풀 (Primary Goal)

**크레이트**: `cypherlite-storage`

**태스크**:
- [ ] `page/buffer_pool.rs`: `BufferPool` 구조체
  - `new(capacity: usize)`: LRU 캐시 초기화 (기본 256 페이지)
  - `fetch_page(page_id)`: 캐시 히트 시 반환, 미스 시 디스크 로드
  - `pin_page(page_id)`: 퇴거 방지
  - `unpin_page(page_id)`: 퇴거 허용
  - `mark_dirty(page_id)`: dirty 플래그 설정
  - `flush_page(page_id)`: dirty 페이지를 WAL에 기록
  - `evict()`: LRU 기반 퇴거 (pinned 페이지 제외)
- [ ] `page/buffer_pool.rs`: `parking_lot::RwLock` 기반 동시 접근 제어

**TDD 전략**:
- 캐시 히트/미스 시나리오 테스트
- LRU 퇴거 순서 검증
- pinned 페이지 퇴거 방지 테스트
- 전체 pin 상태에서 `OutOfSpace` 에러 테스트
- 동시 접근 (multi-thread) 안전성 테스트

**의존성**: M2

---

### M4: WAL Writer & Reader (Primary Goal)

**크레이트**: `cypherlite-storage`

**태스크**:
- [ ] `wal/mod.rs`: `WalFrame` 구조체 (frame_number, page_number, db_size, salt, checksum, page_data)
- [ ] `wal/mod.rs`: `WalHeader` 구조체 (magic, checksum, salt, frame_count)
- [ ] `wal/writer.rs`: `WalWriter` 구조체
  - `write_frame(page_id, page_data)`: WAL 프레임 추가 + fsync
  - `commit(tx_id)`: 커밋 마커 기록
  - `discard(tx_id)`: 미커밋 프레임 폐기
- [ ] `wal/reader.rs`: `WalReader` 구조체
  - WAL 메모리 인덱스 (`DashMap<PageId, FrameOffset>`)
  - `read_page(page_id)`: WAL 인덱스 확인 후 WAL 또는 주 파일에서 읽기
- [ ] `wal/checkpoint.rs`: `Checkpoint` 구현
  - `run()`: 커밋된 프레임을 주 파일로 복사, 프레임 카운터 리셋

**TDD 전략**:
- WAL 프레임 쓰기/읽기 왕복 테스트
- checksum 검증 테스트
- 체크포인트 후 주 파일 일관성 테스트
- 잘못된 checksum 프레임 거부 테스트

**의존성**: M2

---

### M5: B-tree (Node/Edge Store) (Secondary Goal)

**크레이트**: `cypherlite-storage`

**태스크**:
- [ ] `btree/mod.rs`: `BTree<K: Ord>` 제네릭 B-tree 구현
  - `insert(key, value)`: 리프 노드에 삽입, 오버플로우 시 분할(split)
  - `search(key)`: 루트에서 리프까지 O(log n) 탐색
  - `delete(key)`: 키 삭제, 언더플로우 시 병합(merge)/재분배
  - `range_scan(start, end)`: 범위 스캔 이터레이터
- [ ] `btree/node_store.rs`: `NodeStore` (B-tree 위에 Node CRUD 추상화)
  - `create_node(labels, properties) -> NodeId`
  - `get_node(node_id) -> Option<NodeRecord>`
  - `update_node(node_id, properties)`
  - `delete_node(node_id)` (연결된 엣지 삭제 포함)
- [ ] `btree/edge_store.rs`: `EdgeStore` (B-tree 위에 Edge CRUD 추상화)
  - `create_edge(start, end, rel_type, properties) -> EdgeId`
  - `get_edge(edge_id) -> Option<RelationshipRecord>`
  - `get_edges_for_node(node_id) -> Vec<RelationshipRecord>` (인접 체인 순회)
  - `delete_edge(edge_id)` (인접 체인 포인터 갱신 포함)
- [ ] `btree/property_store.rs`: 인라인/overflow 프로퍼티 관리

**TDD 전략**:
- B-tree 삽입/검색/삭제 기본 연산 테스트
- 페이지 분할(split) 및 병합(merge) 시나리오 테스트
- 노드 생성/조회/수정/삭제 왕복 테스트
- 엣지 생성 시 인접 체인 업데이트 검증
- 노드 삭제 시 연결된 엣지 자동 삭제 검증
- 대량 삽입 후 B-tree 균형 유지 검증 (proptest)
- overflow 프로퍼티 저장/읽기 테스트

**의존성**: M3, M4

---

### M6: 크래시 복구 (Secondary Goal)

**크레이트**: `cypherlite-storage`

**태스크**:
- [ ] `wal/recovery.rs`: `Recovery` 구현
  - `recover(wal_path, db_path)`: WAL 스캔 -> 커밋된 프레임 재생 -> 미커밋 프레임 폐기
  - 부분 기록된 프레임 감지 (checksum 불일치)
  - 복구 완료 후 WAL 리셋

**TDD 전략**:
- 정상 종료 후 WAL 재생 테스트 (no-op 확인)
- 커밋 후 크래시 시뮬레이션: WAL 재생으로 데이터 복원 확인
- 미커밋 상태 크래시 시뮬레이션: 미커밋 데이터 폐기 확인
- 부분 기록 WAL 프레임 처리 테스트
- 손상된 WAL 파일 처리 테스트

**의존성**: M4

---

### M7: 트랜잭션 매니저 (MVCC) (Final Goal)

**크레이트**: `cypherlite-storage`

**태스크**:
- [ ] `transaction/mod.rs`: `TransactionManager` 구조체
  - `begin_read() -> ReadTransaction`: 읽기 전용 트랜잭션 (스냅샷 생성)
  - `begin_write() -> WriteTransaction`: 쓰기 트랜잭션 (배타적 잠금 획득)
- [ ] `transaction/mvcc.rs`: MVCC 구현
  - `ReadTransaction`: WAL 프레임 인덱스 스냅샷 기반 일관된 읽기
  - `WriteTransaction`: WAL 쓰기 + 커밋/롤백
  - 쓰기 잠금: `parking_lot::Mutex` 기반

**TDD 전략**:
- 읽기 트랜잭션 스냅샷 격리 테스트
- 쓰기 트랜잭션 커밋 후 가시성 테스트
- 롤백 후 변경 사항 폐기 확인
- 동시 읽기 + 쓰기 간섭 없음 테스트
- 쓰기 잠금 경합 시 `TransactionConflict` 에러 테스트

**의존성**: M4, M5

---

### M8: ACID 통합 검증 (Final Goal)

**크레이트**: `tests/integration/`

**태스크**:
- [ ] `acid_compliance.rs`: 종합 ACID 속성 테스트
  - Atomicity: 트랜잭션 중간 크래시 시 롤백 확인
  - Consistency: 노드/엣지 참조 무결성 검증
  - Isolation: 동시 읽기/쓰기 간 격리 확인
  - Durability: 커밋 후 프로세스 재시작 시 데이터 보존 확인
- [ ] `concurrency.rs`: 멀티 스레드 동시성 테스트
- [ ] 벤치마크: `benches/storage_bench.rs` (criterion 기반)

**의존성**: M5, M6, M7

---

## 2. 파일 구조

### 워크스페이스 Cargo.toml

```toml
[workspace]
members = [
    "crates/cypherlite-core",
    "crates/cypherlite-storage",
]
resolver = "2"
```

### cypherlite-core 구조

```
crates/cypherlite-core/
├── Cargo.toml
└── src/
    ├── lib.rs
    ├── types.rs          # NodeId, EdgeId, PageId, PropertyValue, NodeRecord, RelationshipRecord
    ├── error.rs          # CypherLiteError (thiserror)
    ├── config.rs         # DatabaseConfig
    └── traits.rs         # Transaction trait
```

### cypherlite-storage 구조

```
crates/cypherlite-storage/
├── Cargo.toml
└── src/
    ├── lib.rs            # StorageEngine 통합 진입점
    ├── page/
    │   ├── mod.rs        # PageType, PageHeader
    │   ├── page_manager.rs   # 페이지 할당/해제, 파일 형식
    │   └── buffer_pool.rs    # LRU 캐시, pin/unpin, eviction
    ├── wal/
    │   ├── mod.rs        # WalFrame, WalHeader
    │   ├── writer.rs     # WAL 프레임 쓰기, fsync
    │   ├── reader.rs     # WAL 인덱스, 페이지 읽기 라우팅
    │   ├── checkpoint.rs # WAL -> 주 파일 체크포인트
    │   └── recovery.rs   # 크래시 복구 (WAL 재생)
    ├── btree/
    │   ├── mod.rs        # 제네릭 BTree<K>
    │   ├── node_store.rs # Node CRUD (NodeId -> NodeRecord)
    │   ├── edge_store.rs # Edge CRUD (EdgeId -> RelationshipRecord)
    │   └── property_store.rs  # 인라인/overflow 프로퍼티
    └── transaction/
        ├── mod.rs        # TransactionManager
        └── mvcc.rs       # ReadTransaction, WriteTransaction, MVCC
```

### 테스트 및 벤치마크 구조

```
tests/
├── integration/
│   ├── acid_compliance.rs    # ACID 속성 종합 검증
│   ├── concurrency.rs        # 멀티 스레드 동시성 안전성
│   └── crud_operations.rs    # 노드/엣지 CRUD 통합 테스트
└── fixtures/
    └── (테스트 데이터 파일)

benches/
└── storage_bench.rs          # criterion 기반 성능 벤치마크
```

---

## 3. 핵심 Rust 구조체 및 Trait

### Core 타입 (cypherlite-core)

```
NodeId(u64)                    // 노드 식별자 (newtype)
EdgeId(u64)                    // 엣지 식별자 (newtype)
PageId(u32)                    // 페이지 식별자 (newtype)

PropertyValue                  // enum: Null, Bool(bool), Int64(i64), Float64(f64),
                               //       String(String), Bytes(Vec<u8>), Array(Vec<PropertyValue>)

NodeRecord {
    node_id: NodeId,
    labels: Vec<u32>,          // label string ID
    properties: Vec<(u32, PropertyValue)>,
    next_edge_id: EdgeId,      // 인접 체인 헤드
    overflow_page: Option<PageId>,
}

RelationshipRecord {
    edge_id: EdgeId,
    start_node: NodeId,
    end_node: NodeId,
    rel_type_id: u32,
    direction: Direction,      // Outgoing(0), Incoming(1), Both(2)
    next_out_edge: EdgeId,
    next_in_edge: EdgeId,
    properties: Vec<(u32, PropertyValue)>,
}

CypherLiteError               // enum: IoError, CorruptedPage, TransactionConflict,
                               //       OutOfSpace, InvalidMagicNumber, UnsupportedVersion,
                               //       ChecksumMismatch

DatabaseConfig {
    page_size: u32,            // 4096 (고정)
    cache_capacity: usize,     // 기본 256
    wal_sync_mode: SyncMode,   // Full, Normal
}

trait Transaction {
    fn begin(&mut self) -> Result<(), CypherLiteError>;
    fn commit(&mut self) -> Result<(), CypherLiteError>;
    fn rollback(&mut self) -> Result<(), CypherLiteError>;
    fn is_active(&self) -> bool;
}
```

### Storage 구조체 (cypherlite-storage)

```
PageManager                    // 파일 I/O, 페이지 할당/해제
BufferPool                     // LRU 캐시, pin/unpin, dirty 추적
WalWriter                      // WAL 프레임 쓰기, fsync
WalReader                      // WAL 인덱스 + 주 파일 읽기 라우팅
Checkpoint                     // WAL -> 주 파일 복사
Recovery                       // WAL 재생, 크래시 복구
BTree<K>                       // 제네릭 B-tree (insert, search, delete, range_scan)
NodeStore                      // Node B-tree CRUD 추상화
EdgeStore                      // Edge B-tree CRUD + 인접 체인
PropertyStore                  // 인라인/overflow 프로퍼티 관리
TransactionManager             // 트랜잭션 팩토리
ReadTransaction                // 읽기 전용 스냅샷 트랜잭션
WriteTransaction               // 쓰기 트랜잭션 (배타적 잠금)
StorageEngine                  // 통합 진입점 (open, close, node_store, edge_store, tx)
```

---

## 4. 테스트 전략 (TDD)

### TDD 워크플로우

Phase 1은 `quality.yaml`에 따라 **TDD (RED-GREEN-REFACTOR)** 모드로 진행한다.

1. **RED**: 실패하는 테스트를 먼저 작성한다
2. **GREEN**: 테스트를 통과하는 최소한의 구현을 작성한다
3. **REFACTOR**: 테스트를 유지하면서 코드를 개선한다

### 테스트 분류

| 카테고리 | 도구 | 대상 |
|----------|------|------|
| 단위 테스트 | `#[cfg(test)]` | 각 모듈 내 함수/구조체 |
| 통합 테스트 | `tests/integration/` | 크레이트 간 상호작용 |
| 속성 기반 테스트 | `proptest` | B-tree 균형, 직렬화 왕복 |
| 벤치마크 | `criterion` | 쓰기 처리량, 읽기 지연 |

### 테스트 커버리지 목표

- **전체 커버리지**: 85% 이상 (quality.yaml 기준)
- **크리티컬 경로**: 95% 이상 (WAL, 트랜잭션, 크래시 복구)
- **커버리지 도구**: `cargo-tarpaulin` (Linux) 또는 `cargo-llvm-cov`

### 속성 기반 테스트 대상

- B-tree: 임의의 키 삽입/삭제 순서에서 항상 올바른 검색 결과
- 직렬화: `NodeRecord`/`RelationshipRecord` 직렬화 -> 역직렬화 = 원본
- WAL: 임의 순서의 쓰기/커밋/롤백 후 데이터 무결성

---

## 5. Rust 크레이트 의존성

### cypherlite-core

```toml
[dependencies]
thiserror = "1"
serde = { version = "1", features = ["derive"] }
bincode = "1"

[dev-dependencies]
proptest = "1"
```

### cypherlite-storage

```toml
[dependencies]
cypherlite-core = { path = "../cypherlite-core" }
parking_lot = "0.12"
crossbeam = "0.8"
dashmap = "5"
bincode = "1"

[dev-dependencies]
tempfile = "3"
criterion = { version = "0.5", features = ["html_reports"] }
proptest = "1"

[[bench]]
name = "storage_bench"
harness = false
```

---

## 6. 리스크 분석

### High Priority 리스크

| 리스크 | 영향 | 대응 전략 |
|--------|------|-----------|
| **WAL 구현 버그로 데이터 유실** | 치명적 (ACID 위반) | 크래시 시뮬레이션 테스트 집중 작성, proptest로 임의 시나리오 검증, fsync 호출 검증 |
| **B-tree 분할/병합 로직 오류** | 높음 (데이터 손상) | 경계값 테스트 집중, proptest로 임의 삽입/삭제 시퀀스 검증, 트리 무결성 검증 함수 구현 |
| **동시성 버그 (race condition)** | 높음 (데이터 불일치) | `parking_lot` 기반 잠금 설계, `RUSTFLAGS="-Z sanitizer=thread"` 로 race 탐지, `loom` 크레이트 검토 |

### Medium Priority 리스크

| 리스크 | 영향 | 대응 전략 |
|--------|------|-----------|
| **성능 목표 미달 (< 1000 노드/초)** | 중간 | criterion 벤치마크로 조기 측정, 프로파일링 도구(`flamegraph`) 활용, 핫 패스 최적화 |
| **버퍼 풀 메모리 관리 누수** | 중간 | Rust의 소유권 시스템 활용, `Valgrind` 또는 `miri`로 메모리 안전성 검증 |
| **직렬화 호환성 문제** | 중간 | bincode 버전 고정, 파일 형식 버전 필드로 하위 호환성 관리 |

### Low Priority 리스크

| 리스크 | 영향 | 대응 전략 |
|--------|------|-----------|
| **플랫폼 간 fsync 동작 차이** | 낮음 | 플랫폼별 fsync 래퍼, CI에서 Linux + macOS + Windows 테스트 |
| **대용량 파일에서 Free Space Map 비트맵 부족** | 낮음 | Phase 1에서는 단일 FSM 페이지 (32,768 페이지 = 128MB 제한), 필요 시 다중 FSM 페이지로 확장 |

---

## 7. 성능 검증 접근 방식

### 벤치마크 항목

| 벤치마크 | 측정 대상 | 목표 |
|----------|-----------|------|
| `bench_node_write_sequential` | 노드 순차 생성 처리량 | > 1,000 노드/초 |
| `bench_node_read_cached` | 캐시 히트 노드 읽기 지연 | < 1ms |
| `bench_node_read_uncached` | 캐시 미스 노드 읽기 지연 | < 10ms |
| `bench_wal_write_fsync` | WAL 프레임 쓰기 + fsync 지연 | < 5ms |
| `bench_checkpoint` | 체크포인트 처리량 | < 500ms (1000 프레임) |
| `bench_crash_recovery` | 크래시 복구 시간 | < 1초 (1000 프레임) |
| `bench_concurrent_read_write` | 동시 읽기/쓰기 처리량 | 읽기 성능 저하 < 10% |

### 프로파일링 도구

- **flamegraph**: CPU 프로파일링으로 핫 패스 식별
- **perf** (Linux): 시스템 수준 성능 분석
- **criterion**: 통계적 유의성 있는 벤치마크 비교
- **Instruments** (macOS): I/O 및 메모리 프로파일링

### 성능 최적화 전략

1. **조기 측정**: M3 (버퍼 풀) 완료 후 첫 번째 벤치마크 실행
2. **점진적 최적화**: 각 마일스톤 완료 후 벤치마크 기준선 업데이트
3. **Release 빌드**: 성능 측정은 항상 `--release` 플래그로 실행
4. **I/O 최적화**: 페이지 정렬된 I/O, 배치 fsync 검토

---

## 8. 구현 접근 방식 요약

### 핵심 원칙

1. **TDD 우선**: 모든 구현은 실패하는 테스트로 시작한다
2. **상향식 구축**: 하위 레이어(Core -> Page -> Buffer -> WAL -> B-tree -> TX)부터 구축
3. **안전성 우선**: `unsafe` 사용 최소화, Rust 소유권 시스템 최대 활용
4. **조기 벤치마크**: 성능 문제를 초기에 발견하여 아키텍처 변경 비용 최소화
5. **속성 기반 테스트**: 크리티컬 데이터 구조(B-tree, WAL)는 proptest로 검증

### Phase 2와의 인터페이스

Phase 1의 `StorageEngine`은 다음 API를 Phase 2 (쿼리 엔진)에 노출한다:

```
StorageEngine::open(path) -> Result<StorageEngine>
StorageEngine::node_store() -> &NodeStore
StorageEngine::edge_store() -> &EdgeStore
StorageEngine::begin_read() -> ReadTransaction
StorageEngine::begin_write() -> WriteTransaction
```

이 인터페이스는 Phase 2의 쿼리 실행기가 스토리지 레이어에 접근하는 유일한 경로이다.
