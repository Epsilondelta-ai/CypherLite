---
id: SPEC-DB-001
version: "1.0.0"
status: completed
created: "2026-03-10"
updated: "2026-03-10"
author: epsilondelta
priority: critical
tags: [storage-engine, acid, wal, btree, buffer-pool, transaction]
lifecycle: spec-anchored
---

# SPEC-DB-001: CypherLite Phase 1 - Storage Engine (v0.1)

> CypherLite의 기반 스토리지 레이어. 쿼리 엔진 없이 순수 스토리지 메커니즘으로 노드, 엣지, 프로퍼티에 대한 CRUD 연산과 완전한 ACID 준수를 제공한다.

---

## HISTORY

| Version | Date | Changes |
|---------|------|---------|
| 1.0.0 | 2026-03-10 | Initial SPEC creation |

---

## 1. Environment (환경)

### 1.1 시스템 환경

- **언어**: Rust 1.70+ (Edition 2021)
- **대상 플랫폼**: Linux (x86_64), macOS (x86_64, aarch64), Windows (x86_64)
- **파일 시스템**: POSIX 호환 또는 Windows NTFS (fsync/FlushFileBuffers 지원 필수)
- **실행 모델**: 임베디드 라이브러리 (인-프로세스, 별도 서버 없음)

### 1.2 파일 형식 환경

- **주 데이터 파일**: `.cyl` 확장자 (4KB 페이지 기반)
- **WAL 파일**: `.cyl-wal` 확장자 (Write-Ahead Log)
- **매직 넘버**: `CYLT` (0x43594C54)
- **페이지 크기**: 4,096 바이트 (고정)

### 1.3 동시성 환경

- **모델**: Single-Writer, Multiple-Reader (SQLite와 동일)
- **잠금**: `parking_lot::RwLock` (버퍼 풀), `parking_lot::Mutex` (WAL 쓰기)
- **격리 수준**: Snapshot Isolation (WAL frame index 기반)

### 1.4 핵심 의존성

| 크레이트 | 버전 | 용도 |
|----------|------|------|
| `parking_lot` | 0.12 | RwLock, Mutex |
| `crossbeam` | 0.8 | 채널, 동시성 유틸리티 |
| `dashmap` | 6 | WAL 인덱스용 동시 해시맵 (spec v5에서 업그레이드, 성능 개선) |
| `bincode` | 1 | 이진 직렬화 |
| `thiserror` | 2 | 에러 타입 정의 (spec v1에서 업그레이드) |
| `tempfile` | 3 | 테스트용 임시 파일 |
| `criterion` | 0.5 | 벤치마크 |
| `proptest` | 1 | 속성 기반 테스트 |

---

## 2. Assumptions (가정)

### 2.1 기술 가정

- **A-001**: 호스트 OS가 `fsync` (POSIX) 또는 `FlushFileBuffers` (Windows) 시스템콜을 지원한다.
- **A-002**: 페이지 크기 4KB는 대부분의 OS 파일 시스템 블록 크기와 정렬된다.
- **A-003**: 단일 `.cyl` 파일 크기가 현재 Phase에서 2^32 페이지 (16TB)를 초과하지 않는다.
- **A-004**: 단일 프로퍼티 값은 대부분 31바이트 이내이며, 초과 시 overflow 페이지를 사용한다.

### 2.2 사용 패턴 가정

- **A-005**: 읽기 연산이 쓰기 연산보다 압도적으로 많다 (읽기 우세 워크로드).
- **A-006**: 동시 쓰기 요청은 단일 Writer 락으로 직렬화 처리해도 성능 목표를 달성할 수 있다.
- **A-007**: Phase 1에서는 Cypher 쿼리 엔진 없이 Rust Native API만 제공한다.

### 2.3 제약 가정

- **A-008**: 네트워크 I/O는 Phase 1 범위에 포함되지 않는다 (순수 로컬 파일 접근).
- **A-009**: 플러그인 시스템은 Phase 3에서 도입되므로, Phase 1에서는 확장 포인트만 trait로 정의한다.

---

## 3. Requirements (요구사항)

### 모듈 1: 파일 형식 및 페이지 관리 (REQ-PAGE)

#### REQ-PAGE-001 [Ubiquitous]
시스템은 **항상** 4,096바이트(4KB) 고정 크기 페이지 단위로 데이터를 읽고 쓴다.

#### REQ-PAGE-002 [Ubiquitous]
시스템은 **항상** 페이지 0에 Header Page를 유지하며, 매직 넘버 `CYLT`(0x43594C54), 파일 형식 버전, 루트 페이지 포인터, 전체 페이지 수를 포함한다.

#### REQ-PAGE-003 [Ubiquitous]
시스템은 **항상** 페이지 1에 Free Space Map을 유지하여 비트맵으로 사용/미사용 페이지를 추적한다.

#### REQ-PAGE-004 [Event-Driven]
**WHEN** 새 페이지 할당 요청이 발생하면 **THEN** 시스템은 Free Space Map에서 첫 번째 미사용 비트를 찾아 해당 페이지를 할당하고 비트맵을 업데이트한다.

#### REQ-PAGE-005 [Event-Driven]
**WHEN** 페이지 해제 요청이 발생하면 **THEN** 시스템은 Free Space Map의 해당 비트를 미사용으로 표시한다.

#### REQ-PAGE-006 [Ubiquitous]
시스템은 **항상** 각 페이지 헤더(32바이트)에 page_type(u8), flags(u8), free_start(u16), free_end(u16), overflow_page(u32), item_count(u16)를 포함한다.

#### REQ-PAGE-007 [Unwanted]
시스템은 매직 넘버가 `CYLT`와 일치하지 않는 파일을 데이터베이스로 **열지 않아야 한다**.

#### REQ-PAGE-008 [Unwanted]
시스템은 지원하지 않는 파일 형식 버전을 가진 `.cyl` 파일을 **열지 않아야 한다**.

---

### 모듈 2: 버퍼 풀 및 캐시 관리 (REQ-BUF)

#### REQ-BUF-001 [Ubiquitous]
시스템은 **항상** LRU(Least Recently Used) 정책에 따라 페이지를 캐시하며, 기본 캐시 용량은 256 페이지(1MB)로 설정한다.

#### REQ-BUF-002 [Event-Driven]
**WHEN** 요청된 페이지가 버퍼 풀에 존재하지 않으면 **THEN** 시스템은 디스크(또는 WAL)에서 해당 페이지를 읽어 캐시에 적재한다.

#### REQ-BUF-003 [Event-Driven]
**WHEN** 버퍼 풀이 가득 차고 새 페이지 적재가 필요하면 **THEN** 시스템은 pinned 상태가 아닌 페이지 중 LRU 페이지를 선택하여 퇴거(evict)한다.

#### REQ-BUF-004 [Event-Driven]
**WHEN** dirty 페이지가 퇴거 대상이 되면 **THEN** 시스템은 해당 페이지를 WAL에 기록한 후 퇴거를 완료한다.

#### REQ-BUF-005 [Ubiquitous]
시스템은 **항상** 활성 트랜잭션에서 사용 중인 페이지를 pin 상태로 유지하여 퇴거를 방지한다.

#### REQ-BUF-006 [Complex]
**IF** 모든 버퍼 풀 페이지가 pin 상태이고 **WHEN** 새 페이지 적재가 요청되면 **THEN** 시스템은 `OutOfSpace` 에러를 반환한다.

#### REQ-BUF-007 [Optional]
**가능하면** 시스템은 Config를 통해 버퍼 풀 용량을 사용자 지정 가능하게 제공한다.

---

### 모듈 3: 노드/엣지 CRUD 및 프로퍼티 스토리지 (REQ-STORE)

#### REQ-STORE-001 [Event-Driven]
**WHEN** 노드 생성 요청이 발생하면 **THEN** 시스템은 고유한 `NodeId`(u64)를 할당하고, 레이블 목록과 프로퍼티를 포함하는 `NodeRecord`를 Node B-tree에 삽입한다.

#### REQ-STORE-002 [Event-Driven]
**WHEN** `NodeId`로 노드 조회 요청이 발생하면 **THEN** 시스템은 Node B-tree에서 해당 레코드를 O(log n) 시간 내에 검색하여 반환한다.

#### REQ-STORE-003 [Event-Driven]
**WHEN** 노드 업데이트 요청이 발생하면 **THEN** 시스템은 Node B-tree에서 해당 레코드를 찾아 프로퍼티를 갱신한다.

#### REQ-STORE-004 [Event-Driven]
**WHEN** 노드 삭제 요청이 발생하면 **THEN** 시스템은 해당 노드와 연결된 모든 엣지를 먼저 삭제한 후 Node B-tree에서 레코드를 제거한다.

#### REQ-STORE-005 [Event-Driven]
**WHEN** 엣지 생성 요청이 발생하면 **THEN** 시스템은 고유한 `EdgeId`(u64)를 할당하고, `start_node`, `end_node`, `rel_type_id`를 포함하는 `RelationshipRecord`를 Edge B-tree에 삽입하며, 양쪽 노드의 인접 체인(`next_out_edge`, `next_in_edge`)을 업데이트한다.

#### REQ-STORE-006 [Event-Driven]
**WHEN** `EdgeId`로 엣지 조회 요청이 발생하면 **THEN** 시스템은 Edge B-tree에서 해당 레코드를 O(log n) 시간 내에 검색하여 반환한다.

#### REQ-STORE-007 [Event-Driven]
**WHEN** 특정 노드의 인접 엣지 탐색 요청이 발생하면 **THEN** 시스템은 해당 노드의 `next_edge_id`로부터 인접 체인(linked list)을 순회하여 연결된 모든 엣지를 반환한다 (Index-Free Adjacency).

#### REQ-STORE-008 [Event-Driven]
**WHEN** 엣지 삭제 요청이 발생하면 **THEN** 시스템은 Edge B-tree에서 레코드를 제거하고, 양쪽 노드의 인접 체인에서 해당 엣지 포인터를 갱신한다.

#### REQ-STORE-009 [Ubiquitous]
시스템은 **항상** 프로퍼티를 key_id(u32) + type_tag(u8) + value(최대 31바이트) 형태로 인라인 저장한다.

#### REQ-STORE-010 [Event-Driven]
**WHEN** 프로퍼티 값이 31바이트를 초과하면 **THEN** 시스템은 overflow 페이지에 값을 저장하고 인라인 영역에 overflow 페이지 포인터를 기록한다.

#### REQ-STORE-011 [Ubiquitous]
시스템은 **항상** 다음 프로퍼티 타입을 지원한다: Null(0), Bool(1), Int64(2), Float64(3), String(4), Bytes(5), Array(6).

#### REQ-STORE-012 [Ubiquitous]
시스템은 **항상** Node B-tree와 Edge B-tree를 분기 계수(branching factor) 약 100 (4KB 페이지 기준)으로 유지하며, interior 노드와 leaf 노드를 구분한다.

---

### 모듈 4: WAL 및 체크포인트 (REQ-WAL)

#### REQ-WAL-001 [Ubiquitous]
시스템은 **항상** 데이터 변경을 주 파일(.cyl)에 직접 쓰지 않고, WAL 파일(.cyl-wal)에 먼저 기록한다 (Write-Ahead Logging).

#### REQ-WAL-002 [Event-Driven]
**WHEN** WAL 프레임 쓰기 요청이 발생하면 **THEN** 시스템은 frame_number, page_number, db_size, salt, checksum, page_data(4KB)를 포함하는 WAL 프레임을 기록하고 `fsync`를 수행한다.

#### REQ-WAL-003 [Event-Driven]
**WHEN** 페이지 읽기 요청이 발생하면 **THEN** 시스템은 WAL 메모리 인덱스를 먼저 확인하여 해당 페이지의 최신 프레임이 존재하면 WAL에서 읽고, 없으면 주 파일에서 읽는다.

#### REQ-WAL-004 [Event-Driven]
**WHEN** 체크포인트가 트리거되면 **THEN** 시스템은 커밋된 WAL 프레임의 페이지 데이터를 주 파일로 복사하고, 완료 후 WAL 프레임 카운터를 재설정한다.

#### REQ-WAL-005 [Complex]
**IF** 체크포인트가 진행 중인 상태에서 **WHEN** 읽기 요청이 발생하면 **THEN** 시스템은 체크포인트 완료까지 WAL 인덱스를 계속 사용하여 일관된 읽기를 보장한다.

#### REQ-WAL-006 [Ubiquitous]
시스템은 **항상** WAL 헤더에 magic, checksum, salt, frame_count를 유지하여 WAL 파일의 무결성을 검증한다.

#### REQ-WAL-007 [Unwanted]
시스템은 checksum이 불일치하는 WAL 프레임을 **적용하지 않아야 한다**.

---

### 모듈 5: 트랜잭션 및 ACID 준수 (REQ-TX)

#### REQ-TX-001 [Event-Driven]
**WHEN** 트랜잭션 시작(begin) 요청이 발생하면 **THEN** 시스템은 현재 WAL 프레임 인덱스의 스냅샷을 기록하여 읽기 일관성 기점을 설정한다.

#### REQ-TX-002 [Event-Driven]
**WHEN** 트랜잭션 커밋(commit) 요청이 발생하면 **THEN** 시스템은 모든 미기록 WAL 프레임을 디스크에 `fsync`하고, WAL 인덱스를 업데이트하여 커밋을 원자적으로 완료한다.

#### REQ-TX-003 [Event-Driven]
**WHEN** 트랜잭션 롤백(rollback) 요청이 발생하면 **THEN** 시스템은 해당 트랜잭션의 미커밋 WAL 프레임을 폐기하고, 버퍼 풀에서 dirty 페이지를 무효화한다.

#### REQ-TX-004 [Ubiquitous] - Atomicity
시스템은 **항상** 트랜잭션 내 모든 변경을 원자적으로 처리한다. WAL 프레임은 전부 커밋되거나 전부 폐기된다 (All-or-Nothing).

#### REQ-TX-005 [Ubiquitous] - Consistency
시스템은 **항상** 커밋 전에 내부 제약 조건(노드/엣지 참조 무결성, 페이지 구조 유효성)을 검증한다.

#### REQ-TX-006 [Ubiquitous] - Isolation
시스템은 **항상** Snapshot Isolation을 제공하여, 읽기 트랜잭션은 시작 시점의 일관된 데이터 스냅샷을 보고 쓰기 트랜잭션에 의해 차단되지 않는다.

#### REQ-TX-007 [Ubiquitous] - Durability
시스템은 **항상** 커밋된 트랜잭션의 WAL 프레임에 대해 `fsync`를 수행하여, 프로세스 크래시 또는 전원 장애 후에도 데이터가 보존됨을 보장한다.

#### REQ-TX-008 [Event-Driven] - Crash Recovery
**WHEN** 비정상 종료 후 데이터베이스를 다시 열면 **THEN** 시스템은 WAL 파일을 스캔하여 커밋된 프레임을 재생(replay)하고, 미커밋 프레임을 폐기하여 일관된 상태를 복원한다.

#### REQ-TX-009 [Ubiquitous]
시스템은 **항상** 쓰기 트랜잭션에 배타적 잠금(exclusive lock)을 적용하여 동시에 하나의 Writer만 존재하도록 보장한다.

#### REQ-TX-010 [Complex]
**IF** 쓰기 잠금을 획득할 수 없는 상태에서 **WHEN** 쓰기 트랜잭션 시작 요청이 발생하면 **THEN** 시스템은 `TransactionConflict` 에러를 반환한다.

#### REQ-TX-011 [Unwanted]
시스템은 커밋되지 않은 트랜잭션의 변경 사항을 다른 트랜잭션에서 **읽을 수 없어야 한다**.

---

## 4. Specifications (명세)

### 4.1 에러 타입 명세

| 에러 타입 | 설명 |
|-----------|------|
| `IoError(std::io::Error)` | 파일 시스템 I/O 에러 |
| `CorruptedPage { page_id: u32, reason: String }` | 손상된 페이지 감지 |
| `TransactionConflict` | 쓰기 잠금 경합 |
| `OutOfSpace` | 버퍼 풀 또는 디스크 공간 부족 |
| `InvalidMagicNumber` | 잘못된 매직 넘버 |
| `UnsupportedVersion { found: u32, supported: u32 }` | 지원하지 않는 파일 버전 |
| `ChecksumMismatch { expected: u64, found: u64 }` | 체크섬 불일치 |

### 4.2 성능 목표

| 지표 | 목표값 | 측정 조건 |
|------|--------|-----------|
| 순차 노드 쓰기 속도 | > 1,000 노드/초 | 프로퍼티 3개 포함, 릴리즈 빌드 |
| 단일 노드 읽기 (캐시 히트) | < 1ms | 버퍼 풀 내 페이지 |
| 단일 노드 읽기 (캐시 미스) | < 10ms | 디스크 접근 포함 |
| WAL 프레임 쓰기 + fsync | < 5ms | 단일 페이지 기준 |
| 체크포인트 (1000 프레임) | < 500ms | SSD 기준 |
| 크래시 복구 시간 | < 1초 | 1000 WAL 프레임 기준 |

### 4.3 크레이트 범위

| 크레이트 | Phase 1 범위 |
|----------|-------------|
| `cypherlite-core` | NodeId, EdgeId, PropertyValue, CypherLiteError, Config, Transaction trait |
| `cypherlite-storage` | StorageEngine, BufferPool, WAL, B-tree (Node/Edge), MVCC Transaction Manager |

### 4.4 Traceability (추적성)

| 요구사항 | 구현 대상 | 테스트 대상 |
|----------|-----------|-------------|
| REQ-PAGE-* | `cypherlite-storage::page` | `tests/unit/page_tests.rs` |
| REQ-BUF-* | `cypherlite-storage::page::buffer_pool` | `tests/unit/buffer_pool_tests.rs` |
| REQ-STORE-* | `cypherlite-storage::btree` | `tests/unit/btree_tests.rs`, `tests/integration/crud_tests.rs` |
| REQ-WAL-* | `cypherlite-storage::wal` | `tests/unit/wal_tests.rs`, `tests/integration/recovery_tests.rs` |
| REQ-TX-* | `cypherlite-storage::transaction` | `tests/integration/acid_compliance.rs` |

---

## 5. Implementation Notes

### 5.1 Dependency Divergence from Original Spec

The following dependency versions were changed during the strategy review phase (Phase 1, manager-strategy):

| Dependency | Spec Version | Actual Version | Reason |
|------------|-------------|----------------|--------|
| `dashmap` | 5 | 6 | Performance improvements in v6; API-compatible upgrade |
| `thiserror` | 1 | 2 | Cleaner derive macro in v2; no breaking changes for this usage |

**MSRV**: Rust 1.84+ (original spec stated Rust 1.70+). The actual implementation uses features available in 1.84+ (e.g., `Option::is_none_or`). The minimum supported Rust version was updated during Phase 2B (TDD implementation).

### 5.2 Implementation Status

- **Total tests**: 207 (36 unit in cypherlite-core, 146 unit in cypherlite-storage, 25 integration)
- **Test coverage**: 96.82% line coverage (target: 85%)
- **TRUST 5**: All gates passed (Tested, Readable, Unified, Secured, Trackable)
- **MSRV**: Rust 1.84+

### 5.3 MX Annotations

| Tag | File | Description |
|-----|------|-------------|
| `@MX:WARN` | `src/transaction/mvcc.rs:48` | Uses unsafe transmute to extend MutexGuard lifetime to 'static |
| `@MX:ANCHOR` | `src/wal/checkpoint.rs:13` | Critical data-integrity path: WAL -> main file flush |
| `@MX:NOTE` | `src/wal/recovery.rs:15` | Recovery resets the WAL after replay |
| `@MX:NOTE` | `src/wal/mod.rs:115` | Checksum is wrapping-add of frame_number, page_number, db_size |

### 5.4 I/O Model

Synchronous I/O (no async/await) was chosen, following SQLite's single-process embedded model. This decision was approved during strategy review to minimize complexity while meeting Phase 1 performance targets.

### 5.5 Simplifications Applied (Phase 2.10)

During the simplify phase, the following improvements were applied:
- `BufferPool`: extracted `fix_swapped_frame` helper to reduce code duplication
- `PropertyStore`: fixed deserialization edge case
- `WalReader`: used `Option::is_none_or` for cleaner conditional logic
