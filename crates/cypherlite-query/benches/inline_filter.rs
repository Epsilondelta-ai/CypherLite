// Criterion benchmarks for inline property filters (Phase 8c, SPEC-DB-008).
//
// Compares:
// 1. MATCH without inline property filter (baseline)
// 2. MATCH with single inline property filter
// 3. MATCH with WHERE clause filter (for comparison)

use criterion::{criterion_group, criterion_main, Criterion};
use cypherlite_core::{DatabaseConfig, SyncMode};
use cypherlite_query::CypherLite;
use tempfile::tempdir;

fn test_config(dir: &std::path::Path) -> DatabaseConfig {
    DatabaseConfig {
        path: dir.join("bench.cyl"),
        wal_sync_mode: SyncMode::Normal,
        ..Default::default()
    }
}

/// Pre-populate a database with N nodes having distinct name values.
fn populate_db(db: &mut CypherLite, n: usize) {
    for i in 0..n {
        db.execute(&format!(
            "CREATE (:Target {{name: 'Person{}', age: {}}})",
            i, i
        ))
        .expect("create");
    }
}

fn bench_match_no_inline_filter(c: &mut Criterion) {
    let dir = tempdir().expect("tempdir");
    let mut db = CypherLite::open(test_config(dir.path())).expect("open");
    populate_db(&mut db, 200);

    c.bench_function("match_no_inline_filter_200", |b| {
        b.iter(|| {
            let result = db
                .execute("MATCH (n:Target) RETURN n.name")
                .expect("match");
            assert_eq!(result.rows.len(), 200);
        });
    });
}

fn bench_match_with_inline_filter(c: &mut Criterion) {
    let dir = tempdir().expect("tempdir");
    let mut db = CypherLite::open(test_config(dir.path())).expect("open");
    populate_db(&mut db, 200);

    c.bench_function("match_inline_filter_200", |b| {
        b.iter(|| {
            let result = db
                .execute("MATCH (n:Target {name: 'Person100'}) RETURN n.name")
                .expect("match");
            assert_eq!(result.rows.len(), 1);
        });
    });
}

fn bench_match_with_where_filter(c: &mut Criterion) {
    let dir = tempdir().expect("tempdir");
    let mut db = CypherLite::open(test_config(dir.path())).expect("open");
    populate_db(&mut db, 200);

    c.bench_function("match_where_filter_200", |b| {
        b.iter(|| {
            let result = db
                .execute("MATCH (n:Target) WHERE n.name = 'Person100' RETURN n.name")
                .expect("match");
            assert_eq!(result.rows.len(), 1);
        });
    });
}

criterion_group!(
    benches,
    bench_match_no_inline_filter,
    bench_match_with_inline_filter,
    bench_match_with_where_filter,
);
criterion_main!(benches);
