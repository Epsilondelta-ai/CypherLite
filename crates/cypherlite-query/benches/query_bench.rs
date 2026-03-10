// Criterion benchmarks for cypherlite-query: lexer, parser, and full execution pipeline.

use criterion::{criterion_group, criterion_main, Criterion};
use cypherlite_core::{DatabaseConfig, SyncMode};
use cypherlite_query::lexer::lex;
use cypherlite_query::parser::parse_query;
use cypherlite_query::CypherLite;
use tempfile::tempdir;

// ---------------------------------------------------------------------------
// Lexer benchmarks
// ---------------------------------------------------------------------------

const SIMPLE_QUERY: &str = "MATCH (n:Person) WHERE n.age > 30 RETURN n.name";

const COMPLEX_QUERY: &str = "\
    MATCH (a:Person)-[:KNOWS]->(b:Person)-[:LIVES_IN]->(c:City) \
    WHERE a.age > 25 AND b.name = 'Alice' OR c.population >= 1000000 \
    RETURN a.name, b.name, c.name, a.age \
    ORDER BY a.age DESC \
    LIMIT 10";

fn bench_lex_simple(c: &mut Criterion) {
    c.bench_function("lex_simple", |b| {
        b.iter(|| {
            let _ = lex(SIMPLE_QUERY).expect("lex");
        });
    });
}

fn bench_lex_complex(c: &mut Criterion) {
    c.bench_function("lex_complex", |b| {
        b.iter(|| {
            let _ = lex(COMPLEX_QUERY).expect("lex");
        });
    });
}

// ---------------------------------------------------------------------------
// Parser benchmarks
// ---------------------------------------------------------------------------

fn bench_parse_simple(c: &mut Criterion) {
    c.bench_function("parse_simple", |b| {
        b.iter(|| {
            let _ = parse_query(SIMPLE_QUERY).expect("parse");
        });
    });
}

fn bench_parse_complex(c: &mut Criterion) {
    c.bench_function("parse_complex", |b| {
        b.iter(|| {
            let _ = parse_query(COMPLEX_QUERY).expect("parse");
        });
    });
}

// ---------------------------------------------------------------------------
// Execution pipeline benchmarks
// ---------------------------------------------------------------------------

fn test_config(dir: &std::path::Path) -> DatabaseConfig {
    DatabaseConfig {
        path: dir.join("bench.cyl"),
        wal_sync_mode: SyncMode::Normal,
        ..Default::default()
    }
}

fn bench_execute_create(c: &mut Criterion) {
    c.bench_function("execute_create", |b| {
        b.iter(|| {
            let dir = tempdir().expect("tempdir");
            let mut db = CypherLite::open(test_config(dir.path())).expect("open");
            db.execute("CREATE (n:Person {name: 'Alice', age: 30})")
                .expect("create");
        });
    });
}

fn bench_execute_match(c: &mut Criterion) {
    // Pre-populate a database with 100 nodes, then benchmark MATCH-RETURN.
    let dir = tempdir().expect("tempdir");
    let mut db = CypherLite::open(test_config(dir.path())).expect("open");
    for i in 0..100 {
        db.execute(&format!(
            "CREATE (n:Person {{name: 'Person{i}', age: {i}}})"
        ))
        .expect("create");
    }

    c.bench_function("execute_match_100", |b| {
        b.iter(|| {
            let result = db.execute("MATCH (n:Person) RETURN n.name").expect("match");
            assert_eq!(result.rows.len(), 100);
        });
    });
}

fn bench_execute_filter(c: &mut Criterion) {
    // Pre-populate a database with 100 nodes, then benchmark MATCH-WHERE-RETURN.
    let dir = tempdir().expect("tempdir");
    let mut db = CypherLite::open(test_config(dir.path())).expect("open");
    for i in 0..100 {
        db.execute(&format!(
            "CREATE (n:Person {{name: 'Person{i}', age: {i}}})"
        ))
        .expect("create");
    }

    c.bench_function("execute_filter_100", |b| {
        b.iter(|| {
            let result = db
                .execute("MATCH (n:Person) WHERE n.age > 50 RETURN n.name")
                .expect("filter");
            assert_eq!(result.rows.len(), 49);
        });
    });
}

// ---------------------------------------------------------------------------
// Criterion groups
// ---------------------------------------------------------------------------

criterion_group!(
    benches,
    bench_lex_simple,
    bench_lex_complex,
    bench_parse_simple,
    bench_parse_complex,
    bench_execute_create,
    bench_execute_match,
    bench_execute_filter
);
criterion_main!(benches);
