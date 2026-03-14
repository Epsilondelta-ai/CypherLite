# CypherLite - 기술 스택 및 아키텍처

> Rust 1.84+ (MSRV) 기반 고성능 임베디드 그래프 데이터베이스

---

## 기술 스택 개요

### 핵심 언어 및 런타임

| 항목 | 버전 | 선택 이유 |
|------|------|-----------|
| **Rust** | 1.92+ | 메모리 안전성 + 제로 코스트 추상화 + FFI 지원 |
| **Rust Edition** | 2024 | 최신 언어 기능 (GAT, const generics) 활용 |

### 스토리지 및 동시성 크레이트

| 크레이트 | 역할 | 선택 이유 |
|---------|------|-----------|
| `parking_lot` | RwLock, Mutex | 표준 라이브러리 대비 20-40% 빠른 잠금 성능 |
| `crossbeam` | 채널, 에포크 기반 메모리 재활용 | 잠금 없는 스택/큐, 안전한 동시 접근 |
| `dashmap` | 동시 접근 해시맵 | 락 없는 읽기 우세 워크로드에 최적 |

### 파싱 크레이트

| 크레이트 | 역할 | 선택 이유 |
|---------|------|-----------|
| `logos` | 렉서 (토크나이저) | 선언적 매크로로 초고속 토크나이저 자동 생성 |
| 손수 작성 재귀 하강 파서 | AST 생성 | 커스텀 에러 복구, 더 나은 에러 메시지 |

### 직렬화 크레이트

| 크레이트 | 역할 | 선택 이유 |
|---------|------|-----------|
| `bincode` | 내부 이진 직렬화 | 최소 크기, 최고 속도의 이진 포맷 |
| `serde` | 플러그인 설정 직렬화 | Rust 생태계 표준, JSON/YAML/TOML 지원 |

### 시간 처리 크레이트

| 크레이트 | 역할 | 선택 이유 |
|---------|------|-----------|
| `chrono` | 날짜/시간 타입 및 연산 | Rust 생태계 가장 성숙한 시간 라이브러리 |
| `time` | 저수준 시간 원시 타입 | chrono 보완, no_std 환경 지원 가능 |

### FFI 및 바인딩 크레이트

| 크레이트 | 역할 | 선택 이유 |
|---------|------|-----------|
| `cbindgen` | Rust → C 헤더 자동 생성 | 빌드 스크립트 통합, 안전한 FFI |
| `PyO3` | Python 바인딩 (계획됨) | Rust-Python 바인딩 사실상 표준 |
| `neon` | Node.js 바인딩 (계획됨) | N-API 기반 Node.js 네이티브 모듈 |

### 테스트 및 품질 크레이트

| 크레이트 | 역할 | 선택 이유 |
|---------|------|-----------|
| `criterion` | 마이크로 벤치마크 | 통계적으로 유의미한 성능 측정 |
| `proptest` | 속성 기반 테스트 | 무작위 입력으로 엣지 케이스 자동 탐색 |
| `tempfile` | 임시 파일 생성/정리 | 격리된 테스트 환경, 자동 정리 |

---

## 시스템 아키텍처 (5-레이어)

```
┌─────────────────────────────────────────────────────────────┐
│                    애플리케이션 레이어                         │
│         Python / Node.js / C FFI 바인딩 / 사용자 앱          │
└───────────────────────┬─────────────────────────────────────┘
                        │
┌───────────────────────▼─────────────────────────────────────┐
│                      API 레이어                              │
│    Cypher API  │  Native Rust API  │  Connection Pool        │
│         (cypherlite-ffi / python / node)                    │
└───────────────────────┬─────────────────────────────────────┘
                        │
┌───────────────────────▼─────────────────────────────────────┐
│                    쿼리 엔진 레이어                           │
│                                                              │
│  Lexer → Parser(AST) → Semantic Analysis                    │
│       → Logical Planner → Cost Optimizer                    │
│       → Physical Executor → Result Streaming                │
│                  (cypherlite-query)                          │
└───────────────────────┬─────────────────────────────────────┘
                        │
┌───────────────────────▼─────────────────────────────────────┐
│                    스토리지 레이어                            │
│                                                              │
│  Buffer Pool Manager  │  LRU Page Cache                     │
│  B-tree Node Store    │  B-tree Edge Store                  │
│  B-tree Property Store│  Label Index Pages                  │
│  Free Space Map                                              │
│                 (cypherlite-storage)                         │
└───────────────────────┬─────────────────────────────────────┘
                        │
┌───────────────────────▼─────────────────────────────────────┐
│                  트랜잭션 레이어                              │
│          WAL Manager  │  MVCC Transaction Manager            │
│                 (cypherlite-storage)                         │
└───────────────────────┬─────────────────────────────────────┘
                        │
┌───────────────────────▼─────────────────────────────────────┐
│                  파일 시스템 레이어                           │
│           app.cyl (주 데이터)  │  app.cyl-wal (WAL)          │
└─────────────────────────────────────────────────────────────┘

                     ┌──────────────────────┐
                     │   플러그인 시스템      │ ← 직교 레이어
                     │  Storage │ Index      │   (모든 레이어에
                     │  Query   │ Event      │    훅 연결 가능)
                     └──────────────────────┘
```

---

## 파일 형식 명세

### 주 데이터 파일 (.cyl)

```
app.cyl 내부 구조:
┌──────────────────────────────┐
│  Header Page (Page 0)        │  ← 매직 넘버, 버전, 루트 포인터
│  4KB                         │
├──────────────────────────────┤
│  Free Space Map Page         │  ← 빈 페이지 추적 비트맵
│  4KB                         │
├──────────────────────────────┤
│  Node B-tree Pages           │  ← NodeId → NodeData 매핑
│  각 4KB                      │
├──────────────────────────────┤
│  Edge B-tree Pages           │  ← EdgeId → (from, to, type) 매핑
│  각 4KB                      │
├──────────────────────────────┤
│  Property Pages              │  ← NodeId/EdgeId → Properties 매핑
│  각 4KB                      │
└──────────────────────────────┘
```

**주요 특성**:
- 페이지 크기: 4,096 바이트 (4KB) — OS 파일 시스템과 정렬
- 매직 넘버: `CYLT` (4바이트) — 파일 식별
- 버전 필드: 형식 진화 및 호환성 관리

### WAL 파일 (.cyl-wal)

```
app.cyl-wal 내부 구조:
┌──────────────────────────────┐
│  WAL Header                  │  ← 체크섬, 시퀀스 번호
├──────────────────────────────┤
│  WAL Frame 1                 │  ← 트랜잭션 ID, 페이지 번호, 데이터
├──────────────────────────────┤
│  WAL Frame 2                 │
├──────────────────────────────┤
│  ...                         │
└──────────────────────────────┘
```

**WAL 동작 원리**:
1. 쓰기 작업: 주 파일을 직접 수정하지 않고 WAL에 기록
2. 읽기 작업: WAL에서 최신 버전 확인 후 주 파일 참조
3. 체크포인트: WAL 데이터를 주 파일로 병합, WAL 파일 재설정
4. 충돌 복구: 시작 시 WAL 재생으로 커밋된 트랜잭션 복원

---

## 개발 환경 요구사항

### 필수 도구

```bash
# Rust 툴체인 설치 (rustup으로 관리)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# 최소 Rust 버전 확인
rustc --version  # 1.84.0 이상 필요 (MSRV)

# 워크스페이스 도구
cargo --version

# 코드 포맷터
rustup component add rustfmt

# 린터
rustup component add clippy
```

### 권장 도구

```bash
# cargo-watch: 파일 변경 시 자동 빌드/테스트
cargo install cargo-watch

# cargo-tarpaulin: 코드 커버리지 측정 (Linux)
cargo install cargo-tarpaulin

# cargo-audit: 보안 취약점 스캔
cargo install cargo-audit

# cargo-expand: 매크로 확장 결과 확인
cargo install cargo-expand

# cbindgen: C 헤더 자동 생성 (FFI 크레이트용)
cargo install cbindgen
```

### IDE 설정

**VS Code (권장)**:
- `rust-analyzer` 확장 설치 (LSP 지원)
- `CodeLLDB` 확장 설치 (디버거)

**설정 파일** (`.vscode/settings.json`):
```json
{
  "rust-analyzer.cargo.features": "all",
  "rust-analyzer.checkOnSave.command": "clippy"
}
```

---

## 빌드 및 테스트 명령어

### 기본 빌드

```bash
# 전체 워크스페이스 빌드 (개발 모드)
cargo build

# 전체 워크스페이스 빌드 (릴리즈 모드, 최적화 적용)
cargo build --release

# 특정 크레이트만 빌드
cargo build -p cypherlite-storage
```

### 테스트 실행

```bash
# 전체 테스트 (단위 + 통합)
cargo test

# 특정 크레이트 테스트
cargo test -p cypherlite-storage

# 레이스 컨디션 탐지 (동시성 버그 찾기)
cargo test --release -- --test-threads=1
RUSTFLAGS="-Z sanitizer=thread" cargo +nightly test

# 속성 기반 테스트 (proptest 케이스 수 증가)
PROPTEST_CASES=10000 cargo test

# 커버리지 측정 (cargo-tarpaulin 필요, Linux)
cargo tarpaulin --out Html
```

### 벤치마크 실행

```bash
# 전체 벤치마크 실행
cargo bench

# 특정 벤치마크만 실행
cargo bench --bench storage_bench

# 벤치마크 결과 비교 (baseline 저장)
cargo bench -- --save-baseline main
```

### 코드 품질

```bash
# 포맷 검사
cargo fmt --check

# 포맷 자동 수정
cargo fmt

# Clippy 린트 (경고를 에러로 처리)
cargo clippy -- -D warnings

# 보안 취약점 스캔
cargo audit

# C 헤더 파일 생성 (FFI 크레이트)
cbindgen --config cbindgen.toml --crate cypherlite-ffi --output include/cypherlite.h
```

### 개발 워크플로우

```bash
# 파일 변경 시 자동 테스트 실행
cargo watch -x test

# 파일 변경 시 자동 빌드 + 클립피
cargo watch -x "clippy -- -D warnings"
```

---

## 구현 로드맵

### Phase 1 - v0.1: 스토리지 엔진 (Weeks 1-4)

**목표**: 완전한 ACID 트랜잭션이 지원되는 스토리지 레이어 구현

**구현 항목**:
- 페이지 할당기 및 버퍼 풀
- B-트리 (노드/엣지/프로퍼티 스토어)
- WAL 쓰기 및 충돌 복구
- MVCC 트랜잭션 매니저

**완료 기준**:
- ACID 준수 테스트 통과
- 1,000 노드/초 이상 순차 쓰기 성능

---

### Phase 2 - v0.2: Cypher 쿼리 엔진 (Weeks 5-10)

**목표**: 기본 Cypher 쿼리 파싱 및 실행

**구현 항목**:
- logos 기반 렉서
- 재귀 하강 파서 + AST 정의
- 시맨틱 분석 (타입 검사)
- 논리 플래너 및 물리 실행기

**지원 Cypher 문법 (v0.2)**:
```
MATCH, CREATE, MERGE, SET, DELETE, RETURN
WHERE (비교 연산자, 논리 연산자)
패턴 매칭: (n:Label)-[:REL]->(m)
기본 집계: count(), sum(), avg()
```

---

### Phase 3 - v0.3: 인덱싱 및 최적화 (Weeks 11-14)

**목표**: 쿼리 성능 향상 및 플러그인 시스템 구축

**구현 항목**:
- 레이블 스캔 인덱스
- 비용 기반 쿼리 옵티마이저
- 플러그인 트레이트 및 레지스트리

---

### Phase 4 - v0.4: 시간 차원 (Weeks 15-18)

**목표**: 시간 인식 쿼리 지원

**구현 항목**:
- `AT TIME` 쿼리 파싱 및 실행
- 버전 스토어 (노드/엣지 이력)
- 스냅샷 격리

**지원 Cypher 문법 (v0.4 추가)**:
```
MATCH (n) AT TIME '2024-01-01' RETURN n
MATCH (n) BETWEEN TIME '2024-01-01' AND '2024-12-31' RETURN n
```

---

### Phase 5 - v0.5: 시간 차원 (Temporal Core)

**목표**: 시간 인식 쿼리 기본 지원

**구현 항목**:
- `AT TIME` 쿼리 파싱 및 실행
- 버전 스토어 (노드/엣지 이력 관리)
- 시간 범위 쿼리 지원

---

### Phase 6 - v0.6: 서브그래프 엔티티

**목표**: 서브그래프 스토어 및 중첩 그래프 구조 지원

**구현 항목**:
- SubgraphStore, MembershipIndex
- CREATE/MATCH SNAPSHOT 구문
- SubgraphScan 연산자, 가상 :CONTAINS 관계
- DatabaseHeader v4

---

### Phase 7 - v0.7: 네이티브 하이퍼엣지

**목표**: N:M 관계의 네이티브 하이퍼엣지 지원

**구현 항목**:
- HyperEdgeStore (BTreeMap 기반, 자동 증분 ID)
- HyperEdgeReverseIndex (양방향 매핑)
- DatabaseHeader v5 (hyperedge_root_page, next_hyperedge_id)
- CREATE/MATCH HYPEREDGE 구문 (Lexer → Parser → Planner → Executor)
- 가상 :INVOLVES 관계 확장
- TemporalRef 지연 해결 (VersionStore 체인 워크)
- GraphEntity 확장 (HyperEdge, TemporalRef 변형)

**테스트 결과**: 1,241 테스트 통과, 93.56% 커버리지

---

### Phase 8 - v0.8: 인라인 프로퍼티 필터

**목표**: MATCH 패턴의 인라인 프로퍼티 필터 `{key: value}` 버그 수정

**구현 항목**:
- `build_inline_property_predicate()` 유틸리티 함수 추출 (Subgraph 경로에서 공통화)
- NodeScan 경로: `first_node.properties` 필터 삽입 (Phase 8a)
- Expand 경로: `rel.properties` 및 `target_node.properties` 필터 삽입 (Phase 8b)
- VarLengthExpand 및 Subgraph Expand 경로 동일 적용 (Phase 8b)
- 익명 관계 처리: `_anon_rel` 내부 변수 자동 할당
- Proptest 속성 기반 테스트 4개 + Criterion 벤치마크 3개 (Phase 8c)
- 버전 범프 0.7.0 → 0.8.0

**테스트 결과**: 1,256 테스트 통과 (+15 신규)

---

### Phase 9 - v0.9: CI/CD Pipeline (완료)

**목표**: GitHub Actions 기반 자동 품질 게이트 구축

**구현 항목**:
- 6개 병렬 CI Job: check (clippy + fmt), msrv (Rust 1.84), test, coverage (85% gate), security (cargo-audit), bench-check
- Swatinem/rust-cache@v2 캐싱, dtolnay/rust-toolchain 툴체인 관리
- taiki-e/install-action으로 cargo-llvm-cov, cargo-audit 사전 빌드 바이너리 설치
- Dependabot: Cargo + GitHub Actions 의존성 주 1회 자동 확인

**생성 파일**: .github/workflows/ci.yml, .github/dependabot.yml

---

### Phase 10 - v1.0.0: 플러그인 시스템 (완료)

**목표**: 핵심 엔진을 건드리지 않고 기능을 확장할 수 있는 플러그인 아키텍처 구현

**구현 항목**:
- `Plugin` 베이스 트레이트 (Send + Sync, name + version)
- 4가지 확장 트레이트: `ScalarFunction`, `IndexPlugin`, `Serializer`, `Trigger`
- 제네릭 `PluginRegistry<T>`: register / get / get_mut / list / contains
- `TriggerContext`, `TriggerOperation`, `EntityType` 지원 타입 (trigger_types.rs)
- cypherlite-query 통합: ScalarFnLookup / TriggerLookup, register_* / list_* API
- 4개 플러그인 타입별 통합 테스트 (plugin_{function,index,serializer,trigger}_test.rs)
- 버전 범프 0.9.0 → 1.0.0

**테스트 결과**: 1,309 테스트 통과 (+53 신규)

---

## 설계 결정 근거

| 결정 사항 | 근거 |
|-----------|------|
| 단일 파일 형식 (.cyl) | 단순성, 원자적 백업, 이식성 — SQLite 검증된 방식 |
| 4KB 페이지 크기 | OS 페이지와 정렬로 캐시 효율 극대화 |
| 인덱스 프리 인접성 | O(1) 트래버설 — 그래프 탐색의 핵심 성능 |
| WAL 트랜잭션 | 동시 읽기 + 충돌 복구 모두 지원 |
| Cypher 서브셋 v1.0 | 빠른 딜리버리, 80% 활용 사례 커버 |
| 최소 코어 + 플러그인 | 유연성, 코어 비대화 방지 |
| Rust 구현 | 안전성, 성능, FFI 지원 — C/C++ 대비 메모리 안전 보장 |
| 손수 작성 파서 | 커스텀 에러 복구, 더 나은 개발자 경험 제공 |

---

## CI/CD 파이프라인 (구현 완료 - Phase 9)

`.github/workflows/ci.yml`에 6개 병렬 Job으로 구성:

| Job | 내용 |
|-----|------|
| `check` | `cargo clippy --workspace --all-targets --all-features -D warnings` + `cargo fmt --all --check` |
| `msrv` | Rust 1.84 툴체인으로 `cargo check --workspace --all-features` |
| `test` | `cargo test --workspace --all-features` |
| `coverage` | `cargo llvm-cov --workspace --all-features --fail-under-lines 85` (85% 미만 시 실패) |
| `security` | `cargo audit` (취약점 스캔) |
| `bench-check` | `cargo bench --no-run` (벤치마크 컴파일 검증) |

**도구 및 캐싱**:
- `dtolnay/rust-toolchain`: stable / master 툴체인 관리
- `Swatinem/rust-cache@v2`: Cargo 빌드 캐시
- `taiki-e/install-action`: cargo-llvm-cov, cargo-audit 사전 빌드 바이너리 설치
- `dependabot.yml`: Cargo + GitHub Actions 의존성 주 1회 자동 확인
