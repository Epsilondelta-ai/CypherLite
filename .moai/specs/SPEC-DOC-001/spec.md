# SPEC-DOC-001: Documentation, i18n & Static Website (v1.2.0)

| 필드 | 값 |
|------|-----|
| **SPEC ID** | SPEC-DOC-001 |
| **제목** | Documentation, Multi-Language i18n & Static Website (v1.2.0) |
| **생성일** | 2026-03-15 |
| **상태** | Completed |
| **우선순위** | High |
| **Phase** | 13 (Final) |
| **대상 버전** | v1.2.0 |

---

## 1. Environment (환경)

### 1.1 프로젝트 현황

CypherLite는 Rust 기반 경량 임베디드 그래프 데이터베이스로, Phase 1-12까지 모든 개발이 완료된 상태이다.

**완료된 크레이트 (7개)**:
- `cypherlite-core` (v1.0.0): 공통 타입, 에러 처리, 플러그인 트레이트
- `cypherlite-storage` (v1.0.0): 스토리지 엔진, WAL, B-Tree, MVCC
- `cypherlite-query` (v1.0.0): 쿼리 엔진 (Cypher 파서, 플래너, 실행기)
- `cypherlite-ffi` (v1.0.0): C ABI FFI 바인딩 (cbindgen)
- `cypherlite-python` (v1.0.0): Python 바인딩 (PyO3)
- `cypherlite-node` (v1.0.0): Node.js 바인딩 (napi-rs)
- `bindings/go/cypherlite`: Go 바인딩 (CGo)

**테스트 현황**: ~1,545 테스트 (Rust 1,450 + Go 35 + Python 25 + Node.js 35)

**CI/CD**: GitHub Actions 6 병렬 Job (check, msrv, test, coverage 85%, security, bench-check)

### 1.2 문서 현황 (현재 상태)

| 항목 | 현재 상태 | 목표 상태 |
|------|-----------|-----------|
| README.md | 기본 구조, Phase 10까지만 반영, FFI/Python/Node/Go 미포함 | 전체 기능 반영, 4개 언어 Quick Start, 배지, 10개 언어 번역 |
| CHANGELOG.md | 미존재 | v0.1~v1.2.0 전체 변경 이력 |
| rustdoc | 기본 doc comment만 존재 | 크레이트별 완전한 doc comment + doctest |
| crates.io 메타데이터 | description만 일부 존재 | 전체 필드 완비 (license, keywords, categories 등) |
| examples/ | 디렉토리 미존재 | Rust + Python + Go + Node.js 예제 |
| 버전 | 1.0.0 (일부 크레이트) | 1.2.0 통일 |
| 문서 웹사이트 | 미존재 | Nextra 기반 정적 문서 사이트 (10개 언어) |
| 다국어 지원 | 미존재 | README + 문서 사이트 10개 언어 번역 |

### 1.3 다국어 대상 언어 (화자 수 기준 상위 10개)

| 코드 | 언어 | 화자 수 | 역할 |
|------|------|---------|------|
| en | English | ~1.5B | 기본 언어 (Base) |
| zh | 中文 (Chinese) | ~1.1B | 번역 대상 |
| hi | हिन्दी (Hindi) | ~600M | 번역 대상 |
| es | Español (Spanish) | ~560M | 번역 대상 |
| fr | Français (French) | ~310M | 번역 대상 |
| ar | العربية (Arabic) | ~310M | 번역 대상 (RTL 지원 필요) |
| bn | বাংলা (Bengali) | ~270M | 번역 대상 |
| pt | Português (Portuguese) | ~260M | 번역 대상 |
| ru | Русский (Russian) | ~260M | 번역 대상 |
| ko | 한국어 (Korean) | ~80M | 번역 대상 (프로젝트 개발 언어) |

### 1.4 기술 제약

- Rust Edition 2024, MSRV 1.84
- Workspace 구조: 7개 crate 멤버 + Go 바인딩
- Feature flags: temporal-core (default), temporal-edge, subgraph, hypergraph, full-temporal, plugin
- 라이선스: 미결정 (TBD -> MIT 또는 Apache-2.0 결정 필요)
- 문서 사이트: Nextra (Next.js 기반), Node.js 18+ 필요
- Arabic (ar) RTL 레이아웃 지원 필요
- GitHub Pages 또는 Vercel 배포

---

## 2. Assumptions (가정)

### 2.1 확정 가정

| ID | 가정 | 신뢰도 | 근거 |
|----|------|--------|------|
| A1 | 모든 Rust 크레이트가 `cargo test --workspace --all-features` 통과 | High | CI/CD에서 지속적으로 검증 |
| A2 | FFI, Python, Node.js, Go 바인딩이 각각 기능 테스트 통과 상태 | High | Phase 12 완료 기준 충족 |
| A3 | crates.io 게시 시 라이선스 필드 필수 | High | crates.io 정책 |
| A4 | docs.rs가 자동으로 rustdoc 생성 | High | crates.io 게시 시 자동 트리거 |
| A5 | Keep a Changelog 형식이 Rust 생태계 표준 | High | 광범위한 채택 |
| A6 | Nextra가 built-in i18n을 지원함 | High | Nextra 공식 문서에서 i18n 기능 제공 |
| A7 | GitHub Pages에서 정적 Next.js 사이트 배포 가능 | High | `next export` / static HTML export 지원 |

### 2.2 검증 필요 가정

| ID | 가정 | 신뢰도 | 검증 방법 |
|----|------|--------|-----------|
| A8 | 라이선스 선택이 MIT 또는 Apache-2.0 | Medium | 프로젝트 소유자 확인 필요 |
| A9 | PyPI/npm 게시는 이번 SPEC 범위 외 | Medium | SPEC 범위 확인 |
| A10 | Go 모듈은 GitHub releases로 배포 | Medium | Go 생태계 관행 확인 |
| A11 | 번역 품질은 기계 번역 + 커뮤니티 리뷰로 확보 | Medium | 번역 워크플로우 검증 |
| A12 | Nextra 최신 안정 버전이 i18n + RTL을 완전 지원 | Medium | Nextra 문서 및 릴리즈 노트 확인 |

---

## 3. Requirements (요구사항)

### 3.1 버전 범프 v1.2.0 [R-DOC-050 ~ R-DOC-053]

**R-DOC-050** [Ubiquitous]
모든 워크스페이스 크레이트는 **항상** 동일한 버전 `1.2.0`을 사용해야 한다.
- cypherlite-core: 1.0.0 -> 1.2.0
- cypherlite-storage: 1.0.0 -> 1.2.0
- cypherlite-query: 1.0.0 -> 1.2.0
- cypherlite-ffi: 1.0.0 -> 1.2.0
- cypherlite-python: 1.0.0 -> 1.2.0
- cypherlite-node: 1.0.0 -> 1.2.0

**R-DOC-051** [Ubiquitous]
inter-crate 의존성 버전은 **항상** 워크스페이스 버전과 일치해야 한다.
- `cypherlite-core = { version = "1.2.0", path = "../cypherlite-core" }`

**R-DOC-052** [Event-Driven]
**WHEN** 버전 범프 후 `cargo check --workspace --all-features` 실행 시 **THEN** 에러 0건이어야 한다.

**R-DOC-053** [Event-Driven]
**WHEN** 버전 범프 후 `cargo test --workspace --all-features` 실행 시 **THEN** 전체 테스트 통과해야 한다.

### 3.2 README.md 전면 개편 (English Base) [R-DOC-001 ~ R-DOC-007]

**R-DOC-001** [Ubiquitous]
시스템은 **항상** README.md 상단에 프로젝트 배지를 표시해야 한다.
- CI 상태 배지 (GitHub Actions)
- crates.io 버전 배지
- docs.rs 문서 배지
- 라이선스 배지
- MSRV 배지

**R-DOC-002** [Ubiquitous]
README.md는 **항상** 프로젝트 개요를 포함해야 한다.
- 한 줄 설명 (tagline)
- 핵심 가치 제안 (Zero-config, Single-file, ACID, Embedded, Extensible)
- 경쟁 포지셔닝 요약

**R-DOC-003** [Ubiquitous]
README.md는 **항상** 4개 언어별 Quick Start 가이드를 포함해야 한다.
- Rust: `cargo add cypherlite-query` + 기본 CRUD
- Python: `pip install cypherlite` + 기본 사용법
- Go: `go get` + 기본 사용법
- Node.js: `npm install cypherlite` + 기본 사용법

**R-DOC-004** [Ubiquitous]
README.md는 **항상** Feature Highlights 섹션을 포함해야 한다.
- Storage Engine, Query Engine, Temporal, Subgraph, Hyperedge, Plugin 각 섹션
- 코드 예제 포함 (Cypher 쿼리 위주)
- Phase 1-12 전체 기능 반영

**R-DOC-005** [Ubiquitous]
README.md는 **항상** 텍스트 기반 Architecture Overview 다이어그램을 포함해야 한다.
- 5-Layer 아키텍처 (Application -> API -> Query -> Storage -> Transaction -> File System)
- Plugin System을 직교 레이어로 표현
- 크레이트 의존성 그래프 포함

**R-DOC-006** [Ubiquitous]
README.md는 **항상** Installation 섹션을 포함해야 한다.
- 언어별 설치 방법 (Rust, Python, Go, Node.js, C)
- Feature flags 설명
- MSRV 요구사항

**R-DOC-007** [Ubiquitous]
README.md는 **항상** Contributing, License, Status 섹션과 **다국어 README 링크 테이블**을 포함해야 한다.
- Contributing guidelines (brief, 별도 CONTRIBUTING.md 참조)
- 라이선스 정보
- 프로젝트 상태 (Phase 테이블 업데이트)
- 9개 번역 README 링크 테이블 (Available in: 中文, हिन्दी, Español, ...)

### 3.3 CHANGELOG.md 생성 [R-DOC-010 ~ R-DOC-012]

**R-DOC-010** [Ubiquitous]
시스템은 **항상** Keep a Changelog 형식의 CHANGELOG.md를 제공해야 한다.
- 헤더: "All notable changes to this project will be documented in this file."
- 형식: Added, Changed, Fixed, Removed 카테고리 사용
- 최신 버전이 상단에 위치

**R-DOC-011** [Ubiquitous]
CHANGELOG.md는 **항상** v0.1.0부터 v1.2.0까지 전체 버전 이력을 포함해야 한다.
- v0.1.0: Storage Engine (SPEC-DB-001)
- v0.2.0: Query Engine (SPEC-DB-002)
- v0.3.0: Advanced Query (SPEC-DB-003)
- v0.4.0: Temporal Core (SPEC-DB-004)
- v0.5.0: Temporal Edge (SPEC-DB-005)
- v0.6.0: Subgraph Entities (SPEC-DB-006)
- v0.7.0: Native Hyperedge (SPEC-DB-007)
- v0.8.0: Inline Property Filter (SPEC-DB-008)
- v0.9.0: CI/CD Pipeline (SPEC-INFRA-001)
- v1.0.0: Plugin System (SPEC-PLUGIN-001)
- v1.1.0: Performance Optimization (SPEC-PERF-001)
- v1.2.0: Documentation, i18n & Website (SPEC-DOC-001) -- 현재 SPEC

**R-DOC-012** [Event-Driven]
**WHEN** 새로운 버전이 릴리즈될 때 **THEN** CHANGELOG.md에 해당 버전 섹션이 추가되어야 한다.

### 3.4 Examples 강화 [R-DOC-040 ~ R-DOC-045]

**R-DOC-040** [Ubiquitous]
시스템은 **항상** `examples/` 디렉토리에 Rust 예제를 제공해야 한다.
- `examples/basic_crud.rs`: 기본 CRUD (Create, Read, Update, Delete)
- `examples/knowledge_graph.rs`: GraphRAG 활용 사례 (지식 그래프 구축)

**R-DOC-041** [Ubiquitous]
시스템은 **항상** 바인딩 언어별 Quick Start 예제를 제공해야 한다.
- `examples/python_quickstart.py`: Python 바인딩 기본 사용법
- `examples/go_quickstart.go`: Go 바인딩 기본 사용법
- `examples/node_quickstart.js`: Node.js 바인딩 기본 사용법

**R-DOC-042** [Event-Driven]
**WHEN** `cargo run --example basic_crud --all-features` 실행 시 **THEN** 정상 종료 (exit code 0)해야 한다.

**R-DOC-043** [Event-Driven]
**WHEN** `cargo run --example knowledge_graph --all-features` 실행 시 **THEN** 정상 종료해야 한다.

**R-DOC-044** [Ubiquitous]
각 예제 파일은 **항상** 상단에 목적 설명 주석을 포함해야 한다.
- 예제가 무엇을 보여주는지
- 필요한 feature flag
- 실행 방법

**R-DOC-045** [Optional]
**가능하면** 추가 예제를 제공한다.
- `examples/temporal_queries.rs`: AT TIME 시간 쿼리 예제
- `examples/plugin_example.rs`: 커스텀 플러그인 작성 예제
- `examples/agent_memory.rs`: LLM 에이전트 메모리 활용 예제

### 3.5 Rustdoc 강화 [R-DOC-020 ~ R-DOC-024]

**R-DOC-020** [Ubiquitous]
각 Rust 크레이트는 **항상** 크레이트 레벨 문서(`#![doc = include_str!("../README.md")]` 또는 인라인)를 포함해야 한다.
- `cypherlite-core`: 핵심 타입 및 트레이트 설명
- `cypherlite-storage`: 스토리지 엔진 아키텍처 설명
- `cypherlite-query`: 쿼리 엔진 파이프라인 설명
- `cypherlite-ffi`: C FFI 사용법 및 안전성 가이드

**R-DOC-021** [Ubiquitous]
모든 `pub` 타입, 함수, 트레이트는 **항상** `///` doc comment를 가져야 한다.
- 기능 설명 (1-2줄)
- Parameters/Returns 설명
- `# Examples` 섹션 (doctest로 컴파일 검증)

**R-DOC-022** [Ubiquitous]
시스템은 **항상** `#![warn(missing_docs)]`를 모든 lib.rs에 유지해야 한다.

**R-DOC-023** [Event-Driven]
**WHEN** `cargo doc --workspace --all-features --no-deps` 실행 시 **THEN** 경고 0건이어야 한다.

**R-DOC-024** [Ubiquitous]
Cargo.toml에는 **항상** docs.rs 빌드 설정이 포함되어야 한다.
```toml
[package.metadata.docs.rs]
all-features = true
rustdoc-args = ["--cfg", "docsrs"]
```

### 3.6 crates.io 게시 준비 [R-DOC-030 ~ R-DOC-034]

**R-DOC-030** [Ubiquitous]
각 Rust 크레이트의 Cargo.toml은 **항상** 다음 메타데이터를 포함해야 한다.
- `description`: 크레이트별 설명 (한 줄)
- `license`: MIT OR Apache-2.0 (dual license)
- `repository`: GitHub 저장소 URL
- `homepage`: GitHub 저장소 URL 또는 docs.rs URL
- `keywords`: 최대 5개 키워드
- `categories`: crates.io 카테고리
- `readme`: README.md 경로
- `authors`: 저자 목록

**R-DOC-031** [Ubiquitous]
크레이트별 키워드와 카테고리는 **항상** 다음을 따라야 한다.
- `cypherlite-core`: keywords = ["graph", "database", "embedded", "cypher", "property-graph"]
- `cypherlite-storage`: keywords = ["storage", "wal", "btree", "acid", "embedded-database"]
- `cypherlite-query`: keywords = ["cypher", "query-engine", "graph-database", "parser", "planner"]
- `cypherlite-ffi`: keywords = ["ffi", "c-abi", "bindings", "graph-database", "embedded"]
- categories: ["database-implementations", "data-structures"]

**R-DOC-032** [Event-Driven]
**WHEN** `cargo publish --dry-run -p {crate}` 실행 시 **THEN** 4개 Rust 크레이트 모두 성공해야 한다.
- 게시 순서: cypherlite-core -> cypherlite-storage -> cypherlite-query -> cypherlite-ffi

**R-DOC-033** [State-Driven]
**IF** inter-crate 의존성이 path 참조인 경우 **THEN** crates.io 게시 시 version 참조로 변환해야 한다.
- 예: `cypherlite-core = { path = "../cypherlite-core" }` -> `cypherlite-core = { version = "1.2.0", path = "../cypherlite-core" }`

**R-DOC-034** [Unwanted]
시스템은 `dev-dependencies` 크레이트를 crates.io에 게시**하지 않아야 한다**.
- tempfile, proptest, criterion 등은 dev-only

### 3.7 Documentation Website (Nextra) [R-DOC-060 ~ R-DOC-069]

**R-DOC-060** [Ubiquitous]
문서 사이트는 **항상** 프로젝트 루트의 `docs-site/` 디렉토리에 Nextra 프로젝트로 존재해야 한다.
- Next.js 기반 Nextra 프레임워크 사용
- `docs-site/package.json`에 빌드/개발 스크립트 포함
- `docs-site/next.config.mjs`에 i18n 설정 포함

**R-DOC-061** [Ubiquitous]
문서 사이트는 **항상** 다음 페이지 구조를 포함해야 한다.
- Landing page: 프로젝트 개요, 핵심 가치 제안, CTA (Getting Started 링크)
- Getting Started: 언어별 (Rust, Python, Go, Node.js) 빠른 시작 가이드
- API Reference: docs.rs 링크 (Rust), 바인딩별 API 개요
- Architecture: 5-Layer 아키텍처 개요, 크레이트 의존성 그래프
- Feature Guides: Temporal Queries, Plugin System, Subgraphs, Hyperedges 가이드
- Changelog: CHANGELOG.md 내용 렌더링
- Contributing: CONTRIBUTING.md 내용 렌더링

**R-DOC-062** [Ubiquitous]
문서 사이트는 **항상** 다음 UX 기능을 제공해야 한다.
- Dark mode / Light mode 토글
- 전문 검색 (Nextra built-in search 또는 Flexsearch)
- 반응형 레이아웃 (모바일/태블릿/데스크톱)
- 언어 전환기 (Language Switcher) - 10개 언어 간 전환

**R-DOC-063** [Ubiquitous]
문서 사이트의 모든 페이지는 **항상** Markdown (MDX) 형식으로 작성되어야 한다.
- `docs-site/pages/` 디렉토리 내 `.mdx` 파일
- Nextra 기본 MDX 컴포넌트 활용 (Callout, Tabs, Steps 등)

**R-DOC-064** [Event-Driven]
**WHEN** `cd docs-site && npm run build` 실행 시 **THEN** 빌드 성공 (exit code 0) 및 경고 0건이어야 한다.

**R-DOC-065** [Event-Driven]
**WHEN** `cd docs-site && npm run dev` 실행 시 **THEN** localhost에서 모든 페이지가 정상 렌더링되어야 한다.

**R-DOC-066** [Ubiquitous]
문서 사이트는 **항상** SEO 기본 메타데이터를 포함해야 한다.
- `<title>`, `<meta name="description">`, Open Graph 태그
- 각 페이지별 고유 title/description

**R-DOC-067** [Ubiquitous]
문서 사이트의 디렉토리 구조는 **항상** 다음을 따라야 한다.

```
docs-site/
├── package.json
├── next.config.mjs
├── theme.config.tsx
├── tsconfig.json
├── pages/
│   ├── _meta.json              # 네비게이션 구조
│   ├── index.mdx               # Landing page
│   ├── getting-started/
│   │   ├── _meta.json
│   │   ├── rust.mdx
│   │   ├── python.mdx
│   │   ├── go.mdx
│   │   └── nodejs.mdx
│   ├── api-reference/
│   │   ├── _meta.json
│   │   └── index.mdx           # docs.rs 링크 + 바인딩 API 개요
│   ├── architecture/
│   │   ├── _meta.json
│   │   └── index.mdx
│   ├── guides/
│   │   ├── _meta.json
│   │   ├── temporal-queries.mdx
│   │   ├── plugin-system.mdx
│   │   ├── subgraphs.mdx
│   │   └── hyperedges.mdx
│   ├── changelog.mdx
│   └── contributing.mdx
├── public/
│   └── og-image.png            # Open Graph 이미지
└── styles/
    └── globals.css
```

**R-DOC-068** [State-Driven]
**IF** Arabic (ar) 언어가 활성화된 경우 **THEN** 문서 사이트는 RTL (Right-to-Left) 레이아웃을 지원해야 한다.
- CSS `direction: rtl` 적용
- 네비게이션 및 코드 블록 레이아웃 조정

**R-DOC-069** [Unwanted]
문서 사이트는 코드 예제 내 주석이나 변수명을 번역**하지 않아야 한다**.
- 코드 블록 내용은 English 유지
- 코드 블록 외의 설명 텍스트만 번역

### 3.8 Multi-Language Documentation [R-DOC-070 ~ R-DOC-079]

**R-DOC-070** [Ubiquitous]
README.md는 **항상** 10개 언어 버전을 제공해야 한다.
- `README.md` (English, 기본)
- `docs/i18n/README.zh.md` (中文)
- `docs/i18n/README.hi.md` (हिन्दी)
- `docs/i18n/README.es.md` (Español)
- `docs/i18n/README.fr.md` (Français)
- `docs/i18n/README.ar.md` (العربية)
- `docs/i18n/README.bn.md` (বাংলা)
- `docs/i18n/README.pt.md` (Português)
- `docs/i18n/README.ru.md` (Русский)
- `docs/i18n/README.ko.md` (한국어)

**R-DOC-071** [Ubiquitous]
각 번역 README는 **항상** 영어 원본(README.md)과 동일한 섹션 구조를 유지해야 한다.
- 섹션 순서, 배지, 코드 블록 동일
- 설명 텍스트만 해당 언어로 번역
- 코드 블록 내 주석 및 변수명은 English 유지

**R-DOC-072** [Ubiquitous]
문서 사이트(Nextra)는 **항상** 10개 언어를 지원해야 한다.
- Nextra i18n 설정으로 locale별 페이지 라우팅
- `docs-site/pages/` 아래 locale별 디렉토리 또는 Nextra i18n 파일 구조
- Language Switcher UI에 10개 언어 표시

**R-DOC-073** [Ubiquitous]
문서 사이트의 번역 대상 페이지는 **항상** 다음을 포함해야 한다.
- Landing page (index.mdx)
- Getting Started 가이드 (Rust, Python, Go, Node.js)
- Architecture 개요
- Feature Guides (Temporal, Plugin, Subgraph, Hyperedge)
- Contributing 가이드

**R-DOC-074** [Unwanted]
다음 콘텐츠는 번역 대상이 **아니어야 한다**.
- 소스 코드 주석 (rustdoc English-only)
- CHANGELOG.md (English-only, 업계 표준)
- API Reference 자동 생성 문서 (docs.rs)
- 코드 블록 내 소스 코드 및 주석

**R-DOC-075** [Ubiquitous]
각 번역 파일은 **항상** 파일 상단에 다음 메타데이터를 포함해야 한다.
- 원본 언어: English
- 번역 대상 언어
- 마지막 동기화된 영어 원본 커밋 해시 (번역 최신성 추적)

**R-DOC-076** [Event-Driven]
**WHEN** English 원본 문서가 업데이트될 때 **THEN** 번역 파일의 동기화 커밋 해시가 outdated 상태로 표시되어야 한다.
- 번역 파일 상단 메타데이터의 커밋 해시와 원본 최신 커밋 비교

**R-DOC-077** [Ubiquitous]
번역 README 파일은 **항상** `docs/i18n/` 디렉토리에 위치해야 한다.
- 프로젝트 루트를 오염시키지 않기 위함
- 파일명 형식: `README.{lang_code}.md`

**R-DOC-078** [Ubiquitous]
문서 사이트 i18n 파일 구조는 **항상** Nextra의 i18n 규약을 따라야 한다.
- `next.config.mjs`에 `i18n.locales` 배열로 10개 언어 등록
- `i18n.defaultLocale: 'en'`
- locale별 `_meta.{locale}.json` 또는 locale 디렉토리 구조

**R-DOC-079** [Optional]
**가능하면** 번역 기여 가이드를 제공한다.
- `docs/i18n/TRANSLATING.md`: 번역 규칙, 용어집, PR 워크플로우
- 커뮤니티 번역 참여 방법 안내

### 3.9 Documentation Website Deployment [R-DOC-080 ~ R-DOC-084]

**R-DOC-080** [Ubiquitous]
문서 사이트는 **항상** GitHub Pages 또는 Vercel을 통해 배포 가능해야 한다.
- GitHub Actions 워크플로우로 자동 배포 (main 브랜치 push 시)
- 또는 Vercel 연동 설정 (`docs-site/vercel.json`)

**R-DOC-081** [Ubiquitous]
배포 파이프라인은 **항상** 다음 단계를 포함해야 한다.
- `npm ci` (의존성 설치)
- `npm run build` (정적 사이트 빌드)
- 빌드 산출물을 배포 대상으로 업로드

**R-DOC-082** [Event-Driven]
**WHEN** `main` 브랜치에 `docs-site/` 경로의 변경이 push될 때 **THEN** 문서 사이트 자동 재배포가 트리거되어야 한다.

**R-DOC-083** [Ubiquitous]
배포된 문서 사이트는 **항상** 다음 URL 패턴을 따라야 한다.
- 기본: `https://{domain}/` (English)
- 언어별: `https://{domain}/zh`, `https://{domain}/ko` 등
- domain은 GitHub Pages URL 또는 커스텀 도메인

**R-DOC-084** [Event-Driven]
**WHEN** 문서 사이트가 배포된 후 **THEN** 10개 언어 각각의 Landing page가 HTTP 200을 반환해야 한다.

---

## 4. Specifications (명세)

### 4.1 파일 변경 목록

| 파일 경로 | 작업 유형 | 설명 |
|-----------|-----------|------|
| `README.md` | Rewrite | 전면 개편 (배지, Quick Start, 아키텍처, i18n 링크) |
| `CHANGELOG.md` | Create | Keep a Changelog 형식 신규 생성 |
| `CONTRIBUTING.md` | Create | 기여 가이드라인 신규 생성 |
| `LICENSE-MIT` | Create | MIT 라이선스 파일 |
| `LICENSE-APACHE` | Create | Apache-2.0 라이선스 파일 |
| `crates/cypherlite-core/Cargo.toml` | Update | 메타데이터 + 버전 범프 |
| `crates/cypherlite-storage/Cargo.toml` | Update | 메타데이터 + 버전 범프 |
| `crates/cypherlite-query/Cargo.toml` | Update | 메타데이터 + 버전 범프 |
| `crates/cypherlite-ffi/Cargo.toml` | Update | 메타데이터 + 버전 범프 |
| `crates/cypherlite-python/Cargo.toml` | Update | 메타데이터 + 버전 범프 |
| `crates/cypherlite-node/Cargo.toml` | Update | 메타데이터 + 버전 범프 |
| `crates/*/src/lib.rs` | Update | `#![doc]` 속성 추가/강화 |
| `examples/basic_crud.rs` | Create | Rust CRUD 예제 |
| `examples/knowledge_graph.rs` | Create | GraphRAG 예제 |
| `examples/python_quickstart.py` | Create | Python Quick Start |
| `examples/go_quickstart.go` | Create | Go Quick Start |
| `examples/node_quickstart.js` | Create | Node.js Quick Start |
| `docs/i18n/README.{lang}.md` (x9) | Create | 9개 언어 README 번역 |
| `docs/i18n/TRANSLATING.md` | Create | 번역 가이드 |
| `docs-site/` (전체) | Create | Nextra 문서 사이트 프로젝트 |
| `.github/workflows/docs.yml` | Create | 문서 사이트 배포 워크플로우 |

### 4.2 Cargo.toml 메타데이터 템플릿

```toml
[package]
name = "cypherlite-{crate}"
version = "1.2.0"
edition = "2024"
rust-version = "1.84"
description = "{crate-specific description}"
license = "MIT OR Apache-2.0"
repository = "https://github.com/{owner}/cypherlite"
homepage = "https://github.com/{owner}/cypherlite"
keywords = ["{keyword1}", "{keyword2}", "{keyword3}", "{keyword4}", "{keyword5}"]
categories = ["database-implementations", "data-structures"]
readme = "README.md"

[package.metadata.docs.rs]
all-features = true
rustdoc-args = ["--cfg", "docsrs"]
```

### 4.3 README.md 구조 (English Base)

```
# CypherLite
[badges: CI, crates.io, docs.rs, license, msrv]

> SQLite-like simplicity for graph databases.

**Available in**: [中文](docs/i18n/README.zh.md) | [हिन्दी](docs/i18n/README.hi.md) | [Español](docs/i18n/README.es.md) | [Français](docs/i18n/README.fr.md) | [العربية](docs/i18n/README.ar.md) | [বাংলা](docs/i18n/README.bn.md) | [Português](docs/i18n/README.pt.md) | [Русский](docs/i18n/README.ru.md) | [한국어](docs/i18n/README.ko.md)

## Features
  ### Storage Engine
  ### Query Engine (Cypher)
  ### Temporal Features
  ### Subgraph & Hyperedge
  ### Plugin System
  ### FFI Bindings

## Quick Start
  ### Rust
  ### Python
  ### Go
  ### Node.js

## Installation
  ### Rust (cargo)
  ### Python (pip)
  ### Go (go get)
  ### Node.js (npm)
  ### C (header + static lib)

## Architecture
  [5-Layer diagram]
  [Crate dependency graph]

## Feature Flags
  [table]

## Performance
  [benchmark summary table]

## Documentation
  - docs.rs link
  - Documentation Website link
  - examples/ guide

## Contributing

## License

## Status / Roadmap
  [Phase table v0.1 ~ v1.2.0]
```

### 4.4 CHANGELOG.md 구조

```
# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [1.2.0] - 2026-XX-XX
### Added
- ...

## [1.1.0] - 2026-03-13
### Added
- ...

...down to v0.1.0
```

### 4.5 crates.io 게시 순서

의존성 그래프에 따라 반드시 다음 순서로 게시해야 한다:

1. `cypherlite-core` (의존성 없음)
2. `cypherlite-storage` (core에 의존)
3. `cypherlite-query` (core + storage에 의존)
4. `cypherlite-ffi` (query + core에 의존)

**참고**: `cypherlite-python`과 `cypherlite-node`는 PyPI/npm으로 별도 배포하며 crates.io에는 게시하지 않는다.

### 4.6 Nextra 문서 사이트 기술 스택

| 항목 | 선택 | 버전 | 선택 이유 |
|------|------|------|-----------|
| Framework | Nextra | 3.x | Next.js 기반, built-in i18n, MDX, 검색, Dark mode |
| Runtime | Node.js | 18+ | Next.js 최소 요구사항 |
| Styling | Tailwind CSS | (Nextra 내장) | Nextra 기본 스타일링 |
| Search | Flexsearch | (Nextra 내장) | 클라이언트 사이드 전문 검색 |
| Deployment | GitHub Pages / Vercel | - | 무료 정적 호스팅 |

### 4.7 번역 파일 메타데이터 형식

각 번역 README 상단:

```markdown
<!-- CypherLite Documentation Translation -->
<!-- Source Language: English -->
<!-- Target Language: {Language Name} ({Native Name}) -->
<!-- Last Synced Commit: {commit_hash} -->
<!-- Last Updated: {date} -->
```

### 4.8 docs-site i18n 설정 (next.config.mjs)

```js
const withNextra = require('nextra')({
  theme: 'nextra-theme-docs',
  themeConfig: './theme.config.tsx',
});

module.exports = withNextra({
  i18n: {
    locales: ['en', 'zh', 'hi', 'es', 'fr', 'ar', 'bn', 'pt', 'ru', 'ko'],
    defaultLocale: 'en',
  },
});
```

---

## 5. Traceability (추적성)

| 요구사항 | Milestone | 검증 기준 |
|----------|-----------|-----------|
| R-DOC-050~053 | M1 (Version Bump) | `cargo test --workspace --all-features` 통과, 전체 v1.2.0 |
| R-DOC-001~007 | M2 (README) | README.md 모든 섹션 존재, 배지 URL 유효, i18n 링크 존재 |
| R-DOC-010~012 | M3 (CHANGELOG) | 전체 버전 이력 포함, Keep a Changelog 형식 준수 |
| R-DOC-040~045 | M3 (Examples) | `cargo run --example` 정상 종료, 5개 파일 이상 |
| R-DOC-020~024 | M4 (Rustdoc) | `cargo doc` 경고 0건, doctest 통과 |
| R-DOC-030~034 | M5 (crates.io) | `cargo publish --dry-run` 4개 크레이트 성공 |
| R-DOC-060~069 | M7 (Website) | `npm run build` 성공, 페이지 렌더링 검증 |
| R-DOC-070~079 | M8 (i18n) | 10개 언어 README 존재, 문서 사이트 10개 locale 동작 |
| R-DOC-080~084 | M9 (Deployment) | 배포 URL에서 10개 언어 HTTP 200 |
