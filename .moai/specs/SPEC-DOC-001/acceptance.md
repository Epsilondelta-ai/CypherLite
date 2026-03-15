# SPEC-DOC-001: Acceptance Criteria

| 필드 | 값 |
|------|-----|
| **SPEC ID** | SPEC-DOC-001 |
| **제목** | Documentation, Multi-Language i18n & Static Website (v1.2.0) |
| **검증 방식** | Manual Review + Automated Commands |

---

## AC-1: 버전 범프 v1.2.0 (M1)

### AC-1.1: 전체 크레이트 버전 통일

**Given** 6개 워크스페이스 크레이트가 존재할 때
**When** 각 Cargo.toml의 `version` 필드를 확인하면
**Then** 모든 크레이트가 `1.2.0`이어야 한다:
- [ ] cypherlite-core: version = "1.2.0"
- [ ] cypherlite-storage: version = "1.2.0"
- [ ] cypherlite-query: version = "1.2.0"
- [ ] cypherlite-ffi: version = "1.2.0"
- [ ] cypherlite-python: version = "1.2.0"
- [ ] cypherlite-node: version = "1.2.0"

### AC-1.2: 빌드 검증

**Given** 버전 범프가 완료되었을 때
**When** `cargo check --workspace --all-features` 실행 시
**Then** 다음을 만족해야 한다:
- [ ] 에러 0건
- [ ] 경고 0건 (clippy)

### AC-1.3: 테스트 검증

**Given** 버전 범프가 완료되었을 때
**When** `cargo test --workspace --all-features` 실행 시
**Then** 다음을 만족해야 한다:
- [ ] 전체 테스트 통과 (~1,450 Rust 테스트)
- [ ] 실패 0건

### AC-1.4: inter-crate 의존성 버전 일치

**Given** inter-crate 의존성이 존재할 때
**When** 각 크레이트의 dependencies를 확인하면
**Then** 의존성 버전이 1.2.0과 일치해야 한다:
- [ ] cypherlite-storage -> cypherlite-core = "1.2.0"
- [ ] cypherlite-query -> cypherlite-core = "1.2.0"
- [ ] cypherlite-query -> cypherlite-storage = "1.2.0"
- [ ] cypherlite-ffi -> cypherlite-query = "1.2.0"
- [ ] cypherlite-ffi -> cypherlite-core = "1.2.0"
- [ ] cypherlite-python -> cypherlite-query = "1.2.0"
- [ ] cypherlite-python -> cypherlite-core = "1.2.0"
- [ ] cypherlite-node -> cypherlite-query = "1.2.0"
- [ ] cypherlite-node -> cypherlite-core = "1.2.0"

---

## AC-2: README.md 전면 개편 (M2)

### AC-2.1: 배지 표시

**Given** README.md가 프로젝트 루트에 존재할 때
**When** README.md 상단을 확인하면
**Then** 다음 5개 배지가 모두 존재해야 한다:
- [ ] CI 상태 배지 (GitHub Actions)
- [ ] crates.io 버전 배지
- [ ] docs.rs 문서 배지
- [ ] 라이선스 배지 (MIT/Apache-2.0)
- [ ] MSRV 배지 (1.84)

### AC-2.2: 다국어 링크 테이블

**Given** README.md가 존재할 때
**When** 배지 아래 영역을 확인하면
**Then** 9개 번역 README 링크가 포함되어야 한다:
- [ ] "Available in:" 라벨 존재
- [ ] 中文 링크 -> `docs/i18n/README.zh.md`
- [ ] हिन्दी 링크 -> `docs/i18n/README.hi.md`
- [ ] Español 링크 -> `docs/i18n/README.es.md`
- [ ] Français 링크 -> `docs/i18n/README.fr.md`
- [ ] العربية 링크 -> `docs/i18n/README.ar.md`
- [ ] বাংলা 링크 -> `docs/i18n/README.bn.md`
- [ ] Português 링크 -> `docs/i18n/README.pt.md`
- [ ] Русский 링크 -> `docs/i18n/README.ru.md`
- [ ] 한국어 링크 -> `docs/i18n/README.ko.md`

### AC-2.3: 프로젝트 개요

**Given** README.md가 존재할 때
**When** 프로젝트 개요 섹션을 확인하면
**Then** 다음 항목이 포함되어야 한다:
- [ ] 한 줄 tagline ("SQLite-like simplicity for graph databases" 또는 유사 표현)
- [ ] 핵심 가치 제안 (Zero-config, Single-file, ACID, Embedded, Extensible)
- [ ] Phase 1-12 전체 기능 요약

### AC-2.4: 4개 언어 Quick Start

**Given** README.md에 Quick Start 섹션이 존재할 때
**When** 각 언어별 Quick Start를 확인하면
**Then** 다음 4개 언어의 코드 예제가 모두 포함되어야 한다:
- [ ] Rust: `CypherLite::open()` + CREATE + MATCH + RETURN
- [ ] Python: `import cypherlite` + 기본 CRUD
- [ ] Go: CGo 기반 `cyl_db_open()` + `cyl_db_execute()` + `cyl_db_close()`
- [ ] Node.js: napi-rs 기반 기본 사용법

### AC-2.5: Feature Highlights

**Given** README.md에 Features 섹션이 존재할 때
**When** 각 기능 영역을 확인하면
**Then** 다음 6개 영역이 모두 포함되어야 한다:
- [ ] Storage Engine (ACID, WAL, B+Tree, Single-file)
- [ ] Query Engine (Cypher, Parser, Planner, Executor)
- [ ] Temporal Features (AT TIME, Version Store)
- [ ] Subgraph & Hyperedge (SNAPSHOT, N:M relations)
- [ ] Plugin System (4 types: ScalarFunction, IndexPlugin, Serializer, Trigger)
- [ ] FFI Bindings (C, Python, Go, Node.js)

### AC-2.6: Architecture Diagram

**Given** README.md에 Architecture 섹션이 존재할 때
**When** 아키텍처 다이어그램을 확인하면
**Then** 다음이 포함되어야 한다:
- [ ] 텍스트 기반(ASCII art) 5-Layer 아키텍처 다이어그램
- [ ] Application -> API -> Query -> Storage -> Transaction -> File System 레이어 표현
- [ ] Plugin System이 직교 레이어로 표현
- [ ] 크레이트 의존성 그래프 (core -> storage -> query -> ffi)

### AC-2.7: Installation 섹션

**Given** README.md에 Installation 섹션이 존재할 때
**When** 각 언어별 설치 방법을 확인하면
**Then** 다음 5개 언어의 설치 가이드가 포함되어야 한다:
- [ ] Rust: `cargo add cypherlite-query` (또는 Cargo.toml 직접 추가)
- [ ] Python: `pip install cypherlite`
- [ ] Go: `go get` 가이드
- [ ] Node.js: `npm install cypherlite`
- [ ] C: 헤더 파일 + 정적/동적 라이브러리 링크 방법

### AC-2.8: Contributing & License

**Given** README.md에 Contributing 및 License 섹션이 존재할 때
**When** 해당 섹션을 확인하면
**Then** 다음이 포함되어야 한다:
- [ ] CONTRIBUTING.md 참조 링크
- [ ] 라이선스 정보 (MIT OR Apache-2.0)
- [ ] CONTRIBUTING.md 파일이 프로젝트 루트에 존재
- [ ] LICENSE-MIT 파일이 존재
- [ ] LICENSE-APACHE 파일이 존재

### AC-2.9: Phase Status 테이블

**Given** README.md에 Status 섹션이 존재할 때
**When** Phase 테이블을 확인하면
**Then** 다음이 포함되어야 한다:
- [ ] Phase 1 (v0.1.0) ~ Phase 13 (v1.2.0) 전체 표시
- [ ] 각 Phase의 SPEC ID, 버전, 설명, 상태 포함
- [ ] Phase 11 (Performance), Phase 12 (FFI), Phase 13 (Documentation + i18n + Website) 반영

### AC-2.10: Documentation Website 링크

**Given** README.md에 Documentation 섹션이 존재할 때
**When** 해당 섹션을 확인하면
**Then** 다음이 포함되어야 한다:
- [ ] 문서 사이트 URL 링크
- [ ] docs.rs 링크
- [ ] examples/ 가이드 링크

---

## AC-3: CHANGELOG.md 생성 (M3)

### AC-3.1: 형식 준수

**Given** CHANGELOG.md가 프로젝트 루트에 존재할 때
**When** 파일 형식을 확인하면
**Then** 다음 Keep a Changelog 형식을 준수해야 한다:
- [ ] 헤더에 "All notable changes to this project will be documented in this file." 포함
- [ ] Keep a Changelog 링크 참조
- [ ] Semantic Versioning 링크 참조
- [ ] 최신 버전이 파일 상단에 위치

### AC-3.2: 전체 버전 이력

**Given** CHANGELOG.md가 존재할 때
**When** 버전 섹션을 확인하면
**Then** 다음 12개 버전이 모두 포함되어야 한다:
- [ ] v0.1.0: Storage Engine (SPEC-DB-001)
- [ ] v0.2.0: Query Engine (SPEC-DB-002)
- [ ] v0.3.0: Advanced Query (SPEC-DB-003)
- [ ] v0.4.0: Temporal Core (SPEC-DB-004)
- [ ] v0.5.0: Temporal Edge (SPEC-DB-005)
- [ ] v0.6.0: Subgraph Entities (SPEC-DB-006)
- [ ] v0.7.0: Native Hyperedge (SPEC-DB-007)
- [ ] v0.8.0: Inline Property Filter (SPEC-DB-008)
- [ ] v0.9.0: CI/CD Pipeline (SPEC-INFRA-001)
- [ ] v1.0.0: Plugin System (SPEC-PLUGIN-001)
- [ ] v1.1.0: Performance Optimization (SPEC-PERF-001)
- [ ] v1.2.0: Documentation, i18n & Website (SPEC-DOC-001)

### AC-3.3: 카테고리 사용

**Given** CHANGELOG.md의 각 버전 섹션이 존재할 때
**When** 카테고리를 확인하면
**Then** Added, Changed, Fixed, Removed 중 해당하는 카테고리를 사용해야 한다:
- [ ] 각 버전에 최소 1개 카테고리 존재
- [ ] 각 카테고리에 최소 1개 항목 존재
- [ ] 항목이 구체적이고 이해 가능한 수준

---

## AC-4: Rustdoc 강화 (M4)

### AC-4.1: 크레이트 레벨 문서

**Given** 4개 Rust 크레이트가 존재할 때
**When** 각 크레이트의 `src/lib.rs`를 확인하면
**Then** 크레이트 레벨 문서가 포함되어야 한다:
- [ ] `cypherlite-core`: 핵심 타입 및 트레이트 개요
- [ ] `cypherlite-storage`: 스토리지 엔진 아키텍처 개요
- [ ] `cypherlite-query`: 쿼리 엔진 파이프라인 개요
- [ ] `cypherlite-ffi`: C FFI 사용법 및 안전성 가이드

### AC-4.2: pub 항목 doc comment

**Given** 각 크레이트의 pub 타입/함수/트레이트가 존재할 때
**When** doc comment를 확인하면
**Then** 다음 주요 항목에 doc comment가 존재해야 한다:
- [ ] `CypherLite` struct (query 크레이트)
- [ ] `StorageEngine` struct (storage 크레이트)
- [ ] `NodeId`, `EdgeId`, `PropertyValue` types (core 크레이트)
- [ ] `CypherLiteError` enum variants (core 크레이트)
- [ ] `Plugin`, `ScalarFunction`, `IndexPlugin`, `Serializer`, `Trigger` traits (core 크레이트)
- [ ] `PluginRegistry<T>` struct (core 크레이트)
- [ ] `CylDb`, `CylTx`, `CylResult`, `CylRow` opaque types (ffi 크레이트)
- [ ] `CylValue`, `CylError` C types (ffi 크레이트)

### AC-4.3: cargo doc 경고 0건

**Given** 전체 워크스페이스가 빌드 가능할 때
**When** `cargo doc --workspace --all-features --no-deps` 실행 시
**Then** 다음을 만족해야 한다:
- [ ] 경고(warning) 0건
- [ ] 에러(error) 0건
- [ ] 문서 빌드 성공

### AC-4.4: docs.rs 설정

**Given** 4개 Rust 크레이트의 Cargo.toml이 존재할 때
**When** `[package.metadata.docs.rs]` 섹션을 확인하면
**Then** 다음이 포함되어야 한다:
- [ ] `all-features = true`
- [ ] `rustdoc-args = ["--cfg", "docsrs"]`

### AC-4.5: #![warn(missing_docs)]

**Given** 4개 Rust 크레이트의 `src/lib.rs`가 존재할 때
**When** 파일 상단을 확인하면
**Then** 다음이 포함되어야 한다:
- [ ] `#![warn(missing_docs)]` 속성 (4개 크레이트 모두)

---

## AC-5: crates.io 게시 준비 (M5)

### AC-5.1: Cargo.toml 메타데이터

**Given** 4개 게시 대상 크레이트의 Cargo.toml이 존재할 때
**When** 메타데이터 필드를 확인하면
**Then** 다음 필드가 모두 존재해야 한다:
- [ ] `description`: 각 크레이트별 한 줄 설명
- [ ] `license`: "MIT OR Apache-2.0"
- [ ] `repository`: GitHub 저장소 URL
- [ ] `homepage`: GitHub 또는 docs.rs URL
- [ ] `keywords`: 최대 5개 키워드 (크레이트별 고유)
- [ ] `categories`: `["database-implementations"]` 포함
- [ ] `readme`: README 경로

### AC-5.2: cargo publish --dry-run 성공

**Given** 전체 크레이트가 빌드 가능할 때
**When** `cargo publish --dry-run` 실행 시
**Then** 4개 크레이트가 순서대로 성공해야 한다:
- [ ] `cargo publish --dry-run -p cypherlite-core` 성공
- [ ] `cargo publish --dry-run -p cypherlite-storage` 성공
- [ ] `cargo publish --dry-run -p cypherlite-query` 성공
- [ ] `cargo publish --dry-run -p cypherlite-ffi` 성공

### AC-5.3: inter-crate 버전 참조

**Given** inter-crate 의존성이 존재할 때
**When** Cargo.toml의 dependencies를 확인하면
**Then** 다음 형식이어야 한다:
- [ ] `cypherlite-core = { version = "1.2.0", path = "../cypherlite-core" }`
- [ ] `cypherlite-storage = { version = "1.2.0", path = "../cypherlite-storage" }`
- [ ] `cypherlite-query = { version = "1.2.0", path = "../cypherlite-query" }`

### AC-5.4: 비게시 크레이트 표시

**Given** Python, Node.js 바인딩 크레이트가 존재할 때
**When** Cargo.toml을 확인하면
**Then** 다음이 포함되어야 한다:
- [ ] `cypherlite-python`: `publish = false`
- [ ] `cypherlite-node`: `publish = false`

---

## AC-6: Examples 강화 (M6)

### AC-6.1: Rust 예제 존재

**Given** `examples/` 디렉토리가 존재할 때
**When** Rust 예제 파일을 확인하면
**Then** 다음 파일이 존재해야 한다:
- [ ] `examples/basic_crud.rs`: 기본 CRUD 예제
- [ ] `examples/knowledge_graph.rs`: GraphRAG 활용 예제

### AC-6.2: 바인딩 예제 존재

**Given** `examples/` 디렉토리가 존재할 때
**When** 바인딩 예제 파일을 확인하면
**Then** 다음 파일이 존재해야 한다:
- [ ] `examples/python_quickstart.py`: Python 바인딩 예제
- [ ] `examples/go_quickstart.go`: Go 바인딩 예제
- [ ] `examples/node_quickstart.js`: Node.js 바인딩 예제

### AC-6.3: Rust 예제 실행 검증

**Given** Rust 예제 파일이 존재할 때
**When** `cargo run --example basic_crud --all-features` 실행 시
**Then** 다음을 만족해야 한다:
- [ ] 컴파일 성공
- [ ] 정상 종료 (exit code 0)
- [ ] 에러 메시지 없음

**Given** knowledge_graph 예제가 존재할 때
**When** `cargo run --example knowledge_graph --all-features` 실행 시
**Then** 다음을 만족해야 한다:
- [ ] 컴파일 성공
- [ ] 정상 종료 (exit code 0)

### AC-6.4: 예제 문서화

**Given** 각 예제 파일이 존재할 때
**When** 파일 상단 주석을 확인하면
**Then** 다음이 포함되어야 한다:
- [ ] 예제 목적 설명
- [ ] 필요한 feature flag (Rust 예제)
- [ ] 실행 방법 (command line)

---

## AC-7: Documentation Website (M7)

### AC-7.1: Nextra 프로젝트 구조

**Given** `docs-site/` 디렉토리가 존재할 때
**When** 프로젝트 구조를 확인하면
**Then** 다음 파일/디렉토리가 존재해야 한다:
- [ ] `docs-site/package.json` (nextra, nextra-theme-docs 의존성 포함)
- [ ] `docs-site/next.config.mjs` (Nextra 플러그인 + i18n 설정)
- [ ] `docs-site/theme.config.tsx` (로고, 검색, Dark mode, Language Switcher)
- [ ] `docs-site/tsconfig.json`
- [ ] `docs-site/pages/` 디렉토리

### AC-7.2: 필수 페이지 존재

**Given** `docs-site/pages/` 디렉토리가 존재할 때
**When** 페이지 파일을 확인하면
**Then** 다음 페이지가 모두 존재해야 한다:
- [ ] `pages/index.mdx` (Landing page)
- [ ] `pages/getting-started/rust.mdx`
- [ ] `pages/getting-started/python.mdx`
- [ ] `pages/getting-started/go.mdx`
- [ ] `pages/getting-started/nodejs.mdx`
- [ ] `pages/api-reference/index.mdx`
- [ ] `pages/architecture/index.mdx`
- [ ] `pages/guides/temporal-queries.mdx`
- [ ] `pages/guides/plugin-system.mdx`
- [ ] `pages/guides/subgraphs.mdx`
- [ ] `pages/guides/hyperedges.mdx`
- [ ] `pages/changelog.mdx`
- [ ] `pages/contributing.mdx`

### AC-7.3: 빌드 성공

**Given** `docs-site/` 프로젝트가 존재할 때
**When** `cd docs-site && npm run build` 실행 시
**Then** 다음을 만족해야 한다:
- [ ] 빌드 성공 (exit code 0)
- [ ] 빌드 에러 0건
- [ ] `out/` 또는 `.next/` 디렉토리에 정적 파일 생성

### AC-7.4: UX 기능

**Given** 문서 사이트가 로컬에서 실행 중일 때
**When** 각 기능을 확인하면
**Then** 다음이 동작해야 한다:
- [ ] Dark mode / Light mode 토글
- [ ] 전문 검색 (검색어 입력 시 결과 표시)
- [ ] 반응형 레이아웃 (뷰포트 크기 변경 시 적응)
- [ ] Language Switcher (10개 언어 목록 표시)

### AC-7.5: i18n 설정

**Given** `docs-site/next.config.mjs`가 존재할 때
**When** i18n 설정을 확인하면
**Then** 다음이 포함되어야 한다:
- [ ] `i18n.locales` 배열에 10개 언어 코드: en, zh, hi, es, fr, ar, bn, pt, ru, ko
- [ ] `i18n.defaultLocale`: 'en'

### AC-7.6: SEO 메타데이터

**Given** 문서 사이트 각 페이지가 렌더링될 때
**When** HTML head를 확인하면
**Then** 다음이 포함되어야 한다:
- [ ] `<title>` 태그 (페이지별 고유)
- [ ] `<meta name="description">` 태그
- [ ] Open Graph 태그 (og:title, og:description, og:image)

### AC-7.7: MDX 콘텐츠

**Given** 문서 사이트의 각 페이지가 MDX 형식일 때
**When** 콘텐츠를 확인하면
**Then** 다음을 만족해야 한다:
- [ ] 모든 페이지가 `.mdx` 확장자
- [ ] 코드 블록에 언어 식별자 포함 (rust, python, go, javascript 등)
- [ ] Nextra 컴포넌트 활용 (Callout, Tabs 등) 권장

### AC-7.8: RTL 지원 (Arabic)

**Given** 문서 사이트에서 Arabic (ar) locale이 선택되었을 때
**When** 페이지 레이아웃을 확인하면
**Then** 다음이 적용되어야 한다:
- [ ] 텍스트 방향이 RTL (Right-to-Left)
- [ ] 네비게이션 메뉴가 오른쪽에 위치
- [ ] 코드 블록은 LTR 유지

---

## AC-8: Multi-Language Translation (M8)

### AC-8.1: README 번역 파일 존재

**Given** `docs/i18n/` 디렉토리가 존재할 때
**When** 번역 README 파일을 확인하면
**Then** 다음 9개 파일이 모두 존재해야 한다:
- [ ] `docs/i18n/README.zh.md` (中文)
- [ ] `docs/i18n/README.hi.md` (हिन्दी)
- [ ] `docs/i18n/README.es.md` (Español)
- [ ] `docs/i18n/README.fr.md` (Français)
- [ ] `docs/i18n/README.ar.md` (العربية)
- [ ] `docs/i18n/README.bn.md` (বাংলা)
- [ ] `docs/i18n/README.pt.md` (Português)
- [ ] `docs/i18n/README.ru.md` (Русский)
- [ ] `docs/i18n/README.ko.md` (한국어)

### AC-8.2: 번역 README 구조 동일성

**Given** 각 번역 README 파일이 존재할 때
**When** 영어 원본 README.md와 비교하면
**Then** 다음을 만족해야 한다:
- [ ] 섹션 순서가 영어 원본과 동일
- [ ] 배지가 영어 원본과 동일 (배지 URL은 번역하지 않음)
- [ ] 코드 블록 내용이 영어 원본과 동일 (코드는 번역하지 않음)
- [ ] 설명 텍스트가 해당 언어로 번역됨

### AC-8.3: 번역 메타데이터

**Given** 각 번역 README 파일이 존재할 때
**When** 파일 상단을 확인하면
**Then** 다음 메타데이터가 포함되어야 한다:
- [ ] Source Language: English
- [ ] Target Language 표시
- [ ] Last Synced Commit 해시
- [ ] Last Updated 날짜

### AC-8.4: 문서 사이트 i18n 페이지

**Given** `docs-site/` 프로젝트가 존재할 때
**When** locale별 페이지를 확인하면
**Then** 다음 콘텐츠가 10개 언어로 존재해야 한다:
- [ ] Landing page (10개 언어)
- [ ] Getting Started - Rust (10개 언어)
- [ ] Getting Started - Python (10개 언어)
- [ ] Getting Started - Go (10개 언어)
- [ ] Getting Started - Node.js (10개 언어)
- [ ] Architecture (10개 언어)
- [ ] Guide: Temporal Queries (10개 언어)
- [ ] Guide: Plugin System (10개 언어)
- [ ] Guide: Subgraphs (10개 언어)
- [ ] Guide: Hyperedges (10개 언어)
- [ ] Contributing (10개 언어)

### AC-8.5: 번역 제외 콘텐츠

**Given** 번역된 문서가 존재할 때
**When** 번역 범위를 확인하면
**Then** 다음 콘텐츠는 영어 원본 그대로 유지되어야 한다:
- [ ] CHANGELOG.md (English-only)
- [ ] 코드 블록 내 소스 코드 및 주석
- [ ] API Reference 자동 생성 문서 (docs.rs 링크)
- [ ] 변수명, 함수명, 패키지명 등 기술 식별자

### AC-8.6: 번역 가이드

**Given** `docs/i18n/` 디렉토리가 존재할 때
**When** 번역 가이드를 확인하면
**Then** 다음이 존재해야 한다:
- [ ] `docs/i18n/TRANSLATING.md` 파일 존재
- [ ] 번역 규칙 설명 (코드 블록 번역 금지 등)
- [ ] 번역 용어집 (주요 기술 용어의 각 언어별 표준 번역)
- [ ] PR 워크플로우 설명 (커뮤니티 기여 방법)

---

## AC-9: Deployment (M9)

### AC-9.1: 배포 워크플로우

**Given** 문서 사이트 배포가 설정되었을 때
**When** 배포 설정을 확인하면
**Then** 다음 중 하나가 존재해야 한다:
- [ ] `.github/workflows/docs.yml` (GitHub Pages 배포) 또는
- [ ] `docs-site/vercel.json` (Vercel 배포)

### AC-9.2: 자동 배포 트리거

**Given** 배포 워크플로우가 존재할 때
**When** 트리거 조건을 확인하면
**Then** 다음이 설정되어야 한다:
- [ ] `main` 브랜치 push 시 트리거
- [ ] `docs-site/**` 경로 필터 (문서 변경 시만 배포)

### AC-9.3: 10개 언어 접근성

**Given** 문서 사이트가 배포되었을 때
**When** 각 언어 URL에 접근하면
**Then** 다음을 만족해야 한다:
- [ ] English (`/`) HTTP 200
- [ ] 中文 (`/zh`) HTTP 200
- [ ] हिन्दी (`/hi`) HTTP 200
- [ ] Español (`/es`) HTTP 200
- [ ] Français (`/fr`) HTTP 200
- [ ] العربية (`/ar`) HTTP 200
- [ ] বাংলা (`/bn`) HTTP 200
- [ ] Português (`/pt`) HTTP 200
- [ ] Русский (`/ru`) HTTP 200
- [ ] 한국어 (`/ko`) HTTP 200

### AC-9.4: Language Switcher 동작

**Given** 배포된 문서 사이트에 접속한 상태에서
**When** Language Switcher를 클릭하면
**Then** 다음이 동작해야 한다:
- [ ] 10개 언어 목록이 드롭다운으로 표시
- [ ] 언어 선택 시 해당 locale 페이지로 이동
- [ ] 선택한 언어 설정이 브라우저에서 유지 (localStorage 또는 cookie)

---

## Definition of Done

SPEC-DOC-001은 다음 **모든** 조건이 충족될 때 완료로 간주한다:

### 필수 조건 (Must Have)

**Rust 프로젝트**:
- [ ] 전체 크레이트 v1.2.0 통일 (AC-1.1)
- [ ] `cargo test --workspace --all-features` 전체 통과 (AC-1.3)
- [ ] `cargo clippy --workspace --all-targets --all-features -- -D warnings` 경고 0건
- [ ] README.md가 AC-2 전체 기준을 충족 (배지, i18n 링크, Quick Start, Features, Architecture)
- [ ] CHANGELOG.md가 AC-3 전체 기준을 충족
- [ ] `cargo doc --workspace --all-features --no-deps` 경고 0건 (AC-4.3)
- [ ] 4개 크레이트 `cargo publish --dry-run` 성공 (AC-5.2)
- [ ] 5개 이상 예제 파일 존재 (AC-6.1, AC-6.2)
- [ ] Rust 예제 `cargo run --example` 정상 종료 (AC-6.3)
- [ ] LICENSE-MIT, LICENSE-APACHE 파일 존재

**문서 사이트**:
- [ ] `docs-site/` Nextra 프로젝트 존재 (AC-7.1)
- [ ] 13개 이상 필수 페이지 존재 (AC-7.2)
- [ ] `npm run build` 성공 (AC-7.3)
- [ ] i18n 10개 언어 설정 (AC-7.5)

**다국어 지원**:
- [ ] 9개 언어 README 번역 파일 존재 (AC-8.1)
- [ ] 번역 README 구조가 영어 원본과 동일 (AC-8.2)
- [ ] 문서 사이트 10개 언어 페이지 존재 (AC-8.4)
- [ ] 번역 제외 콘텐츠 규칙 준수 (AC-8.5)

**배포**:
- [ ] 배포 워크플로우 존재 (AC-9.1)
- [ ] 10개 언어 Landing page HTTP 200 (AC-9.3)

### 권장 조건 (Nice to Have)

- [ ] 추가 예제 (temporal_queries.rs, plugin_example.rs, agent_memory.rs)
- [ ] CONTRIBUTING.md에 PR 워크플로우 상세 설명
- [ ] Go/Python/Node.js 예제 실행 검증
- [ ] Dark mode / Light mode 토글 동작 (AC-7.4)
- [ ] 전문 검색 동작 (AC-7.4)
- [ ] Language Switcher 동작 (AC-9.4)
- [ ] 번역 가이드 `TRANSLATING.md` 존재 (AC-8.6)
- [ ] RTL (Arabic) 레이아웃 정상 동작 (AC-7.8)

### 검증 명령어 요약

```bash
# 1. 빌드 검증
cargo check --workspace --all-features

# 2. 테스트 검증
cargo test --workspace --all-features

# 3. 린트 검증
cargo clippy --workspace --all-targets --all-features -- -D warnings

# 4. 문서 빌드 검증
cargo doc --workspace --all-features --no-deps

# 5. 게시 검증 (순서 중요)
cargo publish --dry-run -p cypherlite-core
cargo publish --dry-run -p cypherlite-storage
cargo publish --dry-run -p cypherlite-query
cargo publish --dry-run -p cypherlite-ffi

# 6. 예제 실행 검증
cargo run --example basic_crud --all-features
cargo run --example knowledge_graph --all-features

# 7. 포맷 검증
cargo fmt --all --check

# 8. 문서 사이트 빌드 검증
cd docs-site && npm ci && npm run build

# 9. i18n 파일 존재 검증
ls docs/i18n/README.*.md | wc -l  # 9개 파일

# 10. 문서 사이트 로컬 검증
cd docs-site && npm run dev
# 수동: http://localhost:3000 + 각 locale 경로 확인
```
