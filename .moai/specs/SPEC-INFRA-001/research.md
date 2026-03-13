---
id: SPEC-INFRA-001
type: research
created: "2026-03-13"
author: epsilondelta
---

# SPEC-INFRA-001 Research: CI/CD Pipeline

## 1. Workspace Structure

- 3 crates: cypherlite-core, cypherlite-storage, cypherlite-query (all v0.8.0)
- Edition: 2021, MSRV: Rust 1.84+
- Feature chain: temporal-core -> temporal-edge -> subgraph -> hypergraph -> full-temporal
- Default feature: temporal-core

## 2. Test Infrastructure

- Total: 1,256 tests (--all-features), 306 (default features)
- Framework: std test + proptest (property-based)
- Integration tests: tests/ directories in storage and query crates
- Proptest: configurable case counts (10K for fast, 1K for slow)
- Temp files: tempfile = "3" for isolated test DBs

## 3. Benchmark Infrastructure

- Framework: Criterion 0.5 with html_reports
- 6 benchmark targets:
  - storage: storage_bench (harness = false)
  - query: query_bench, temporal_edge, subgraph (requires-features), hypergraph (requires-features), inline_filter

## 4. Code Quality Current State

- Clippy: No clippy.toml, uses `cargo clippy --workspace --all-targets -- -D warnings`
- Rustfmt: No rustfmt.toml, default formatting
- Coverage: cargo-llvm-cov, current 93%+, target 85%
- No cargo-audit setup

## 5. CI/CD Current State

- No .github/workflows/ directory
- No GitHub Actions configured
- No Dependabot configuration
- No security scanning

## 6. Dependencies

Production: thiserror 2, serde 1, bincode 1, parking_lot 0.12, crossbeam 0.8, dashmap 6, logos 0.14
Dev: proptest 1, criterion 0.5, tempfile 3

## 7. Key Commands

```bash
cargo test --workspace --all-features          # All tests
cargo clippy --workspace --all-targets -- -D warnings  # Linting
cargo fmt --all -- --check                     # Format check
cargo llvm-cov --workspace --all-features --summary-only  # Coverage
cargo bench --workspace --no-run               # Bench compile check
cargo audit                                    # Security audit
```

## 8. Constraints

- cargo-llvm-cov requires llvm-tools-preview rustup component
- Some benchmarks require feature flags (subgraph, hypergraph)
- Proptest slow tests can take 15+ seconds
- No async/await (synchronous I/O)
