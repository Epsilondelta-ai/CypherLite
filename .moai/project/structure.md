# CypherLite - 프로젝트 구조

> Rust 워크스페이스 기반 멀티 크레이트 아키텍처

---

## 전체 디렉토리 구조

```
CypherLite/
├── Cargo.toml                    # 워크스페이스 루트 (멤버 크레이트 목록)
├── Cargo.lock                    # 의존성 잠금 파일 (버전 재현성 보장)
├── README.md                     # 프로젝트 소개 및 빠른 시작 가이드 (English, v1.2.0)
├── CHANGELOG.md                  # 전체 버전 이력 (Keep a Changelog 형식, v0.1~v1.2.0)
├── CONTRIBUTING.md               # 기여 가이드라인
├── LICENSE-MIT                   # MIT 라이선스
├── LICENSE-APACHE                # Apache-2.0 라이선스
│
├── .github/
│   ├── workflows/
│   │   ├── ci.yml                # CI/CD 파이프라인 (6 병렬 Job)
│   │   └── docs.yml              # 문서 사이트 자동 배포 워크플로우 (GitHub Pages)
│   └── dependabot.yml            # 의존성 자동 업데이트
│
├── crates/                       # 멀티 크레이트 워크스페이스
│   ├── cypherlite-core/          # 공통 타입, 에러 처리, 설정, 플러그인 시스템
│   ├── cypherlite-storage/       # 파일 형식, 페이지 관리, WAL, B-트리
│   ├── cypherlite-query/         # 렉서, 파서, AST, 플래너, 실행기
│   ├── cypherlite-ffi/           # C ABI FFI 바인딩 (cbindgen)
│   ├── cypherlite-python/        # Python 바인딩 (PyO3)
│   └── cypherlite-node/          # Node.js 바인딩 (napi-rs)
│
├── bindings/
│   └── go/cypherlite/            # Go 바인딩 (CGo)
│
├── docs/                         # 설계 문서 및 다국어 번역
│   ├── INDEX.md                  # 문서 목차 및 탐색 가이드
│   ├── 00_master_overview.md     # 전체 아키텍처 마스터 문서
│   ├── research/                 # 기술 조사 및 배경 연구
│   │   ├── 01_existing_technologies.md
│   │   ├── 02_cypher_rdf_temporal.md
│   │   └── 03_graphrag_agent_usecases.md
│   ├── design/                   # 구현 설계 문서
│   │   ├── 01_core_architecture.md
│   │   ├── 02_storage_engine.md
│   │   ├── 03_query_engine.md
│   │   └── 04_plugin_architecture.md
│   └── i18n/                     # 다국어 README 번역 (9개 언어)
│       ├── TRANSLATING.md        # 번역 기여 가이드
│       ├── README.zh.md          # 中文
│       ├── README.hi.md          # हिन्दी
│       ├── README.es.md          # Español
│       ├── README.fr.md          # Français
│       ├── README.ar.md          # العربية (RTL 지원)
│       ├── README.bn.md          # বাংলা
│       ├── README.pt.md          # Português
│       ├── README.ru.md          # Русский
│       └── README.ko.md          # 한국어
│
├── docs-site/                    # Nextra 3.x 정적 문서 사이트
│   ├── package.json              # Node.js 의존성 및 빌드 스크립트
│   ├── next.config.mjs           # Next.js + i18n 설정 (10개 locale)
│   ├── theme.config.tsx          # Nextra 테마 설정
│   ├── tsconfig.json             # TypeScript 설정
│   ├── pages/                    # MDX 문서 페이지
│   │   ├── _meta.json            # 네비게이션 구조
│   │   ├── index.mdx             # Landing page
│   │   ├── getting-started/      # 언어별 빠른 시작 (Rust, Python, Go, Node.js)
│   │   ├── api-reference/        # API 레퍼런스 (docs.rs 링크 + 바인딩 개요)
│   │   ├── architecture/         # 5-Layer 아키텍처 개요
│   │   ├── guides/               # Feature 가이드 (Temporal, Plugin, Subgraph, Hyperedge)
│   │   ├── changelog.mdx         # CHANGELOG 렌더링
│   │   └── contributing.mdx      # CONTRIBUTING 렌더링
│   ├── public/
│   │   └── og-image.png          # Open Graph 이미지
│   └── styles/
│       └── globals.css
│
├── examples/                     # 빠른 시작 예제
│   ├── basic_crud.rs             # Rust 기본 CRUD 예제
│   ├── knowledge_graph.rs        # GraphRAG 지식 그래프 예제
│   ├── python_quickstart.py      # Python 바인딩 quickstart
│   ├── go_quickstart.go          # Go 바인딩 quickstart
│   └── node_quickstart.js        # Node.js 바인딩 quickstart
│
├── tests/                        # 통합 테스트 (크레이트 경계 초월)
│   ├── integration/
│   │   ├── acid_compliance.rs    # ACID 속성 검증
│   │   ├── cypher_queries.rs     # Cypher 쿼리 e2e 테스트
│   │   └── concurrency.rs        # 동시성 안전성 테스트
│   └── fixtures/                 # 테스트용 데이터 파일
│
└── benches/                      # criterion 벤치마크
    ├── storage_bench.rs          # 스토리지 성능 측정
    ├── query_bench.rs            # 쿼리 처리 성능 측정
    └── concurrent_bench.rs       # 동시성 처리량 측정
```

---

## 크레이트별 상세 설명

### cypherlite-core (공통 기반)

**역할**: 모든 크레이트가 공유하는 기반 타입과 인터페이스 정의

**주요 포함 내용**:
- `NodeId`, `EdgeId`, `PropertyValue` 등 핵심 도메인 타입
- `HyperEdgeId`, `HyperEdgeRecord`, `GraphEntity` 확장 타입 (hypergraph 피처)
- `CypherLiteError` 에러 타입 계층 (thiserror 기반)
- `Config` 구조체: 페이지 크기, 캐시 크기, WAL 설정 등
- `Transaction` 트레이트: 트랜잭션 경계 추상화
- 공통 유틸리티: 직렬화 헬퍼, 체크섬 함수

**의존 크레이트**: 없음 (최하위 레이어)

**핵심 파일**:
```
crates/cypherlite-core/
├── Cargo.toml
└── src/
    ├── lib.rs
    ├── types.rs           # NodeId, EdgeId, Property 등
    ├── error.rs           # CypherLiteError 정의
    ├── config.rs          # DatabaseConfig 구조체
    ├── traits.rs          # Transaction, Cursor 트레이트
    ├── trigger_types.rs   # EntityType, TriggerContext, TriggerOperation
    └── plugin/
        └── mod.rs         # Plugin, ScalarFunction, IndexPlugin, Serializer, Trigger 트레이트 + PluginRegistry<T>
```

---

### cypherlite-storage (스토리지 엔진)

**역할**: 디스크 I/O, 파일 형식, 트랜잭션 관리의 완전한 구현

**주요 포함 내용**:
- `.cyl` 파일 형식 구현 (4KB 페이지 기반)
- 버퍼 풀 매니저 (LRU 페이지 캐시)
- WAL(Write-Ahead Log) 관리자: 쓰기 및 복구
- B-트리 구현: 노드 스토어, 엣지 스토어, 프로퍼티 스토어
- 레이블 인덱스 페이지
- 여유 공간 맵 (Free Space Map)
- MVCC 트랜잭션 매니저

**의존 크레이트**: `cypherlite-core`, `parking_lot`, `crossbeam`, `bincode`

**핵심 파일**:
```
crates/cypherlite-storage/
├── Cargo.toml
└── src/
    ├── lib.rs
    ├── page/
    │   ├── mod.rs
    │   ├── page_manager.rs    # 페이지 할당/해제
    │   └── buffer_pool.rs     # LRU 캐시
    ├── wal/
    │   ├── mod.rs
    │   ├── writer.rs          # WAL 쓰기
    │   └── recovery.rs        # 충돌 복구
    ├── btree/
    │   ├── mod.rs
    │   ├── node_store.rs      # 노드 B-트리
    │   ├── edge_store.rs      # 엣지 B-트리
    │   └── property_store.rs  # 프로퍼티 저장
    ├── transaction/
    │   ├── mod.rs
    │   └── mvcc.rs            # MVCC 구현
    ├── version/
    │   └── mod.rs             # 버전 스토어 (노드/엣지 이력)
    ├── subgraph/
    │   ├── mod.rs             # 서브그래프 스토어
    │   └── membership.rs     # 멤버십 인덱스
    └── hyperedge/
        ├── mod.rs             # 하이퍼엣지 스토어 (BTreeMap)
        └── reverse_index.rs  # 역방향 인덱스 (엔티티→하이퍼엣지)
```

---

### cypherlite-query (쿼리 엔진)

**역할**: Cypher 쿼리 파싱부터 실행까지 전체 파이프라인

**주요 포함 내용**:
- 렉서 (logos 크레이트 기반 토크나이저)
- 손수 작성한 재귀 하강 파서 → AST 생성
- 시맨틱 분석 (타입 검사, 스코프 해석)
- 논리 플래너: 쿼리 → 논리 계획
- 비용 기반 옵티마이저: 최적 물리 계획 선택
- 물리 실행기: 이터레이터 모델로 결과 스트리밍

**의존 크레이트**: `cypherlite-core`, `cypherlite-storage`, `logos`

**핵심 파일**:
```
crates/cypherlite-query/
├── Cargo.toml
└── src/
    ├── lib.rs
    ├── lexer/
    │   └── mod.rs             # logos 기반 토크나이저
    ├── parser/
    │   ├── mod.rs
    │   └── ast.rs             # AST 노드 정의
    ├── semantic/
    │   └── mod.rs             # 타입/스코프 분석
    ├── planner/
    │   ├── logical.rs         # 논리 플래너
    │   └── physical.rs        # 물리 플래너
    ├── optimizer/
    │   └── cost_model.rs      # 비용 모델
    └── executor/
        ├── mod.rs
        ├── eval.rs             # 표현식 평가, 시간 참조 해결
        └── operators/
            ├── mod.rs
            ├── expand.rs       # 관계 확장 (:INVOLVES 가상 관계 포함)
            └── hyperedge_scan.rs # 하이퍼엣지 스캔 연산자
tests/
    ├── inline_property_filter.rs  # 인라인 프로퍼티 필터 통합 테스트 (Phase 8a+8b)
    ├── proptest_inline_filter.rs  # 인라인 프로퍼티 필터 속성 기반 테스트 (Phase 8c)
    ├── plugin_function_test.rs    # ScalarFunction 플러그인 통합 테스트 (Phase 10)
    ├── plugin_index_test.rs       # IndexPlugin 통합 테스트 (Phase 10)
    ├── plugin_serializer_test.rs  # Serializer 플러그인 통합 테스트 (Phase 10)
    └── plugin_trigger_test.rs     # Trigger 플러그인 통합 테스트 (Phase 10)
benches/
    └── inline_filter.rs           # 인라인 프로퍼티 필터 성능 벤치마크 (Phase 8c)
```

**플러그인 통합 포인트** (executor/mod.rs, api/mod.rs):
- `ScalarFnLookup`: Cypher 함수 호출 시 ScalarFunction 플러그인 디스패치
- `TriggerLookup`: CREATE/SET/DELETE 실행 전후 Trigger 플러그인 호출
- `register_scalar_fn()` / `register_index_plugin()` / `register_serializer()` / `register_trigger()` API
- `list_scalar_fns()` / `list_index_plugins()` / `list_serializers()` / `list_triggers()` API

---

### cypherlite-ffi (C FFI 바인딩)

**역할**: C ABI 인터페이스 레이어. 불투명 포인터와 태그 유니언 값 시스템을 통해 CypherLite를 C에 노출

**주요 포함 내용**:
- `#[no_mangle]`이 적용된 `extern "C"` 함수 (C 상호 운용)
- `CylDb` (Mutex<CypherLite> 래핑), `CylTx`, `CylResult`, `CylRow` 불투명 타입
- `CylValue` `#[repr(C)]` 태그 유니언 (13가지 변형)
- `CylError` `#[repr(i32)]` 에러 코드 (20개 이상 상수)
- 스레드 로컬 에러 메시지 버퍼
- cbindgen으로 생성된 C11 헤더

**의존 크레이트**: `cypherlite-query`, `cypherlite-core`, `libc`

**핵심 파일**:
```
crates/cypherlite-ffi/
├── Cargo.toml
├── cbindgen.toml           # C 헤더 생성 설정
├── include/
│   └── cypherlite.h        # 생성된 C11 헤더 (557줄)
└── src/
    ├── lib.rs              # 모듈 선언, cyl_version(), cyl_features()
    ├── error.rs            # CylError 열거형, 스레드 로컬 메시지
    ├── db.rs               # CylDb 라이프사이클 (open/close)
    ├── query.rs            # 쿼리 실행 FFI
    ├── transaction.rs      # 트랜잭션 FFI
    ├── result.rs           # Result/Row 접근 FFI
    └── value.rs            # CylValue 태그 유니언, 파라미터 생성자
```

---

### cypherlite-python (Python 바인딩, 완료)

**역할**: PyO3를 통한 Python 네이티브 모듈 제공

**의존 크레이트**: `cypherlite-query`, `cypherlite-core`, `pyo3`

**구현 완료**: SPEC-FFI-003 (Phase 12)

---

### cypherlite-node (Node.js 바인딩, 완료)

**역할**: napi-rs를 통한 Node.js 네이티브 모듈 제공

**의존 크레이트**: `cypherlite-query`, `cypherlite-core`, `napi`, `napi-derive`

**구현 완료**: SPEC-FFI-004 (Phase 12)

---

## 문서 구조 (docs/)

### research/ - 기술 조사 자료

| 파일 | 내용 |
|------|------|
| `01_existing_technologies.md` | SQLite, Neo4j, DuckDB, KùzuDB 심층 분석 |
| `02_cypher_rdf_temporal.md` | openCypher 명세, RDF 표준, 시간 쿼리 연구 |
| `03_graphrag_agent_usecases.md` | GraphRAG 패턴, LLM 에이전트 메모리 활용 사례 |

### design/ - 구현 설계 문서

| 파일 | 내용 |
|------|------|
| `01_core_architecture.md` | 5-레이어 아키텍처 설계, 크레이트 경계 정의 |
| `02_storage_engine.md` | 파일 형식 명세, 페이지 레이아웃, WAL 프로토콜 |
| `03_query_engine.md` | Cypher 문법 정의, AST 노드 타입, 실행 계획 |
| `04_plugin_architecture.md` | 플러그인 트레이트 설계, 레지스트리 API |

---

## Phase 1 초기 파일 (Weeks 1-4)

Phase 1 (v0.1 - 스토리지 엔진)에서 처음 생성되는 파일들:

```
CypherLite/
├── Cargo.toml                                    # 워크스페이스 설정
├── crates/
│   ├── cypherlite-core/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── types.rs                          # 핵심 타입 정의
│   │       └── error.rs                          # 에러 타입
│   └── cypherlite-storage/
│       ├── Cargo.toml
│       └── src/
│           ├── lib.rs
│           ├── page/page_manager.rs              # 페이지 할당기
│           ├── page/buffer_pool.rs               # 버퍼 풀
│           ├── btree/node_store.rs               # 노드 B-트리
│           ├── btree/edge_store.rs               # 엣지 B-트리
│           ├── wal/writer.rs                     # WAL 쓰기
│           ├── wal/recovery.rs                   # 충돌 복구
│           └── transaction/mvcc.rs               # MVCC 트랜잭션
├── tests/integration/
│   └── acid_compliance.rs                        # ACID 검증 테스트
└── benches/
    └── storage_bench.rs                          # 스토리지 벤치마크
```

---

## 크레이트 의존성 그래프

현재 구현된 크레이트:

```
cypherlite-ffi ───────────────────────────────────┐
                                                   ↓
cypherlite-query ──→ cypherlite-storage ──→ cypherlite-core
       ↑                      ↑                  (plugin/ 모듈 포함)
       └──────────────────────┘
  (query도 storage를 직접 참조)
```

추가 바인딩 크레이트 (Phase 12, 완료):

```
cypherlite-node ──────────────────────────────────┐
cypherlite-python ────────────────────────────────┤
                                                   ↓
                        cypherlite-query ──→ cypherlite-storage ──→ cypherlite-core
```

Go 바인딩 (`bindings/go/cypherlite/`):

```
bindings/go/cypherlite ──→ (CGo) ──→ cypherlite-ffi ──→ cypherlite-query ──→ ...
```

**규칙**:
- `cypherlite-core`는 외부 의존성을 최소화 (thiserror, serde 정도)
- `cypherlite-core/src/plugin/` 모듈에 플러그인 트레이트 및 레지스트리 포함
- `cypherlite-storage`는 core에만 의존
- `cypherlite-query`는 core + storage에 의존
- `cypherlite-ffi`는 cypherlite-query 전체 스택에 의존 (C ABI 노출)

---

## Phase 10 - 플러그인 시스템 (v1.0.0, 완료)

Phase 10에서 구현된 플러그인 시스템 파일들:

```
crates/cypherlite-core/src/
├── trigger_types.rs                              # EntityType, TriggerContext, TriggerOperation
└── plugin/
    └── mod.rs                                    # Plugin 베이스 트레이트 + 4개 확장 트레이트 + PluginRegistry<T>

crates/cypherlite-query/tests/
├── plugin_function_test.rs                       # ScalarFunction 플러그인 통합 테스트
├── plugin_index_test.rs                          # IndexPlugin 통합 테스트
├── plugin_serializer_test.rs                     # Serializer 플러그인 통합 테스트
└── plugin_trigger_test.rs                        # Trigger 플러그인 통합 테스트
```

**4가지 플러그인 타입**:

| 트레이트 | 역할 | 주요 메서드 |
|---------|------|------------|
| `ScalarFunction` | Cypher에서 호출 가능한 사용자 정의 함수 | `call(&[PropertyValue])` |
| `IndexPlugin` | 플러그형 커스텀 인덱스 구현 | `insert`, `remove`, `lookup` |
| `Serializer` | 커스텀 직렬화 포맷 (임포트/익스포트) | `export`, `import` |
| `Trigger` | CREATE/UPDATE/DELETE 전후 이벤트 훅 | `on_before_create`, `on_after_create`, ... |

**테스트 결과**: 1,309 테스트 통과 (버전 v1.0.0)

---

## Phase 12 - C FFI 바인딩 (완료)

Phase 12에서 구현된 C FFI 바인딩 파일들:

```
crates/cypherlite-ffi/
├── Cargo.toml
├── cbindgen.toml
├── include/
│   └── cypherlite.h                          # cbindgen 생성 C11 헤더 (557줄)
└── src/
    ├── lib.rs                                # 모듈 루트, cyl_version(), cyl_features()
    ├── error.rs                              # CylError (20개 코드), 스레드 로컬 에러 메시지
    ├── db.rs                                 # cyl_db_open/open_with_config/close
    ├── query.rs                              # cyl_db_execute/execute_with_params
    ├── transaction.rs                        # cyl_tx_begin/execute/commit/rollback/free
    ├── result.rs                             # cyl_result_*/cyl_row_get*
    └── value.rs                              # CylValue 태그 유니언, cyl_param_* 생성자
```

**핵심 설계 결정**:

| 결정 | 선택 | 근거 |
|------|------|------|
| CylDb 동기화 | `Mutex<CypherLite>` 래핑 | C 호출자에게 단순한 단일 잠금 시맨틱 제공 |
| 트랜잭션 상태 | `AtomicBool` in-transaction 플래그 | 활성 트랜잭션 재진입 방지 |
| 값 표현 | `#[repr(C)]` 태그 유니언 | C11 호환, 제로 코스트 변환 |
| 에러 메시지 | 스레드 로컬 `RefCell<Option<CString>>` | 각 스레드가 독립적인 에러 컨텍스트 유지 |

**테스트 결과**: 115 TDD 테스트 통과 (워크스페이스 전체 1,450 테스트)

---

## Phase 13 - Documentation, i18n & Static Website (v1.2.0, 완료)

Phase 13에서 생성된 문서화 파일들 (SPEC-DOC-001):

```
CypherLite/
├── README.md                             # 전면 개편 (배지, Quick Start 4개 언어, 아키텍처)
├── CHANGELOG.md                          # Keep a Changelog 형식 (v0.1~v1.2.0)
├── CONTRIBUTING.md                       # 기여 가이드라인
├── LICENSE-MIT                           # MIT 라이선스
├── LICENSE-APACHE                        # Apache-2.0 라이선스
│
├── .github/workflows/
│   └── docs.yml                          # 문서 사이트 GitHub Pages 배포
│
├── docs/i18n/
│   ├── TRANSLATING.md                    # 번역 기여 가이드
│   ├── README.zh.md                      # 中文 번역
│   ├── README.hi.md                      # हिन्दी 번역
│   ├── README.es.md                      # Español 번역
│   ├── README.fr.md                      # Français 번역
│   ├── README.ar.md                      # العربية 번역 (RTL)
│   ├── README.bn.md                      # বাংলা 번역
│   ├── README.pt.md                      # Português 번역
│   ├── README.ru.md                      # Русский 번역
│   └── README.ko.md                      # 한국어 번역
│
├── docs-site/                            # Nextra 3.x 정적 문서 사이트
│   ├── package.json
│   ├── next.config.mjs                   # i18n 10개 locale 설정
│   ├── theme.config.tsx
│   ├── pages/
│   │   ├── index.mdx                     # Landing page
│   │   ├── getting-started/{rust,python,go,nodejs}.mdx
│   │   ├── api-reference/index.mdx
│   │   ├── architecture/index.mdx
│   │   ├── guides/{temporal-queries,plugin-system,subgraphs,hyperedges}.mdx
│   │   ├── changelog.mdx
│   │   └── contributing.mdx
│   └── public/og-image.png
│
└── examples/
    ├── basic_crud.rs                     # Rust CRUD 예제
    ├── knowledge_graph.rs                # GraphRAG 지식 그래프 예제
    ├── python_quickstart.py              # Python 바인딩 quickstart
    ├── go_quickstart.go                  # Go 바인딩 quickstart
    └── node_quickstart.js                # Node.js 바인딩 quickstart
```

**핵심 변경 사항**:

| 항목 | 이전 | 이후 |
|------|------|------|
| 버전 | 1.0.0 (각 크레이트) | 1.2.0 (전체 통일) |
| 라이선스 | 미결정 | MIT OR Apache-2.0 |
| README | 기본 구조 | 전면 개편 (배지, 4개 언어 Quick Start) |
| 다국어 지원 | 없음 | 10개 언어 (README + 문서 사이트) |
| 문서 사이트 | 없음 | Nextra 3.x (docs-site/) |
| 예제 | 미완성 | Rust + Python/Go/Node.js quickstart |
