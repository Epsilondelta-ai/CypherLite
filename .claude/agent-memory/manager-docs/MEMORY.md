# manager-docs Agent Memory — CypherLite

## Project Context

- **Project**: CypherLite — embedded single-file graph database in Rust
- **Workspace**: `cypherlite-core` + `cypherlite-storage` crates
- **MSRV**: Rust 1.84+ (uses `Option::is_none_or`)
- **File extensions**: `.cyl` (data), `.cyl-wal` (WAL)
- **Magic number**: `CYLT` (0x43594C54)

## Key Patterns

### Clippy approx_constant Warning

Rust clippy denies `3.14`, `2.718`, `2.71828`, `3.14159` as approximate constants for PI/E.
Use exact representations like `1.5_f64`, `2.5_f64` in tests that just need a Float64 value.
Affected files in SPEC-DB-001:
- `crates/cypherlite-storage/src/btree/property_store.rs`
- `crates/cypherlite-core/src/types.rs`
- `crates/cypherlite-storage/tests/crud_operations.rs`

### Dependency Versions (actual, not spec)

| Crate | Actual | Notes |
|-------|--------|-------|
| dashmap | 6 | Upgraded from spec v5 during strategy review |
| thiserror | 2 | Upgraded from spec v1 |
| parking_lot | 0.12 | As specced |

### SPEC-DB-001 Status

- Status: **completed** (sync done 2026-03-10)
- Coverage: 96.82% (target 85%)
- Tests: 207 passing

### GitHub Remote

- Remote was NOT configured at project init
- GitHub repo may not yet exist at `Epsilondelta-ai/CypherLite`
- `gh` CLI requires `gh auth login` before PR creation
- When GitHub is set up, remote is: `https://github.com/Epsilondelta-ai/CypherLite.git`
