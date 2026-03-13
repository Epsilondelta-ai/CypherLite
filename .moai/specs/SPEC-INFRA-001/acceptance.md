---
id: SPEC-INFRA-001
type: acceptance
version: "0.9.0"
status: approved
created: "2026-03-13"
updated: "2026-03-13"
author: epsilondelta
tags: [ci-cd, github-actions, quality-gate]
traceability:
  spec: spec.md
  plan: plan.md
---

# SPEC-INFRA-001 수락 기준: CI/CD Pipeline

## AC-001: Push 시 CI 트리거 및 전체 검사 통과

**요구사항**: REQ-CI-001, REQ-CI-002, REQ-CI-003

```gherkin
Scenario: main 브랜치 push 시 CI 워크플로우 실행
  Given main 브랜치에 모든 테스트가 통과하는 코드가 있다
  And 모든 코드가 clippy 경고 없이 통과한다
  And 모든 코드가 rustfmt 포매팅을 준수한다
  When 개발자가 main 브랜치에 push 한다
  Then CI 워크플로우가 자동으로 트리거된다
  And check job이 clippy와 fmt 검사를 수행한다
  And test job이 모든 테스트를 실행한다
  And 모든 job이 성공 상태로 완료된다
```

---

## AC-002: PR에서 테스트 실패 시 머지 차단

**요구사항**: REQ-CI-001

```gherkin
Scenario: 실패하는 테스트가 포함된 PR
  Given main 을 대상으로 하는 PR이 열려있다
  And PR 코드에 실패하는 테스트가 포함되어 있다
  When CI 워크플로우가 실행된다
  Then test job이 실패 상태를 반환한다
  And PR의 CI 체크가 실패로 표시된다
  And PR 머지가 차단된다 (branch protection rule 설정 시)
```

---

## AC-003: 커버리지 85% 미만 시 PR 실패

**요구사항**: REQ-COV-001, REQ-COV-002, REQ-COV-003

```gherkin
Scenario: 커버리지가 임계값 미만인 PR
  Given main 을 대상으로 하는 PR이 열려있다
  And PR 코드의 라인 커버리지가 85% 미만이다
  When coverage job이 cargo llvm-cov --fail-under-lines 85 를 실행한다
  Then coverage job이 실패 상태를 반환한다
  And CI 로그에 현재 커버리지 수치가 출력된다
  And PR 머지가 차단된다

Scenario: 커버리지가 임계값 이상인 PR
  Given main 을 대상으로 하는 PR이 열려있다
  And PR 코드의 라인 커버리지가 85% 이상이다
  When coverage job이 cargo llvm-cov --fail-under-lines 85 를 실행한다
  Then coverage job이 성공 상태를 반환한다
  And CI 로그에 현재 커버리지 수치가 출력된다
```

---

## AC-004: Clippy 경고 시 CI 실패

**요구사항**: REQ-CI-002

```gherkin
Scenario: Clippy 경고가 있는 코드
  Given PR 코드에 clippy 경고를 유발하는 코드가 포함되어 있다
  When check job이 cargo clippy --workspace --all-targets -- -D warnings 를 실행한다
  Then check job이 실패 상태를 반환한다
  And CI 로그에 clippy 경고 내용이 출력된다
  And PR의 CI 체크가 실패로 표시된다
```

---

## AC-005: Rustfmt 위반 시 CI 실패

**요구사항**: REQ-CI-003

```gherkin
Scenario: 포매팅 미준수 코드
  Given PR 코드에 rustfmt 표준을 따르지 않는 코드가 포함되어 있다
  When check job이 cargo fmt --all -- --check 를 실행한다
  Then check job이 실패 상태를 반환한다
  And CI 로그에 포매팅 위반 파일 목록이 출력된다
  And PR의 CI 체크가 실패로 표시된다
```

---

## AC-006: 보안 취약점 발견 시 CI 실패

**요구사항**: REQ-SEC-001, REQ-SEC-002

```gherkin
Scenario: 알려진 취약점이 있는 의존성
  Given 프로젝트 의존성 중 알려진 보안 취약점이 있다
  When security job이 cargo audit 를 실행한다
  Then security job이 실패 상태를 반환한다
  And CI 로그에 취약점 상세 정보(CVE 번호, 영향 크레이트, 심각도)가 출력된다

Scenario: 알려진 취약점이 없는 의존성
  Given 프로젝트 의존성에 알려진 보안 취약점이 없다
  When security job이 cargo audit 를 실행한다
  Then security job이 성공 상태를 반환한다
```

---

## AC-007: 벤치마크 컴파일 확인

**요구사항**: REQ-BENCH-001

```gherkin
Scenario: 벤치마크 코드 컴파일 성공
  Given 모든 벤치마크 코드가 유효하다
  When bench-check job이 cargo bench --workspace --no-run 를 실행한다
  Then bench-check job이 성공 상태를 반환한다
  And 벤치마크 바이너리가 컴파일되지만 실행되지는 않는다

Scenario: 벤치마크 코드 컴파일 실패
  Given 벤치마크 코드에 컴파일 에러가 있다
  When bench-check job이 cargo bench --workspace --no-run 를 실행한다
  Then bench-check job이 실패 상태를 반환한다
  And CI 로그에 컴파일 에러 내용이 출력된다
```

---

## AC-008: Dependabot 의존성 업데이트 PR 생성

**요구사항**: REQ-DEP-001, REQ-DEP-002

```gherkin
Scenario: 오래된 Cargo 의존성 감지
  Given .github/dependabot.yml 에 Cargo 에코시스템이 구성되어 있다
  And 프로젝트 의존성 중 새 버전이 사용 가능한 크레이트가 있다
  When Dependabot 주간 스케줄이 실행된다
  Then Dependabot이 해당 의존성의 버전 업데이트 PR을 생성한다
  And PR 제목에 업데이트 크레이트 이름과 버전이 포함된다

Scenario: GitHub Actions 의존성 업데이트
  Given .github/dependabot.yml 에 github-actions 에코시스템이 구성되어 있다
  And 사용 중인 GitHub Actions에 새 버전이 있다
  When Dependabot 주간 스케줄이 실행된다
  Then Dependabot이 해당 Actions의 버전 업데이트 PR을 생성한다
```

---

## AC-009: CI 실행 시간 최적화

**요구사항**: REQ-NFR-001, REQ-NFR-002, REQ-NFR-003

```gherkin
Scenario: 캐시 적중 시 빌드 시간 최적화
  Given 이전 CI 실행에서 Cargo 캐시가 저장되었다
  And Cargo.lock 파일이 변경되지 않았다
  When CI 워크플로우가 트리거된다
  Then 캐시가 복원된다 (cache hit)
  And 의존성 다운로드/컴파일이 생략된다
  And 전체 CI 실행 시간이 10분 이내이다

Scenario: 병렬 Job 실행
  Given CI 워크플로우가 트리거되었다
  When 각 job이 시작된다
  Then check, test, coverage, security, bench-check job이 동시에 실행된다
  And 하나의 job 실패가 다른 job의 실행을 중단시키지 않는다
```

---

## AC-010: MSRV 호환성 검증

**요구사항**: REQ-CI-004

```gherkin
Scenario: MSRV 1.84 호환 코드
  Given 모든 코드가 Rust 1.84에서 지원하는 기능만 사용한다
  When msrv job이 Rust 1.84 툴체인으로 cargo check --workspace --all-features 를 실행한다
  Then msrv job이 성공 상태를 반환한다

Scenario: MSRV 비호환 코드
  Given 코드에 Rust 1.84에서 지원하지 않는 기능이 사용되었다
  When msrv job이 Rust 1.84 툴체인으로 cargo check --workspace --all-features 를 실행한다
  Then msrv job이 실패 상태를 반환한다
  And CI 로그에 호환성 에러 내용이 출력된다
```

---

## Definition of Done

- [ ] `.github/workflows/ci.yml` 파일이 생성되었다
- [ ] `.github/dependabot.yml` 파일이 생성되었다
- [ ] CI가 push(main) 및 PR(main) 시 자동 트리거된다
- [ ] check job: clippy -D warnings + fmt --check 통과
- [ ] msrv job: Rust 1.84에서 cargo check 통과
- [ ] test job: cargo test --workspace --all-features 통과
- [ ] coverage job: 85% 미만 시 실패
- [ ] security job: cargo audit 실행 및 취약점 시 실패
- [ ] bench-check job: cargo bench --workspace --no-run 통과
- [ ] Dependabot: Cargo + GitHub Actions 주간 업데이트 구성
- [ ] 캐싱: Cargo registry/target 캐싱 적용
- [ ] 모든 job이 병렬 실행된다
- [ ] 캐시 적중 시 전체 CI 10분 이내 완료

---

## 검증 방법

### 수동 검증

1. **CI 트리거 확인**: 테스트 브랜치에서 PR을 생성하여 CI 자동 실행 확인
2. **실패 시나리오**: 의도적으로 clippy 경고/fmt 위반을 포함한 PR로 실패 확인
3. **커버리지 게이트**: 테스트를 제거한 PR로 커버리지 실패 확인
4. **캐시 효과**: 동일 Cargo.lock으로 재실행하여 캐시 복원 확인
5. **Dependabot**: 구성 파일 배포 후 Dependabot 탭에서 활성화 확인

### 자동 검증

- 각 job의 exit code가 성공/실패를 결정한다
- GitHub Actions UI에서 job별 상태 확인 가능
- Branch protection rules 설정으로 실패 시 머지 자동 차단
