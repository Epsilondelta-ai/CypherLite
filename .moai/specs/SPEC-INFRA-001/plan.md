---
id: SPEC-INFRA-001
type: plan
version: "0.9.0"
status: approved
created: "2026-03-13"
updated: "2026-03-13"
author: epsilondelta
tags: [ci-cd, github-actions, quality-gate]
traceability:
  spec: spec.md
  acceptance: acceptance.md
---

# SPEC-INFRA-001 구현 계획: CI/CD Pipeline

## 1. 구현 단계

### Phase 9a: Core CI 워크플로우 (Primary Goal)

**목표**: 기본 CI 파이프라인 구축 (test + clippy + fmt + MSRV)

**생성 파일**:
- `.github/workflows/ci.yml`

**작업 내용**:

1. **CI 워크플로우 파일 생성**
   - push(main) 및 PR(main) 트리거 설정
   - 5개 병렬 job 구조 정의

2. **check job 구현**
   - `cargo clippy --workspace --all-targets -- -D warnings`
   - `cargo fmt --all -- --check`
   - Rust stable 툴체인 사용

3. **msrv job 구현**
   - Rust 1.84 툴체인 설치
   - `cargo check --workspace --all-features`
   - MSRV 호환성 검증

4. **test job 구현**
   - `cargo test --workspace --all-features`
   - 테스트 결과 출력

5. **캐싱 전략 적용**
   - `actions/cache` 를 활용한 Cargo 레지스트리/target 캐싱
   - 캐시 키: `${{ runner.os }}-cargo-${{ hashFiles('**/Cargo.lock') }}`
   - 복원 키: `${{ runner.os }}-cargo-`

**검증 기준**:
- REQ-CI-001: 모든 테스트 통과
- REQ-CI-002: clippy 경고 0개
- REQ-CI-003: fmt 위반 0개
- REQ-CI-004: MSRV 1.84 호환
- REQ-NFR-002: 캐싱 적용

---

### Phase 9b: 커버리지 게이트 (Secondary Goal)

**목표**: 커버리지 측정 및 85% 임계값 적용

**수정/생성 파일**:
- `.github/workflows/ci.yml` (coverage job 추가)

**작업 내용**:

1. **coverage job 추가**
   - `rustup component add llvm-tools-preview`
   - `cargo install cargo-llvm-cov`
   - `cargo llvm-cov --workspace --all-features --fail-under-lines 85`

2. **커버리지 요약 출력**
   - `--summary-only` 플래그로 CI 로그에 요약 출력
   - 실패 시 명확한 에러 메시지 제공

**검증 기준**:
- REQ-COV-001: 커버리지 측정 실행
- REQ-COV-002: 85% 미만 시 실패
- REQ-COV-003: 커버리지 요약 출력

---

### Phase 9c: 보안 + Dependabot (Final Goal)

**목표**: 보안 감사 자동화 및 의존성 관리

**생성 파일**:
- `.github/dependabot.yml`
- `.github/workflows/ci.yml` (security + bench-check job 추가)

**작업 내용**:

1. **security job 추가**
   - `cargo install cargo-audit`
   - `cargo audit`
   - 취약점 발견 시 CI 실패

2. **bench-check job 추가**
   - `cargo bench --workspace --no-run`
   - 벤치마크 컴파일 확인 (실행 안 함)

3. **Dependabot 구성 생성**
   - Cargo 의존성: 주 1회 자동 확인
   - GitHub Actions 의존성: 주 1회 자동 확인

**검증 기준**:
- REQ-SEC-001: cargo audit 실행
- REQ-SEC-002: 취약점 발견 시 실패
- REQ-BENCH-001: 벤치마크 컴파일 확인
- REQ-DEP-001: Cargo Dependabot 활성
- REQ-DEP-002: Actions Dependabot 활성

---

## 2. 기술적 접근 방식

### 2.1 워크플로우 구조: 단일 파일 + 병렬 Job

**선택 이유**:
- 단일 `ci.yml` 파일로 모든 CI 작업을 관리하여 유지보수 단순화
- 각 job이 독립적으로 병렬 실행되어 전체 실행 시간 최소화
- job 간 의존성이 없으므로 하나의 실패가 다른 job을 차단하지 않음

**대안 고려**:
- 다중 워크플로우 파일: 관리 복잡성 증가, 현재 규모에 불필요
- 단일 job + 순차 실행: 전체 실행 시간이 길어짐

### 2.2 캐싱 전략

```
캐시 구조:
  ~/.cargo/registry/index  -- Cargo 패키지 인덱스
  ~/.cargo/registry/cache  -- 다운로드된 크레이트
  ~/.cargo/git/db          -- Git 의존성
  target/                  -- 빌드 결과물
```

**캐시 키 전략**:
- Primary: OS + Cargo.lock hash (정확한 의존성 매칭)
- Fallback: OS 접두사 (부분 캐시 복원)

### 2.3 커버리지 도구 선택

**cargo-llvm-cov 선택 이유**:
- 프로젝트에서 이미 사용 중 (research.md 확인)
- `--fail-under-lines` 플래그로 임계값 자동 적용 가능
- LLVM 기반으로 정확한 라인 커버리지 측정
- 워크스페이스 전체 통합 커버리지 지원

### 2.4 Toolchain 캐싱

`dtolnay/rust-toolchain` 액션을 사용하여 Rust 툴체인을 설치한다. 이 액션은 자체 캐싱을 지원하며, 복수 컴포넌트(clippy, rustfmt, llvm-tools-preview) 설치를 간편하게 처리한다.

---

## 3. 생성/수정 파일 목록

| 파일 | 동작 | Phase | 설명 |
|------|------|-------|------|
| `.github/workflows/ci.yml` | 생성 | 9a, 9b, 9c | 메인 CI 워크플로우 |
| `.github/dependabot.yml` | 생성 | 9c | Dependabot 구성 |

**참고**: Cargo.toml 수정은 불필요하다. 모든 필요한 도구는 CI 환경에서 설치한다.

---

## 4. CI Job 구조 상세

```
ci.yml
  |
  +-- check (clippy + fmt)
  |     Toolchain: stable
  |     Steps: clippy, fmt --check
  |
  +-- msrv (MSRV 호환성)
  |     Toolchain: 1.84
  |     Steps: cargo check --workspace --all-features
  |
  +-- test (테스트 실행)
  |     Toolchain: stable
  |     Steps: cargo test --workspace --all-features
  |
  +-- coverage (커버리지 게이트)
  |     Toolchain: stable + llvm-tools-preview
  |     Steps: install cargo-llvm-cov, run with --fail-under-lines 85
  |
  +-- security (보안 감사)
  |     Toolchain: stable
  |     Steps: install cargo-audit, cargo audit
  |
  +-- bench-check (벤치마크 컴파일)
        Toolchain: stable
        Steps: cargo bench --workspace --no-run
```

모든 job은 병렬 실행되며, 각 job은 독립적인 러너에서 수행된다.

---

## 5. 리스크 분석

### R1: CI 실행 시간 초과

- **위험**: --all-features 테스트(1,256개) + proptest 슬로우 테스트로 10분 초과 가능
- **확률**: 중간
- **대응**: 캐싱 최적화, proptest 케이스 수 CI 환경 조정 검토

### R2: cargo-llvm-cov 설치 시간

- **위험**: CI에서 cargo-llvm-cov 바이너리 컴파일에 시간 소요
- **확률**: 낮음
- **대응**: `cargo install` 대신 `taiki-e/install-action` 으로 사전 빌드된 바이너리 설치

### R3: cargo-audit 오탐지

- **위험**: 사용하지 않는 취약점이나 패치 미적용 의존성으로 인한 빌드 실패
- **확률**: 낮음
- **대응**: `--ignore` 플래그로 알려진 오탐지 제외 가능

### R4: GitHub Actions 무료 티어 제한

- **위험**: 월 2,000분 제한 소진
- **확률**: 낮음 (현재 기여자 수 소수)
- **대응**: 캐싱으로 실행 시간 최소화, 필요시 조건부 트리거 추가

### R5: Dependabot PR 과다 생성

- **위험**: 다수의 의존성 업데이트 PR이 동시 생성
- **확률**: 중간
- **대응**: `open-pull-requests-limit` 설정으로 동시 PR 수 제한

---

## 6. 전문가 상담 권장

### DevOps 전문가 (expert-devops)

이 SPEC은 CI/CD 인프라에 관한 것으로, 구현 시 expert-devops 에이전트 상담을 권장한다:

- GitHub Actions 워크플로우 최적화
- 캐싱 전략 검증
- 보안 모범 사례 확인
- 실행 시간 최적화

---

## 7. 추적성

| Phase | 요구사항 | 수락 기준 |
|-------|----------|-----------|
| 9a | REQ-CI-001~004, REQ-NFR-002 | AC-001~005, AC-009 |
| 9b | REQ-COV-001~003 | AC-003 |
| 9c | REQ-SEC-001~002, REQ-BENCH-001, REQ-DEP-001~002 | AC-006~008 |
