// Criterion benchmarks for temporal edge filtering (Group FF-T3).

use criterion::{criterion_group, criterion_main, Criterion};
use cypherlite_core::{DatabaseConfig, SyncMode};
use cypherlite_query::executor::{Params, Value};
use cypherlite_query::CypherLite;
use tempfile::tempdir;

fn setup_temporal_db() -> (tempfile::TempDir, CypherLite) {
    let dir = tempdir().expect("tmpdir");
    let config = DatabaseConfig {
        path: dir.path().join("bench.cyl"),
        wal_sync_mode: SyncMode::Normal,
        ..Default::default()
    };
    let mut db = CypherLite::open(config).expect("open");

    // Create a chain of 10 nodes with edges
    let mut params = Params::new();
    params.insert("__query_start_ms__".to_string(), Value::Int64(100));
    for i in 0..10 {
        let label = format!("BNode{}", i);
        let next_label = format!("BNode{}", i + 1);
        if i == 0 {
            let q = format!("CREATE (a:{} {{idx: {}}})", label, i);
            db.execute_with_params(&q, params.clone()).expect("create");
        }
        if i < 9 {
            let q = format!(
                "CREATE (a:{} {{idx: {}}})-[:BLINK]->(b:{} {{idx: {}}})",
                label, i, next_label, i + 1
            );
            db.execute_with_params(&q, params.clone()).expect("create chain");
        }
    }

    // Set validity on some edges
    for i in 0..9 {
        let label = format!("BNode{}", i);
        let next_label = format!("BNode{}", i + 1);
        let valid_from = 100 + i as i64 * 100;
        let valid_to = valid_from + 500;
        let q = format!(
            "MATCH (a:{})-[r:BLINK]->(b:{}) SET r._valid_from = {}, r._valid_to = {}",
            label, next_label, valid_from, valid_to
        );
        let _ = db.execute(&q);
    }

    (dir, db)
}

fn bench_at_time_simple(c: &mut Criterion) {
    let (dir, mut db) = setup_temporal_db();

    let mut params = Params::new();
    params.insert("__query_start_ms__".to_string(), Value::Int64(100));
    db.execute_with_params(
        "CREATE (a:BenchStart {name: 'S'})-[:BENCH]->(b:BenchEnd {name: 'E'})",
        params,
    )
    .expect("create");

    db.execute("MATCH (a:BenchStart)-[r:BENCH]->(b:BenchEnd) SET r._valid_from = 100, r._valid_to = 5000")
        .expect("set");

    c.bench_function("at_time_simple_match", |b| {
        b.iter(|| {
            let _ = db
                .execute("MATCH (a:BenchStart)-[r:BENCH]->(b:BenchEnd) AT TIME 1000 RETURN a.name")
                .expect("query");
        });
    });

    drop(dir);
}

fn bench_at_time_no_filter(c: &mut Criterion) {
    let (_dir, mut db) = setup_temporal_db();

    let mut params = Params::new();
    params.insert("__query_start_ms__".to_string(), Value::Int64(100));
    db.execute_with_params(
        "CREATE (a:BenchNoF {name: 'S'})-[:BENCHNF]->(b:BenchNoF {name: 'E'})",
        params,
    )
    .expect("create");

    c.bench_function("match_without_temporal_filter", |b| {
        b.iter(|| {
            let _ = db
                .execute("MATCH (a:BenchNoF)-[r:BENCHNF]->(b:BenchNoF) RETURN a.name")
                .expect("query");
        });
    });
}

criterion_group!(temporal_edge_benches, bench_at_time_simple, bench_at_time_no_filter);
criterion_main!(temporal_edge_benches);
