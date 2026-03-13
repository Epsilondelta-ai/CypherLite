---
id: SPEC-INFRA-001
title: "CI/CD Pipeline - GitHub Actions"
version: "0.9.0"
status: approved
created: "2026-03-13"
updated: "2026-03-13"
author: epsilondelta
priority: high
tags: [ci-cd, github-actions, quality-gate, automation]
traceability:
  plan: plan.md
  acceptance: acceptance.md
  research: research.md
---

# SPEC-INFRA-001: CI/CD Pipeline - GitHub Actions

## 1. 개요

CypherLite 프로젝트에 GitHub Actions 기반 CI/CD 파이프라인을 구축한다. 현재 프로젝트에는 자동화된 품질 게이트가 전혀 존재하지 않으며, 모든 검증이 수동으로 이루어지고 있다. 이 SPEC은 테스트, 린팅, 포매팅, 커버리지, 보안 감사를 자동화하여 코드 품질을 보장하는 CI 파이프라인을 정의한다.

### 1.1 배경

- **워크스페이스**: 3개 크레이트 (cypherlite-core, cypherlite-storage, cypherlite-query), 모두 v0.8.0
- **에디션**: Rust 2021, MSRV: Rust 1.84+
- **피처 체인**: temporal-core -> temporal-edge -> subgraph -> hypergraph -> full-temporal
- **테스트**: 기본 피처 306개, --all-features 시 1,256개
- **커버리지**: 현재 93%+ (목표 85%)
- **벤치마크**: Criterion 0.5, 6개 벤치마크 타겟

### 1.2 목적

- PR 및 push 시 자동 품질 검증
- 코드 커버리지 85% 미만 시 PR 차단
- 보안 취약점 자동 감지
- 의존성 업데이트 자동화

---

## 2. 환경 (Environment)

| 항목 | 값 |
|------|-----|
| CI 플랫폼 | GitHub Actions |
| Rust 버전 | stable + MSRV 1.84 |
| OS | ubuntu-latest |
| 캐싱 | actions/cache (Cargo registry, target) |
| 커버리지 도구 | cargo-llvm-cov (llvm-tools-preview 필요) |
| 보안 감사 | cargo-audit |
| 의존성 관리 | Dependabot |

---

## 3. 가정 (Assumptions)

- A1: GitHub Actions 무료 티어(2,000분/월)가 초기 운영에 충분하다.
- A2: cargo-llvm-cov가 워크스페이스 전체의 통합 커버리지를 정확히 측정한다.
- A3: 모든 1,256개 테스트(--all-features)가 CI 환경에서 10분 이내 완료된다.
- A4: cargo-audit 데이터베이스가 Rust 의존성의 알려진 취약점을 충분히 커버한다.
- A5: Dependabot이 Cargo.toml/Cargo.lock 의존성을 정확히 감지하고 PR을 생성한다.

---

## 4. 요구사항 (Requirements)

### 4.1 CI 워크플로우 (핵심)

#### REQ-CI-001: 테스트 실행 (Event-Driven)

**WHEN** main 브랜치에 push 하거나 main을 대상으로 하는 PR이 열리면, **THEN** 시스템은 `cargo test --workspace --all-features` 를 실행하여 모든 테스트가 통과하는지 검증해야 한다.

#### REQ-CI-002: Clippy 린팅 (Event-Driven)

**WHEN** CI 워크플로우가 트리거되면, **THEN** 시스템은 `cargo clippy --workspace --all-targets -- -D warnings` 를 실행하여 모든 clippy 경고가 없음을 검증해야 한다.

#### REQ-CI-003: Rustfmt 포매팅 검사 (Event-Driven)

**WHEN** CI 워크플로우가 트리거되면, **THEN** 시스템은 `cargo fmt --all -- --check` 를 실행하여 모든 코드가 표준 포매팅을 준수하는지 검증해야 한다.

#### REQ-CI-004: MSRV 호환성 검증 (Event-Driven)

**WHEN** CI 워크플로우가 트리거되면, **THEN** 시스템은 Rust 1.84(MSRV)에서 `cargo check --workspace --all-features` 를 실행하여 하위 호환성을 검증해야 한다.

### 4.2 커버리지 게이트

#### REQ-COV-001: 커버리지 측정 (Event-Driven)

**WHEN** main을 대상으로 하는 PR이 열리면, **THEN** 시스템은 `cargo llvm-cov --workspace --all-features` 를 실행하여 라인 커버리지를 측정해야 한다.

#### REQ-COV-002: 커버리지 임계값 (State-Driven + Unwanted)

**IF** 측정된 라인 커버리지가 85% 미만이면, **THEN** 시스템은 해당 CI 작업을 실패 처리하여 PR 머지를 차단해야 한다. 시스템은 커버리지가 85% 미만인 PR의 머지를 **허용하지 않아야 한다**.

#### REQ-COV-003: 커버리지 리포트 (Event-Driven)

**WHEN** 커버리지 측정이 완료되면, **THEN** 시스템은 커버리지 요약을 CI 로그에 출력해야 한다.

### 4.3 보안 감사

#### REQ-SEC-001: 의존성 보안 감사 (Event-Driven)

**WHEN** CI 워크플로우가 트리거되면, **THEN** 시스템은 `cargo audit` 를 실행하여 알려진 취약점이 있는 의존성을 감지해야 한다.

#### REQ-SEC-002: 보안 감사 실패 처리 (State-Driven + Unwanted)

**IF** cargo audit가 취약점을 발견하면, **THEN** 시스템은 해당 CI 작업을 실패 처리해야 한다. 시스템은 알려진 보안 취약점이 있는 상태에서의 머지를 **허용하지 않아야 한다**.

### 4.4 벤치마크 검증

#### REQ-BENCH-001: 벤치마크 컴파일 확인 (Event-Driven)

**WHEN** CI 워크플로우가 트리거되면, **THEN** 시스템은 `cargo bench --workspace --no-run` 를 실행하여 벤치마크 코드가 컴파일되는지 검증해야 한다 (실행하지 않음).

### 4.5 의존성 자동 업데이트

#### REQ-DEP-001: Dependabot 구성 (Ubiquitous)

시스템은 **항상** Dependabot을 통해 Cargo 의존성 업데이트를 주 1회 자동 확인해야 한다.

#### REQ-DEP-002: GitHub Actions 업데이트 (Ubiquitous)

시스템은 **항상** Dependabot을 통해 GitHub Actions 의존성 업데이트를 주 1회 자동 확인해야 한다.

### 4.6 비기능 요구사항

#### REQ-NFR-001: CI 실행 시간 (State-Driven)

**IF** 캐시가 적중(hit)한 상태이면, **THEN** 전체 CI 워크플로우 실행 시간은 10분을 초과하지 않아야 한다.

#### REQ-NFR-002: 캐싱 전략 (Ubiquitous)

시스템은 **항상** Cargo registry, Cargo index, target 디렉토리를 캐싱하여 빌드 시간을 최적화해야 한다.

#### REQ-NFR-003: 병렬 실행 (Optional)

**가능하면** 독립적인 CI 작업(clippy, fmt, test, audit)을 병렬로 실행하여 전체 파이프라인 시간을 단축해야 한다.

---

## 5. 명세 (Specifications)

### 5.1 워크플로우 아키텍처

단일 워크플로우 파일(`.github/workflows/ci.yml`)에 여러 job을 병렬 실행하는 구조를 채택한다.

```
ci.yml
  |
  +-- job: check (clippy + fmt + MSRV check)
  +-- job: test (cargo test --workspace --all-features)
  +-- job: coverage (cargo-llvm-cov, 85% gate)
  +-- job: security (cargo audit)
  +-- job: bench-check (cargo bench --no-run)
```

### 5.2 트리거 조건

```yaml
on:
  push:
    branches: [main]
  pull_request:
    branches: [main]
```

### 5.3 캐싱 전략

- **캐시 키**: `${{ runner.os }}-cargo-${{ hashFiles('**/Cargo.lock') }}`
- **캐시 경로**: `~/.cargo/registry`, `~/.cargo/git`, `target/`
- **복원 키**: `${{ runner.os }}-cargo-` (부분 캐시 복원)

### 5.4 커버리지 임계값 적용

cargo-llvm-cov의 `--fail-under-lines` 플래그를 사용하여 85% 미만 시 자동 실패 처리한다.

```bash
cargo llvm-cov --workspace --all-features --fail-under-lines 85
```

### 5.5 Dependabot 구성

```yaml
# .github/dependabot.yml
version: 2
updates:
  - package-ecosystem: "cargo"
    directory: "/"
    schedule:
      interval: "weekly"
  - package-ecosystem: "github-actions"
    directory: "/"
    schedule:
      interval: "weekly"
```

---

## 6. 제약사항 (Constraints)

- C1: cargo-llvm-cov 실행에 `llvm-tools-preview` rustup 컴포넌트가 필요하다.
- C2: 일부 벤치마크는 피처 플래그(subgraph, hypergraph)가 필요하다.
- C3: proptest 슬로우 테스트는 15초 이상 소요될 수 있다.
- C4: GitHub Actions 무료 티어 제한(2,000분/월)을 고려해야 한다.

---

## 7. 추적성 (Traceability)

| 요구사항 | 구현 파일 | 테스트 |
|----------|-----------|--------|
| REQ-CI-001~004 | .github/workflows/ci.yml | acceptance.md AC-001~005 |
| REQ-COV-001~003 | .github/workflows/ci.yml (coverage job) | acceptance.md AC-003 |
| REQ-SEC-001~002 | .github/workflows/ci.yml (security job) | acceptance.md AC-006 |
| REQ-BENCH-001 | .github/workflows/ci.yml (bench-check job) | acceptance.md AC-007 |
| REQ-DEP-001~002 | .github/dependabot.yml | acceptance.md AC-008 |
| REQ-NFR-001~003 | .github/workflows/ci.yml (caching, parallel jobs) | acceptance.md AC-009 |
