# SPEC-PUBLISH-001: Multi-Ecosystem Package Publishing Pipeline

| Field | Value |
|-------|-------|
| **SPEC ID** | SPEC-PUBLISH-001 |
| **Title** | Multi-Ecosystem Package Publishing Pipeline (crates.io, PyPI, npm, Go) |
| **Created** | 2026-03-15 |
| **Status** | Planned |
| **Priority** | Critical |
| **Target Version** | v1.2.1 |

---

## 1. Problem Statement

README.md and docs-site promise users can install CypherLite via:
- `cargo add cypherlite-query` (Rust)
- `pip install cypherlite` (Python)
- `npm install cypherlite` (Node.js)
- `go get github.com/Epsilondelta-ai/CypherLite/bindings/go/cypherlite` (Go)

**Reality**: Only Go (source-only) partially works. Rust, Python, and Node.js packages are NOT published to their registries. This is a critical documentation-reality mismatch.

---

## 2. Current State Analysis

| Ecosystem | Config Ready | Published | CI/CD | Version |
|-----------|-------------|-----------|-------|---------|
| Rust (crates.io) | YES | NO | NO | 1.2.0 |
| Python (PyPI) | YES (maturin + pyproject.toml) | NO | NO | 1.2.0 |
| Node.js (npm) | YES (napi-rs + package.json) | NO | NO | 1.0.0 (MISMATCH) |
| Go (modules) | YES (go.mod) | PARTIAL (source only) | NO | N/A |

### Critical Issues
1. Node.js package.json version (1.0.0) != Cargo.toml version (1.2.0)
2. No GitHub Actions release/publish workflows
3. No pre-built wheels (Python) or native addons (Node.js)
4. Go requires Rust toolchain at build time (no pre-built static library)

---

## 3. Requirements (EARS Format)

### R-PUB-001 [Event-Driven] - Release Trigger
WHEN a git tag matching `v*` is pushed to main THEN the release pipeline MUST trigger automatically.

### R-PUB-002 [Event-Driven] - Rust crates.io Publishing
WHEN the release pipeline triggers THEN `cargo publish` MUST succeed for 4 crates in dependency order: core -> storage -> query -> ffi.

### R-PUB-003 [Event-Driven] - Python PyPI Publishing
WHEN the release pipeline triggers THEN maturin MUST build wheels for Linux (x86_64, aarch64), macOS (x86_64, arm64), Windows (x86_64) and upload to PyPI.

### R-PUB-004 [Event-Driven] - Node.js npm Publishing
WHEN the release pipeline triggers THEN napi-rs MUST build native addons for Linux (x86_64, aarch64), macOS (x86_64, arm64), Windows (x86_64) and publish to npm.

### R-PUB-005 [Event-Driven] - Go Pre-built Libraries
WHEN the release pipeline triggers THEN static libraries MUST be compiled for Linux/macOS/Windows and attached to the GitHub Release as assets.

### R-PUB-006 [Ubiquitous] - Version Sync
All package versions MUST be synchronized: Cargo.toml (all 6), pyproject.toml, package.json MUST show the same version.

### R-PUB-007 [Event-Driven] - GitHub Release
WHEN the release pipeline triggers THEN a GitHub Release MUST be created with changelog entry and pre-built binary assets.

### R-PUB-008 [Event-Driven] - Verification
WHEN packages are published THEN `pip install cypherlite`, `npm install cypherlite`, `cargo add cypherlite-query` MUST succeed from a clean environment.

### R-PUB-009 [Ubiquitous] - Node.js Version Fix
The Node.js package.json version MUST be updated from 1.0.0 to 1.2.0 to match Cargo.toml.

---

## 4. Architecture

### Release Workflow (.github/workflows/release.yml)

```
Tag push (v1.2.1) ─┬─> Job 1: Rust crates.io (sequential: core->storage->query->ffi)
                    ├─> Job 2: Python wheels (matrix: linux/macos/windows x x86_64/arm64)
                    ├─> Job 3: Node.js native addons (matrix: linux/macos/windows x x86_64/arm64)
                    ├─> Job 4: Go static libraries (matrix: linux/macos/windows x x86_64/arm64)
                    └─> Job 5: GitHub Release (waits for all above)
```

### Build Matrix (Python + Node.js + Go)

| OS | Arch | Python Wheel | Node.js Addon | Go Static Lib |
|----|------|-------------|---------------|---------------|
| Linux | x86_64 | manylinux | linux-x64 | libcypherlite.a |
| Linux | aarch64 | manylinux | linux-arm64 | libcypherlite.a |
| macOS | x86_64 | macosx | darwin-x64 | libcypherlite.a |
| macOS | arm64 | macosx | darwin-arm64 | libcypherlite.a |
| Windows | x86_64 | win_amd64 | win32-x64 | cypherlite.lib |

### Required Secrets
- `CRATES_IO_TOKEN`: crates.io API token
- `PYPI_TOKEN`: PyPI API token (or Trusted Publisher)
- `NPM_TOKEN`: npm access token

---

## 5. Implementation Plan

### TAG-001: Fix Node.js Version Mismatch
- Update `crates/cypherlite-node/package.json` version from 1.0.0 to 1.2.0
- Complexity: Small

### TAG-002: Create Release Workflow
- Create `.github/workflows/release.yml` with 5 jobs
- Trigger: `on: push: tags: ['v*']`
- Complexity: Large

### TAG-003: Rust crates.io Publishing Job
- Sequential `cargo publish -p {crate}` with delay between each
- Requires `CRATES_IO_TOKEN` secret
- Complexity: Medium

### TAG-004: Python PyPI Publishing Job
- Use `maturin` GitHub Action for cross-platform wheel building
- Use PyPI Trusted Publisher (OIDC) or `PYPI_TOKEN`
- 5 platform targets (linux x86_64/aarch64, macos x86_64/arm64, windows x86_64)
- Complexity: Medium

### TAG-005: Node.js npm Publishing Job
- Use `napi-rs/napi-rs` GitHub Action for cross-platform native addon building
- Use `NPM_TOKEN` for npm publish
- 5 platform targets
- Complexity: Medium

### TAG-006: Go Static Library Job
- Cross-compile `cypherlite-ffi` as static library for 5 targets
- Attach to GitHub Release as downloadable assets
- Complexity: Medium

### TAG-007: GitHub Release Job
- Create GitHub Release from tag
- Extract CHANGELOG.md section for release notes
- Attach all pre-built artifacts
- Complexity: Small

### TAG-008: Verification & Docs Update
- Update README/docs if any install commands changed
- Verify all packages installable from registries
- Complexity: Small

---

## 6. Traceability

| Requirement | TAG | Verification |
|-------------|-----|-------------|
| R-PUB-001 | TAG-002 | `git tag v1.2.1 && git push --tags` triggers workflow |
| R-PUB-002 | TAG-003 | `cargo install cypherlite-query` succeeds |
| R-PUB-003 | TAG-004 | `pip install cypherlite` succeeds on all platforms |
| R-PUB-004 | TAG-005 | `npm install cypherlite` succeeds on all platforms |
| R-PUB-005 | TAG-006 | GitHub Release has static lib assets |
| R-PUB-006 | TAG-001 | All version fields show same version |
| R-PUB-007 | TAG-007 | GitHub Release exists with changelog |
| R-PUB-008 | TAG-008 | Clean-env install test passes |
| R-PUB-009 | TAG-001 | package.json version matches |

## 7. Risks

| Risk | Impact | Mitigation |
|------|--------|-----------|
| crates.io name `cypherlite-*` taken | High | Check availability before publish |
| PyPI name `cypherlite` taken | High | Check availability before publish |
| npm name `cypherlite` taken | High | Check availability before publish |
| Cross-compilation failures | Medium | Use official GitHub Actions (maturin, napi-rs) |
| API token leaks | Critical | Use GitHub Secrets + environment protection rules |

## 8. Prerequisites (User Action Required)

Before running this SPEC, the user must:
1. Create crates.io account and API token -> add as `CRATES_IO_TOKEN` secret
2. Create PyPI account and configure Trusted Publisher (or API token) -> add as `PYPI_TOKEN` secret
3. Create npm account and access token -> add as `NPM_TOKEN` secret
4. Verify package names are available on all 3 registries
