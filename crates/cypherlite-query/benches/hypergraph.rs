// Criterion benchmarks for hypergraph operations (PP-003 / NN-001).
//
// Benchmarks:
// 1. Hyperedge creation with single source
// 2. Hyperedge creation with multiple sources via chain
// 3. HyperEdgeScan listing all hyperedges
// 4. Temporal reference hyperedge creation
// 5. Multiple hyperedge creation (batch)

use criterion::{criterion_group, criterion_main, Criterion};
use cypherlite_core::{DatabaseConfig, SyncMode};
use cypherlite_query::CypherLite;
use tempfile::tempdir;

fn bench_config(dir: &std::path::Path) -> DatabaseConfig {
    DatabaseConfig {
        path: dir.join("bench.cyl"),
        wal_sync_mode: SyncMode::Normal,
        ..Default::default()
    }
}

// Benchmark: CREATE HYPEREDGE with single source node
fn bench_create_hyperedge_single_source(c: &mut Criterion) {
    c.bench_function("hyperedge_create_single_source", |b| {
        b.iter_with_setup(
            || {
                let dir = tempdir().expect("tmpdir");
                let mut db = CypherLite::open(bench_config(dir.path())).expect("open");
                db.execute("CREATE (a:HeBenchSrc {name: 'source'})")
                    .expect("create");
                (dir, db)
            },
            |(_dir, mut db)| {
                db.execute("MATCH (a:HeBenchSrc) CREATE HYPEREDGE (h:BenchHE) FROM (a) TO ()")
                    .expect("create hyperedge");
            },
        );
    });
}

// Benchmark: CREATE HYPEREDGE with two participants via relationship chain
fn bench_create_hyperedge_chain(c: &mut Criterion) {
    c.bench_function("hyperedge_create_chain", |b| {
        b.iter_with_setup(
            || {
                let dir = tempdir().expect("tmpdir");
                let mut db = CypherLite::open(bench_config(dir.path())).expect("open");
                db.execute(
                    "CREATE (a:ChainSrc {name: 'start'})-[:LINK]->(b:ChainTgt {name: 'end'})",
                )
                .expect("create chain");
                (dir, db)
            },
            |(_dir, mut db)| {
                db.execute(
                    "MATCH (a:ChainSrc)-[:LINK]->(b:ChainTgt) CREATE HYPEREDGE (h:ChainHE) FROM (a) TO (b)",
                )
                .expect("create hyperedge chain");
            },
        );
    });
}

// Benchmark: MATCH HYPEREDGE scan over N hyperedges
fn bench_hyperedge_scan(c: &mut Criterion) {
    let dir = tempdir().expect("tmpdir");
    let mut db = CypherLite::open(bench_config(dir.path())).expect("open");

    // Create 50 hyperedges
    for i in 0..50 {
        let q = format!("CREATE (x:ScanSrc{} {{idx: {}}})", i, i);
        db.execute(&q).expect("create node");
    }
    for i in 0..50 {
        let q = format!(
            "MATCH (x:ScanSrc{}) CREATE HYPEREDGE (h:ScanHE) FROM (x) TO ()",
            i
        );
        db.execute(&q).expect("create hyperedge");
    }

    c.bench_function("hyperedge_scan_50", |b| {
        b.iter(|| {
            let _ = db.execute("MATCH HYPEREDGE (h) RETURN h").expect("scan");
        });
    });

    drop(dir);
}

// Benchmark: CREATE HYPEREDGE with temporal reference
fn bench_create_hyperedge_temporal_ref(c: &mut Criterion) {
    c.bench_function("hyperedge_create_temporal_ref", |b| {
        b.iter_with_setup(
            || {
                let dir = tempdir().expect("tmpdir");
                let mut db = CypherLite::open(bench_config(dir.path())).expect("open");
                db.execute(
                    "CREATE (a:TempBSrc {name: 'src'})-[:KNOWS]->(b:TempBTgt {name: 'tgt'})",
                )
                .expect("create chain");
                (dir, db)
            },
            |(_dir, mut db)| {
                db.execute(
                    "MATCH (a:TempBSrc)-[:KNOWS]->(b:TempBTgt) CREATE HYPEREDGE (h:TempHE) FROM (a AT TIME 100) TO (b)",
                )
                .expect("temporal hyperedge");
            },
        );
    });
}

// Benchmark: Batch creation of multiple hyperedges
fn bench_create_hyperedge_batch(c: &mut Criterion) {
    c.bench_function("hyperedge_create_batch_20", |b| {
        b.iter_with_setup(
            || {
                let dir = tempdir().expect("tmpdir");
                let mut db = CypherLite::open(bench_config(dir.path())).expect("open");
                for i in 0..20 {
                    let q = format!("CREATE (x:BatchSrc{} {{idx: {}}})", i, i);
                    db.execute(&q).expect("create");
                }
                (dir, db)
            },
            |(_dir, mut db)| {
                for i in 0..20 {
                    let q = format!(
                        "MATCH (x:BatchSrc{}) CREATE HYPEREDGE (h:BatchHE) FROM (x) TO ()",
                        i
                    );
                    db.execute(&q).expect("batch create");
                }
            },
        );
    });
}

criterion_group!(
    hypergraph_benches,
    bench_create_hyperedge_single_source,
    bench_create_hyperedge_chain,
    bench_hyperedge_scan,
    bench_create_hyperedge_temporal_ref,
    bench_create_hyperedge_batch,
);
criterion_main!(hypergraph_benches);
