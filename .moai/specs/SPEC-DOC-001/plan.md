# SPEC-DOC-001: Implementation Plan

| 필드 | 값 |
|------|-----|
| **SPEC ID** | SPEC-DOC-001 |
| **제목** | Documentation, Multi-Language i18n & Static Website (v1.2.0) |
| **대상 버전** | v1.2.0 |
| **개발 모드** | Documentation (TDD 면제 - 코드 구현이 아닌 문서/사이트 작성) |

---

## Milestone 개요

| Milestone | 제목 | 우선순위 | 의존성 | 핵심 산출물 |
|-----------|------|----------|--------|-------------|
| M1 | 버전 범프 v1.2.0 | Primary | 없음 | 전체 크레이트 1.2.0 |
| M2 | README.md 전면 개편 (English Base) | Primary | M1 | README.md |
| M3 | CHANGELOG.md + Examples | Primary | M1 | CHANGELOG.md, examples/ |
| M4 | Rustdoc 강화 | Primary | M1 | `cargo doc` 경고 0건 |
| M5 | crates.io 게시 준비 | Primary | M4, M1 | `cargo publish --dry-run` 성공 |
| M6 | Examples 강화 | Secondary | M1 | examples/ 디렉토리 5+ 파일 |
| M7 | Documentation Website (Nextra) | Primary | M2 | docs-site/ 빌드 성공 |
| M8 | Multi-Language Translation | Primary | M2, M7 | 9개 언어 README + 문서 사이트 i18n |
| M9 | Deployment | Primary | M7, M8 | GitHub Pages/Vercel 배포 완료 |

**실행 순서**:

```
M1 (Version Bump)
 │
 ├──> M2 (README English) ─────────┐
 ├──> M3 (CHANGELOG + Examples)    ├──> M7 (Nextra Website) ──> M8 (i18n Translation) ──> M9 (Deployment)
 └──> M4 (Rustdoc) + M5 (crates.io)
                                   │
                                   └──> M6 (Examples Enhancement)
```

- M1은 선행 필수 (모든 Milestone의 전제 조건)
- M2 + M3 + M6는 병렬 실행 가능
- M4 + M5는 병렬 실행 가능 (M1 완료 후)
- M7은 M2 완료 후 시작 (README 내용을 사이트에 반영)
- M8은 M2 + M7 완료 후 시작 (영어 원본 확정 후 번역)
- M9는 M7 + M8 완료 후 최종 배포

---

## M1: 버전 범프 v1.2.0

### 목표
모든 워크스페이스 크레이트를 v1.2.0으로 통일 범프한다.

### 기술 접근

**버전 범프 대상**:

| 크레이트 | 현재 버전 | 목표 버전 |
|----------|-----------|-----------|
| cypherlite-core | 1.0.0 | 1.2.0 |
| cypherlite-storage | 1.0.0 | 1.2.0 |
| cypherlite-query | 1.0.0 | 1.2.0 |
| cypherlite-ffi | 1.0.0 | 1.2.0 |
| cypherlite-python | 1.0.0 | 1.2.0 |
| cypherlite-node | 1.0.0 | 1.2.0 |

**inter-crate 의존성 업데이트**:
- `cypherlite-storage`의 `cypherlite-core` 의존성에 `version = "1.2.0"` 추가
- `cypherlite-query`의 core, storage 의존성에 `version = "1.2.0"` 추가
- `cypherlite-ffi`의 query, core 의존성에 `version = "1.2.0"` 추가
- `cypherlite-python`의 query, core 의존성에 `version = "1.2.0"` 추가
- `cypherlite-node`의 query, core 의존성에 `version = "1.2.0"` 추가

**검증**:
```bash
cargo check --workspace --all-features
cargo test --workspace --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

### 산출물
- 6개 Cargo.toml 버전 필드 업데이트
- `cargo test --workspace --all-features` 전체 통과

### 리스크

| 리스크 | 확률 | 영향 | 대응 |
|--------|------|------|------|
| inter-crate 호환성 문제 | Low | Medium | cargo check로 즉시 검출 |
| Cargo.lock 충돌 | Low | Low | `cargo update` 실행 |

### 관련 요구사항
R-DOC-050, R-DOC-051, R-DOC-052, R-DOC-053

---

## M2: README.md 전면 개편 (English Base)

### 목표
README.md를 v1.2.0 수준으로 완전히 재작성한다. 4개 언어 (Rust, Python, Go, Node.js) Quick Start를 포함하고, 모든 Phase 기능을 반영하며, 9개 번역 README 링크를 포함한다.

### 기술 접근

**배지 구성**:
- GitHub Actions CI 배지: `![CI](https://github.com/{owner}/cypherlite/actions/workflows/ci.yml/badge.svg)`
- crates.io 배지: `[![crates.io](https://img.shields.io/crates/v/cypherlite-query.svg)](https://crates.io/crates/cypherlite-query)`
- docs.rs 배지: `[![docs.rs](https://docs.rs/cypherlite-query/badge.svg)](https://docs.rs/cypherlite-query)`
- License 배지: `![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg)`
- MSRV 배지: `![MSRV](https://img.shields.io/badge/MSRV-1.84-orange.svg)`

**i18n 링크 테이블** (배지 바로 아래):
```
**Available in**: [中文](docs/i18n/README.zh.md) | [हिन्दी](docs/i18n/README.hi.md) | ...
```

**Quick Start 코드 예제 원칙**:
- 각 예제는 독립적으로 실행 가능해야 함
- 최소한의 코드로 핵심 기능 시연
- 에러 처리 포함 (`.unwrap()` 지양, `?` 연산자 사용)
- Phase 12 이후 FFI 기반 바인딩 API를 정확히 반영

**아키텍처 다이어그램**:
- ASCII art 기반 (이미지 의존성 없음)
- 기존 tech.md의 5-Layer 다이어그램 기반으로 FFI 레이어 추가
- 크레이트 의존성 그래프 포함

### 산출물
- `README.md` (전면 재작성)
- `CONTRIBUTING.md` (신규)
- `LICENSE-MIT` (신규)
- `LICENSE-APACHE` (신규)

### 리스크

| 리스크 | 확률 | 영향 | 대응 |
|--------|------|------|------|
| 바인딩 API 변경 | Low | Medium | 실제 코드 기반 예제 작성 |
| GitHub 저장소 URL 미확정 | Medium | Low | 플레이스홀더 사용 후 최종 치환 |
| 라이선스 미결정 | Medium | High | MIT OR Apache-2.0 듀얼 라이선스 제안 |

### 관련 요구사항
R-DOC-001, R-DOC-002, R-DOC-003, R-DOC-004, R-DOC-005, R-DOC-006, R-DOC-007

---

## M3: CHANGELOG.md 생성

### 목표
v0.1.0부터 v1.2.0까지 전체 변경 이력을 Keep a Changelog 형식으로 작성한다.

### 기술 접근

**변경 이력 수집 방법**:
1. Git 히스토리에서 각 Phase별 주요 변경 사항 추출
2. 기존 SPEC 문서 (SPEC-DB-001 ~ SPEC-FFI-001) 의 완료 기준 참조
3. 각 버전의 추가(Added), 변경(Changed), 수정(Fixed), 제거(Removed) 분류

**버전별 핵심 항목**:

| 버전 | SPEC | 핵심 변경 |
|------|------|-----------|
| v0.1.0 | SPEC-DB-001 | Storage Engine: WAL, B+Tree, Buffer Pool, ACID |
| v0.2.0 | SPEC-DB-002 | Query Engine: Cypher lexer, parser, planner, executor |
| v0.3.0 | SPEC-DB-003 | Advanced Query: MERGE, WITH, ORDER BY, optimizer |
| v0.4.0 | SPEC-DB-004 | Temporal Core: AT TIME, version store |
| v0.5.0 | SPEC-DB-005 | Temporal Edge: edge versioning, temporal relationship |
| v0.6.0 | SPEC-DB-006 | Subgraph Entities: SubgraphStore, CREATE/MATCH SNAPSHOT |
| v0.7.0 | SPEC-DB-007 | Native Hyperedge: N:M relations, HYPEREDGE syntax |
| v0.8.0 | SPEC-DB-008 | Inline Property Filter: {key: value} in MATCH |
| v0.9.0 | SPEC-INFRA-001 | CI/CD Pipeline: GitHub Actions 6 jobs |
| v1.0.0 | SPEC-PLUGIN-001 | Plugin System: 4 plugin types, PluginRegistry |
| v1.1.0 | SPEC-PERF-001 | Performance Optimization |
| v1.2.0 | SPEC-DOC-001 | Documentation, i18n & Website |

### 산출물
- `CHANGELOG.md` (신규)

### 관련 요구사항
R-DOC-010, R-DOC-011, R-DOC-012

---

## M4: Rustdoc 강화

### 목표
모든 public API에 doc comment를 추가하고, doctest를 포함하여 `cargo doc` 경고 0건을 달성한다.

### 기술 접근

**크레이트별 작업**:

1. **cypherlite-core**:
   - `lib.rs`: 크레이트 레벨 문서 추가 (핵심 타입과 트레이트 설명)
   - `types.rs`: `NodeId`, `EdgeId`, `PropertyValue` 등 모든 pub 타입 docstring
   - `error.rs`: `CypherLiteError` 변형별 설명
   - `plugin/mod.rs`: `Plugin`, `ScalarFunction`, `IndexPlugin`, `Serializer`, `Trigger` 트레이트 docstring + 예제

2. **cypherlite-storage**:
   - `lib.rs`: StorageEngine 크레이트 레벨 문서
   - `StorageEngine` 주요 pub 메서드 docstring

3. **cypherlite-query**:
   - `lib.rs`: 쿼리 엔진 크레이트 레벨 문서
   - `api/mod.rs`: `CypherLite`, `QueryResult`, `Row`, `Transaction` docstring
   - 주요 pub 함수에 `# Examples` doctest 추가

4. **cypherlite-ffi**:
   - `lib.rs`: C FFI 크레이트 레벨 문서 (안전성 가이드 포함)
   - 각 `extern "C"` 함수 docstring (C 호출 관점)

**docs.rs 설정**:
- 각 크레이트 Cargo.toml에 `[package.metadata.docs.rs]` 섹션 추가
- `all-features = true`로 전체 기능 문서화
- `#[cfg_attr(docsrs, doc(cfg(feature = "...")))]`로 feature-gated 항목 표시

### 산출물
- 4개 크레이트 `src/lib.rs` 업데이트
- 주요 pub 항목 doc comment 추가
- `cargo doc --workspace --all-features --no-deps` 경고 0건

### 리스크

| 리스크 | 확률 | 영향 | 대응 |
|--------|------|------|------|
| doctest 컴파일 실패 | Medium | Medium | doctest에서 `no_run` 속성 활용, 필요 시 `ignore` |
| 대량 pub 항목 | Medium | Low | 핵심 API 우선, 내부 유틸은 `#[doc(hidden)]` |
| FFI 함수 doctest 불가 | High | Low | FFI 함수는 설명 중심, `no_run` doctest |

### 관련 요구사항
R-DOC-020, R-DOC-021, R-DOC-022, R-DOC-023, R-DOC-024

---

## M5: crates.io 게시 준비

### 목표
4개 Rust 크레이트 (core, storage, query, ffi)가 `cargo publish --dry-run`을 통과하도록 준비한다.

### 기술 접근

**Cargo.toml 메타데이터 추가**:
- description, license, repository, homepage, keywords, categories, readme, authors
- `version` 필드가 workspace 전체에서 1.2.0으로 통일

**inter-crate 의존성 처리**:
- 로컬 개발: `path = "../cypherlite-core"` 유지
- crates.io 호환: `version = "1.2.0"` 추가
- 결과: `cypherlite-core = { version = "1.2.0", path = "../cypherlite-core" }`

**게시 순서 검증**:
```bash
# 1. core (의존성 없음)
cargo publish --dry-run -p cypherlite-core

# 2. storage (core에 의존)
cargo publish --dry-run -p cypherlite-storage

# 3. query (core + storage에 의존)
cargo publish --dry-run -p cypherlite-query

# 4. ffi (query + core에 의존)
cargo publish --dry-run -p cypherlite-ffi
```

**비게시 크레이트 처리**:
- `cypherlite-python`: Cargo.toml에 `publish = false` 추가 (PyPI로 별도 배포)
- `cypherlite-node`: Cargo.toml에 `publish = false` 추가 (npm으로 별도 배포)

### 산출물
- 6개 Cargo.toml 업데이트
- `cargo publish --dry-run` 4개 크레이트 성공

### 리스크

| 리스크 | 확률 | 영향 | 대응 |
|--------|------|------|------|
| crates.io 이름 충돌 | Medium | High | 사전에 `cargo search cypherlite` 확인 |
| inter-crate 버전 미스매치 | Low | High | 자동 검증 스크립트 작성 |
| 누락된 메타데이터 | Low | Medium | `cargo publish --dry-run` 에러 메시지 확인 |

### 관련 요구사항
R-DOC-030, R-DOC-031, R-DOC-032, R-DOC-033, R-DOC-034

---

## M6: Examples 강화

### 목표
`examples/` 디렉토리를 생성하고, Rust + Python + Go + Node.js 예제를 작성한다.

### 기술 접근

**Rust 예제** (cypherlite-query 크레이트 내 `examples/`):

1. `basic_crud.rs`:
   - `CypherLite::open()` -> CREATE -> MATCH -> SET -> DELETE -> RETURN
   - 에러 처리 `?` 연산자 사용
   - tempfile 기반 임시 DB (clean up 보장)

2. `knowledge_graph.rs`:
   - GraphRAG 시나리오: 도메인 엔티티 생성, 관계 구축, 다중 홉 쿼리
   - 플러그인 시스템 활용 (optional)

**바인딩 예제** (프로젝트 루트 `examples/`):

3. `python_quickstart.py`:
   - `import cypherlite` -> `db.open()` -> 기본 CRUD
   - PyO3 바인딩 API 기반

4. `go_quickstart.go`:
   - CGo 기반 CypherLite 사용
   - `cyl_db_open()` -> `cyl_db_execute()` -> 결과 읽기 -> `cyl_db_close()`

5. `node_quickstart.js`:
   - napi-rs 바인딩 API 기반
   - async/await 패턴 (Node.js 스타일)

### 산출물
- `examples/basic_crud.rs`
- `examples/knowledge_graph.rs`
- `examples/python_quickstart.py`
- `examples/go_quickstart.go`
- `examples/node_quickstart.js`

### 관련 요구사항
R-DOC-040, R-DOC-041, R-DOC-042, R-DOC-043, R-DOC-044, R-DOC-045

---

## M7: Documentation Website (Nextra)

### 목표
`docs-site/` 디렉토리에 Nextra 기반 정적 문서 사이트를 구축한다. English 페이지를 먼저 완성하고, i18n 인프라를 설정한다.

### 기술 접근

**프로젝트 초기화**:
```bash
mkdir docs-site && cd docs-site
npx create-next-app@latest . --typescript
npm install nextra nextra-theme-docs
```

**핵심 설정 파일**:

1. `next.config.mjs`:
   - Nextra 플러그인 설정
   - i18n locales 10개 언어 등록
   - Static export 설정 (GitHub Pages 호환)

2. `theme.config.tsx`:
   - 로고, 프로젝트 링크 (GitHub)
   - Dark mode 설정
   - Search 설정 (Flexsearch)
   - i18n Language Switcher 설정
   - Footer 설정

3. 페이지 구조 (English 기본):
   - `pages/index.mdx`: Landing page (프로젝트 개요, CTA)
   - `pages/getting-started/`: Rust, Python, Go, Node.js Quick Start
   - `pages/api-reference/index.mdx`: docs.rs 링크 + 바인딩 API 개요
   - `pages/architecture/index.mdx`: 5-Layer 아키텍처
   - `pages/guides/`: Temporal, Plugin, Subgraph, Hyperedge 가이드
   - `pages/changelog.mdx`: CHANGELOG 내용
   - `pages/contributing.mdx`: CONTRIBUTING 내용

**페이지 콘텐츠 소스**:
- README.md의 Feature/Quick Start 섹션을 Getting Started 페이지로 확장
- tech.md의 아키텍처 다이어그램을 Architecture 페이지로 확장
- CHANGELOG.md를 Changelog 페이지로 렌더링
- product.md의 사용 사례를 Landing page에 반영

**SEO 설정**:
- 각 페이지 `_meta.json`에 title, description 설정
- Open Graph 이미지 (`public/og-image.png`)
- `theme.config.tsx`에 `head` 설정

**RTL 지원 (Arabic)**:
- CSS `direction: rtl` 조건부 적용
- Nextra의 `useConfig()` 훅으로 현재 locale 감지
- `theme.config.tsx`에서 locale별 `dir` 속성 설정

### 산출물
- `docs-site/` 전체 프로젝트 (10+ 파일)
- `npm run build` 성공
- `npm run dev` 로컬 서버 정상 동작

### 리스크

| 리스크 | 확률 | 영향 | 대응 |
|--------|------|------|------|
| Nextra 버전 호환성 | Low | Medium | Nextra 공식 문서 기반 설정 |
| Static export와 i18n 충돌 | Medium | Medium | next.config.mjs에서 trailingSlash + output: export 설정 |
| RTL 레이아웃 깨짐 | Medium | Low | Arabic locale에서만 RTL 적용, 코드 블록은 LTR 유지 |
| 검색 인덱스 10개 언어 크기 | Low | Low | Flexsearch 자동 처리, 필요 시 locale별 인덱스 분리 |

### 관련 요구사항
R-DOC-060, R-DOC-061, R-DOC-062, R-DOC-063, R-DOC-064, R-DOC-065, R-DOC-066, R-DOC-067, R-DOC-068, R-DOC-069

---

## M8: Multi-Language Translation

### 목표
README.md와 문서 사이트 콘텐츠를 9개 추가 언어로 번역한다.

### 기술 접근

**번역 대상 및 범위**:

| 콘텐츠 | 번역 대상 | 비고 |
|--------|-----------|------|
| README.md | 전체 텍스트 (코드 블록 제외) | 9개 언어 x 1파일 = 9파일 |
| docs-site Landing page | 전체 텍스트 | 9개 locale 페이지 |
| Getting Started (4개) | 전체 텍스트 (코드 블록 제외) | 9 x 4 = 36 페이지 |
| Architecture | 전체 텍스트 | 9 페이지 |
| Feature Guides (4개) | 전체 텍스트 (코드 블록 제외) | 9 x 4 = 36 페이지 |
| Contributing | 전체 텍스트 | 9 페이지 |
| **합계** | | **README 9파일 + 사이트 ~99 페이지** |

**번역 워크플로우**:
1. English 원본 확정 (M2 + M7 완료)
2. 번역 파일 생성 (skeleton: 영어 원본 복사)
3. 텍스트 번역 (코드 블록은 유지)
4. 메타데이터 헤더 추가 (원본 커밋 해시, 날짜)
5. 리뷰 및 검증

**번역 우선순위** (화자 수 기준):
1. 중국어 (zh) - 1.1B
2. 힌디어 (hi) - 600M
3. 스페인어 (es) - 560M
4. 프랑스어 (fr) - 310M
5. 아랍어 (ar) - 310M
6. 벵골어 (bn) - 270M
7. 포르투갈어 (pt) - 260M
8. 러시아어 (ru) - 260M
9. 한국어 (ko) - 80M

**번역 파일 위치**:
- README 번역: `docs/i18n/README.{lang}.md`
- 문서 사이트 번역: Nextra i18n 파일 구조 (locale별 `_meta.{locale}.json` 또는 `pages/{locale}/`)

**번역 품질 관리**:
- 각 번역 파일에 원본 커밋 해시 기록
- 원본 업데이트 시 번역 파일 outdated 표시
- 용어 일관성을 위한 용어집 (`docs/i18n/TRANSLATING.md`)

### 산출물
- `docs/i18n/README.{zh,hi,es,fr,ar,bn,pt,ru,ko}.md` (9개 파일)
- `docs/i18n/TRANSLATING.md` (번역 가이드)
- `docs-site/pages/` 내 9개 locale 페이지 세트

### 리스크

| 리스크 | 확률 | 영향 | 대응 |
|--------|------|------|------|
| 번역 품질 불균일 | Medium | Medium | 커뮤니티 리뷰 PR 워크플로우 |
| 기술 용어 번역 불일치 | Medium | Medium | 용어집(glossary) 작성 |
| 대량 파일 관리 부담 | Medium | Low | 자동화 스크립트 (outdated 감지) |
| RTL 텍스트 렌더링 문제 | Low | Medium | Arabic 번역 시 RTL 테스트 병행 |

### 관련 요구사항
R-DOC-070, R-DOC-071, R-DOC-072, R-DOC-073, R-DOC-074, R-DOC-075, R-DOC-076, R-DOC-077, R-DOC-078, R-DOC-079

---

## M9: Deployment

### 목표
문서 사이트를 GitHub Pages 또는 Vercel에 배포하고, 10개 언어 모두 정상 접근 가능함을 검증한다.

### 기술 접근

**배포 옵션 A: GitHub Pages**:
- `.github/workflows/docs.yml` 워크플로우 생성
- 트리거: `main` 브랜치 push + `docs-site/**` 경로 필터
- 단계: checkout -> Node.js setup -> npm ci -> npm run build -> GitHub Pages action
- `next.config.mjs`에 `output: 'export'`, `basePath`, `trailingSlash: true` 설정

**배포 옵션 B: Vercel**:
- `docs-site/vercel.json` 설정
- Vercel 프로젝트 연결 (root directory: `docs-site/`)
- 자동 배포 (GitHub 연동)

**GitHub Actions 워크플로우 (Option A)**:
```yaml
name: Deploy Docs
on:
  push:
    branches: [main]
    paths: ['docs-site/**']
jobs:
  deploy:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-node@v4
        with:
          node-version: '20'
          cache: 'npm'
          cache-dependency-path: docs-site/package-lock.json
      - run: cd docs-site && npm ci
      - run: cd docs-site && npm run build
      - uses: actions/upload-pages-artifact@v3
        with:
          path: docs-site/out
      - uses: actions/deploy-pages@v4
```

**배포 후 검증**:
- 10개 언어 Landing page HTTP 200 확인
- 검색 기능 동작 확인
- Dark mode 토글 동작 확인
- 모바일 반응형 레이아웃 확인
- RTL (Arabic) 레이아웃 확인

### 산출물
- `.github/workflows/docs.yml` (또는 `docs-site/vercel.json`)
- 배포된 문서 사이트 URL
- 10개 언어 접근성 검증 결과

### 리스크

| 리스크 | 확률 | 영향 | 대응 |
|--------|------|------|------|
| GitHub Pages 빌드 실패 | Low | Medium | 로컬 빌드 검증 후 push |
| 커스텀 도메인 DNS 설정 | Low | Low | GitHub Pages 기본 URL로 시작 |
| Static export 제약 | Medium | Medium | Nextra 공식 문서의 static export 가이드 참조 |

### 관련 요구사항
R-DOC-080, R-DOC-081, R-DOC-082, R-DOC-083, R-DOC-084

---

## 전체 실행 전략

### 병렬 실행 가능 구간

```
Phase 1 (선행):
  M1: 버전 범프 v1.2.0

Phase 2 (병렬 가능):
  M2: README.md (English Base)   ─┐
  M3: CHANGELOG.md               ├─ 동시 진행 가능
  M6: Examples 강화               ─┘

Phase 3 (M1 완료 후, Phase 2와 병렬 가능):
  M4: Rustdoc 강화 ──> M5: crates.io 게시 준비

Phase 4 (M2 완료 후):
  M7: Documentation Website (Nextra)

Phase 5 (M2 + M7 완료 후):
  M8: Multi-Language Translation

Phase 6 (M7 + M8 완료 후):
  M9: Deployment
```

### 의존성 관계

```
M1 (Version Bump)
 ├──> M2 (README English)
 │     ├──> M7 (Nextra Website)
 │     │     ├──> M8 (i18n Translation)
 │     │     │     └──> M9 (Deployment)
 │     │     └──> M9 (Deployment)
 │     └──> M8 (i18n Translation)
 ├──> M3 (CHANGELOG)
 ├──> M4 (Rustdoc)
 │     └──> M5 (crates.io)
 └──> M6 (Examples)
```

### 품질 게이트

| 게이트 | 기준 | 시점 |
|--------|------|------|
| 빌드 검증 | `cargo check --workspace --all-features` 에러 0 | M1 완료 후 |
| 테스트 검증 | `cargo test --workspace --all-features` 전체 통과 | M1 완료 후 |
| 문서 검증 | `cargo doc --workspace --all-features --no-deps` 경고 0 | M4 완료 후 |
| 게시 검증 | `cargo publish --dry-run` 4개 크레이트 성공 | M5 완료 후 |
| 예제 검증 | `cargo run --example basic_crud` 정상 종료 | M6 완료 후 |
| 린트 검증 | `cargo clippy --workspace --all-targets --all-features -- -D warnings` | 전체 완료 후 |
| 사이트 빌드 검증 | `cd docs-site && npm run build` 성공 | M7 완료 후 |
| i18n 검증 | 10개 언어 README 존재 + 사이트 locale 동작 | M8 완료 후 |
| 배포 검증 | 10개 언어 Landing page HTTP 200 | M9 완료 후 |
