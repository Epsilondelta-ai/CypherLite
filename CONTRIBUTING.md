# Contributing to CypherLite

Thank you for your interest in contributing to CypherLite. This guide covers everything you need to get started.

## Reporting Bugs

Please use [GitHub Issues](https://github.com/Epsilondelta-ai/CypherLite/issues) to report bugs. Include:

- A clear description of the problem
- Steps to reproduce
- Expected vs. actual behavior
- Your Rust version (`rustc --version`) and OS

## Submitting Changes

1. **Fork** the repository and create a branch from `main`
2. **Name your branch** descriptively: `feat/my-feature` or `fix/issue-123`
3. **Make your changes** following the guidelines below
4. **Open a Pull Request** against `main` with a clear description

For significant changes, open an issue first to discuss the approach.

## Development Setup

**Requirements:**

- Rust 1.84 or later (MSRV)
- The standard Rust toolchain via [rustup](https://rustup.rs/)

**Setup:**

```bash
git clone https://github.com/Epsilondelta-ai/CypherLite.git
cd CypherLite
cargo build --workspace
cargo test --workspace --all-features
```

**Feature flags:**

| Flag | Description |
|------|-------------|
| `temporal-core` | Temporal graph support (default) |
| `temporal-edge` | Temporal edge attributes |
| `subgraph` | Subgraph extraction |
| `hypergraph` | Hypergraph support (requires `subgraph`) |
| `full-temporal` | All temporal features |
| `plugin` | Plugin system |

## Code Style

Run these before submitting:

```bash
cargo fmt --all
cargo clippy --workspace --all-features -- -D warnings
```

Clippy warnings are treated as errors in CI. Pay attention to:

- Avoid approximate float constants (use exact values like `1.5_f64` instead of `3.14`)
- Follow standard Rust naming conventions
- Write English-language code comments

## Testing

All changes must include tests.

```bash
# Run all tests
cargo test --workspace --all-features

# Run with coverage (requires cargo-tarpaulin)
cargo tarpaulin --workspace --all-features

# Run with thread sanitizer for concurrency safety
RUSTFLAGS="-Z sanitizer=thread" cargo test --workspace
```

**Requirements:**

- Maintain 85%+ test coverage across the workspace
- Write tests before implementation (TDD)
- Tests should validate behavior, not implementation details
- Use `cargo test --workspace --all-features` to confirm all feature combinations pass

## Commit Messages

Use [Conventional Commits](https://www.conventionalcommits.org/):

```
<type>(<scope>): <short description>

[optional body]
```

Types: `feat`, `fix`, `docs`, `test`, `refactor`, `perf`, `chore`, `ci`

Examples:
- `feat(storage): add B-tree cursor iteration`
- `fix(core): handle empty graph edge case`
- `docs(readme): add installation instructions`

## License

By contributing, you agree that your contributions will be licensed under the same dual license as the project:

**MIT OR Apache-2.0**

See [LICENSE-MIT](LICENSE-MIT) and [LICENSE-APACHE](LICENSE-APACHE) for details.
