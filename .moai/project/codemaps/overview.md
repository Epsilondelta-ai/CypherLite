# CypherLite 아키텍처 개요 (플레이스홀더)

> 구현 코드가 없는 설계 단계입니다. Phase 1 구현 후 `/moai codemaps`를 실행하면 실제 아키텍처 맵이 생성됩니다.

---

## 프로젝트 목표

CypherLite는 **"그래프 데이터베이스의 SQLite"** 를 목표로 합니다.

- 단일 `.cyl` 파일에 전체 데이터베이스 저장
- openCypher 쿼리 언어 지원
- ACID 트랜잭션 (WAL 기반)
- 인프로세스 임베딩, 서버 불필요

## 계획된 모듈 구조

```
cypherlite-core     ← 공통 타입, 에러, 설정
       ↓
cypherlite-storage  ← 파일 I/O, 페이지 관리, B-tree, WAL
       ↓
cypherlite-query    ← Lexer, Parser, AST, Planner, Executor
       ↓
cypherlite-plugin   ← 플러그인 트레이트, 레지스트리
       ↓
cypherlite-ffi      ← C FFI (cbindgen)
cypherlite-python   ← PyO3 Python 바인딩
cypherlite-node     ← neon Node.js 바인딩
```

## 구현 단계

| 단계 | 버전 | 기간 | 주요 모듈 |
|------|------|------|----------|
| Phase 1 | v0.1 | Weeks 1-4 | cypherlite-storage |
| Phase 2 | v0.2 | Weeks 5-10 | cypherlite-query |
| Phase 3 | v0.3 | Weeks 11-14 | cypherlite-plugin |
| Phase 4 | v0.4 | Weeks 15-18 | temporal 확장 |
| Phase 5 | v0.5 | Weeks 19-22 | cypherlite-ffi/python/node |
| Phase 6 | v1.0 | Weeks 23-28 | 프로덕션 강화 |

---

*이 파일은 Phase 1 구현 후 실제 코드 분석으로 업데이트됩니다.*
