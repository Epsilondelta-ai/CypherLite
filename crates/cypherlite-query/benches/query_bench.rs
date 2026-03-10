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
// Phase 3 benchmarks: index scan, variable-length paths, MERGE
// ---------------------------------------------------------------------------

fn bench_index_scan_vs_full_scan(c: &mut Criterion) {
    // Pre-populate with 500 nodes, then compare indexed vs non-indexed property lookup.
    let dir = tempdir().expect("tempdir");
    let mut db = CypherLite::open(test_config(dir.path())).expect("open");
    for i in 0..500 {
        db.execute(&format!(
            "CREATE (n:Person {{name: 'Person{i}', age: {i}}})"
        ))
        .expect("create");
    }

    // Benchmark WITHOUT index (full scan + filter)
    c.bench_function("full_scan_500", |b| {
        b.iter(|| {
            let result = db
                .execute("MATCH (n:Person) WHERE n.age = 250 RETURN n.name")
                .expect("full scan");
            assert_eq!(result.rows.len(), 1);
        });
    });

    // Create index, then benchmark WITH index
    db.execute("CREATE INDEX idx_person_age ON :Person(age)")
        .expect("create index");

    c.bench_function("index_scan_500", |b| {
        b.iter(|| {
            let result = db
                .execute("MATCH (n:Person) WHERE n.age = 250 RETURN n.name")
                .expect("index scan");
            assert_eq!(result.rows.len(), 1);
        });
    });
}

fn bench_var_length_path(c: &mut Criterion) {
    // Build a chain of 20 nodes: n0 -> n1 -> ... -> n19
    let dir = tempdir().expect("tempdir");
    let mut db = CypherLite::open(test_config(dir.path())).expect("open");

    db.execute("CREATE (n:Node {idx: 0})").expect("create first");
    for i in 1..20 {
        db.execute(&format!(
            "MATCH (a:Node {{idx: {}}}) CREATE (a)-[:NEXT]->(b:Node {{idx: {}}})",
            i - 1,
            i
        ))
        .expect("create chain");
    }

    c.bench_function("var_path_1_5", |b| {
        b.iter(|| {
            let result = db
                .execute("MATCH (a:Node {idx: 0})-[:NEXT*1..5]->(b) RETURN b")
                .expect("var path");
            assert!(result.rows.len() >= 5);
        });
    });

    c.bench_function("var_path_1_10", |b| {
        b.iter(|| {
            let result = db
                .execute("MATCH (a:Node {idx: 0})-[:NEXT*1..10]->(b) RETURN b")
                .expect("var path");
            assert!(result.rows.len() >= 10);
        });
    });
}

fn bench_merge_vs_create(c: &mut Criterion) {
    // Compare MERGE (match-or-create) vs plain CREATE
    c.bench_function("merge_new_node", |b| {
        let dir = tempdir().expect("tempdir");
        let mut db = CypherLite::open(test_config(dir.path())).expect("open");
        let mut i = 0;
        b.iter(|| {
            db.execute(&format!("MERGE (n:Person {{name: 'Person{i}'}})",))
                .expect("merge");
            i += 1;
        });
    });

    c.bench_function("create_node", |b| {
        let dir = tempdir().expect("tempdir");
        let mut db = CypherLite::open(test_config(dir.path())).expect("open");
        let mut i = 0;
        b.iter(|| {
            db.execute(&format!("CREATE (n:Person {{name: 'Person{i}'}})",))
                .expect("create");
            i += 1;
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
    bench_execute_filter,
    bench_index_scan_vs_full_scan,
    bench_var_length_path,
    bench_merge_vs_create
);
criterion_main!(benches);
