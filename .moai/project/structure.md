# CypherLite - 프로젝트 구조

> Rust 워크스페이스 기반 멀티 크레이트 아키텍처

---

## 전체 디렉토리 구조

```
CypherLite/
├── Cargo.toml                    # 워크스페이스 루트 (멤버 크레이트 목록)
├── Cargo.lock                    # 의존성 잠금 파일 (버전 재현성 보장)
├── README.md                     # 프로젝트 소개 및 빠른 시작 가이드
├── LICENSE                       # 오픈소스 라이센스
│
├── .github/
│   ├── workflows/
│   │   └── ci.yml                # CI/CD 파이프라인 (6 병렬 Job)
│   └── dependabot.yml            # 의존성 자동 업데이트
│
├── crates/                       # 멀티 크레이트 워크스페이스
│   ├── cypherlite-core/          # 공통 타입, 에러 처리, 설정
│   ├── cypherlite-storage/       # 파일 형식, 페이지 관리, WAL, B-트리
│   ├── cypherlite-query/         # 렉서, 파서, AST, 플래너, 실행기
│   ├── cypherlite-plugin/        # 플러그인 트레이트, 레지스트리, 라이프사이클
│   ├── cypherlite-ffi/           # C FFI 바인딩 (cbindgen)
│   ├── cypherlite-python/        # PyO3 Python 바인딩
│   └── cypherlite-node/          # neon Node.js 바인딩
│
├── docs/                         # 설계 문서 및 연구 자료
│   ├── INDEX.md                  # 문서 목차 및 탐색 가이드
│   ├── 00_master_overview.md     # 전체 아키텍처 마스터 문서
│   ├── research/                 # 기술 조사 및 배경 연구
│   │   ├── 01_existing_technologies.md
│   │   ├── 02_cypher_rdf_temporal.md
│   │   └── 03_graphrag_agent_usecases.md
│   └── design/                   # 구현 설계 문서
│       ├── 01_core_architecture.md
│       ├── 02_storage_engine.md
│       ├── 03_query_engine.md
│       └── 04_plugin_architecture.md
│
├── tests/                        # 통합 테스트 (크레이트 경계 초월)
│   ├── integration/
│   │   ├── acid_compliance.rs    # ACID 속성 검증
│   │   ├── cypher_queries.rs     # Cypher 쿼리 e2e 테스트
│   │   └── concurrency.rs        # 동시성 안전성 테스트
│   └── fixtures/                 # 테스트용 데이터 파일
│
├── benches/                      # criterion 벤치마크
│   ├── storage_bench.rs          # 스토리지 성능 측정
│   ├── query_bench.rs            # 쿼리 처리 성능 측정
│   └── concurrent_bench.rs       # 동시성 처리량 측정
│
└── examples/                     # 사용 예제
    ├── basic_crud.rs             # 기본 CRUD 예제
    ├── knowledge_graph.rs        # 지식 그래프 구축 예제
    ├── temporal_queries.rs       # 시간 인식 쿼리 예제
    └── agent_memory.rs           # LLM 에이전트 메모리 활용 예제
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
    ├── types.rs        # NodeId, EdgeId, Property 등
    ├── error.rs        # CypherLiteError 정의
    ├── config.rs       # DatabaseConfig 구조체
    └── traits.rs       # Transaction, Cursor 트레이트
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
    └── proptest_inline_filter.rs  # 인라인 프로퍼티 필터 속성 기반 테스트 (Phase 8c)
benches/
    └── inline_filter.rs           # 인라인 프로퍼티 필터 성능 벤치마크 (Phase 8c)
```

---

### cypherlite-plugin (플러그인 시스템)

**역할**: 플러그인 트레이트 정의, 레지스트리 관리, 라이프사이클 제어

**6가지 플러그인 타입**:

| 타입 | 설명 | 예시 |
|------|------|------|
| `StoragePlugin` | 대체 백엔드, 암호화, 압축 | AES 암호화 스토리지 |
| `IndexPlugin` | 벡터(HNSW), 전문 검색, 공간 인덱스 | HNSW 벡터 인덱스 |
| `QueryPlugin` | 커스텀 함수, 프로시저, 그래프 알고리즘 | PageRank 알고리즘 |
| `SerializerPlugin` | RDF/OWL, JSON-LD, GraphML, CSV | RDF 임포트/익스포트 |
| `EventPlugin` | 변경 전/후 훅 | 감사 로그 생성 |
| `BusinessLogicPlugin` | 시맨틱 레이어, 키네틱 레이어 | 워크플로우 트리거 |

**핵심 파일**:
```
crates/cypherlite-plugin/
├── Cargo.toml
└── src/
    ├── lib.rs
    ├── traits/
    │   ├── storage.rs         # StoragePlugin 트레이트
    │   ├── index.rs           # IndexPlugin 트레이트
    │   ├── query.rs           # QueryPlugin 트레이트
    │   ├── serializer.rs      # SerializerPlugin 트레이트
    │   └── event.rs           # EventPlugin 트레이트
    ├── registry.rs            # 플러그인 레지스트리
    └── lifecycle.rs           # 초기화/종료 관리
```

---

### cypherlite-ffi (C FFI 바인딩)

**역할**: C 헤더 파일 자동 생성, 안전한 Rust-C 인터페이스 노출

**주요 포함 내용**:
- `extern "C"` 함수 정의 (cbindgen으로 헤더 자동 생성)
- Rust 패닉의 C 호환 에러 코드 변환
- 불투명 포인터 패턴으로 안전한 인터페이스

**생성 파일**: `include/cypherlite.h`

---

### cypherlite-python (Python 바인딩)

**역할**: PyO3를 통한 Python 네이티브 모듈 제공 (계획됨)

**예상 Python API**:
```python
import cypherlite

db = cypherlite.open("app.cyl")
with db.transaction() as tx:
    tx.run("CREATE (n:Person {name: 'Alice'})")
    result = tx.run("MATCH (n:Person) RETURN n.name")
```

---

### cypherlite-node (Node.js 바인딩)

**역할**: neon을 통한 Node.js 네이티브 모듈 제공 (계획됨)

**예상 Node.js API**:
```javascript
const { CypherLite } = require('cypherlite');

const db = new CypherLite('app.cyl');
const result = await db.run('MATCH (n) RETURN count(n)');
```

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

```
cypherlite-node ──────────────────────────────────┐
cypherlite-python ────────────────────────────────┤
cypherlite-ffi ───────────────────────────────────┤
                                                   ↓
cypherlite-plugin ──→ cypherlite-query ──→ cypherlite-storage ──→ cypherlite-core
                              ↑                        ↑
                              └────────────────────────┘
                         (query도 storage를 직접 참조)
```

**규칙**:
- `cypherlite-core`는 외부 의존성을 최소화 (thiserror, serde 정도)
- `cypherlite-storage`는 core에만 의존
- `cypherlite-query`는 core + storage에 의존
- FFI/바인딩 크레이트는 모든 크레이트를 집약
