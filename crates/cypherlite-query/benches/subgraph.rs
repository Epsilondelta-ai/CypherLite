// Criterion benchmarks for subgraph operations (KK-004).
//
// Benchmarks:
// 1. Subgraph creation with 100 and 1000 members
// 2. Membership lookup (forward: list members, reverse: via query)
// 3. SubgraphScan with property filter

use criterion::{criterion_group, criterion_main, Criterion};
use cypherlite_core::{DatabaseConfig, SyncMode};
use cypherlite_query::CypherLite;
use tempfile::tempdir;

fn setup_db_with_nodes(n: usize) -> (tempfile::TempDir, CypherLite) {
    let dir = tempdir().expect("tmpdir");
    let config = DatabaseConfig {
        path: dir.path().join("bench.cyl"),
        wal_sync_mode: SyncMode::Normal,
        ..Default::default()
    };
    let mut db = CypherLite::open(config).expect("open");

    // Create N nodes
    for i in 0..n {
        let q = format!("CREATE (x:BenchSG {{idx: {}}})", i);
        db.execute(&q).expect("create node");
    }

    (dir, db)
}

// Benchmark: Create a snapshot of 100 members
fn bench_create_snapshot_100(c: &mut Criterion) {
    c.bench_function("subgraph_create_snapshot_100", |b| {
        b.iter_with_setup(
            || {
                let dir = tempdir().expect("tmpdir");
                let config = DatabaseConfig {
                    path: dir.path().join("bench.cyl"),
                    wal_sync_mode: SyncMode::Normal,
                    ..Default::default()
                };
                let mut db = CypherLite::open(config).expect("open");
                for i in 0..100 {
                    let q = format!("CREATE (x:Snap100 {{idx: {}}})", i);
                    db.execute(&q).expect("create");
                }
                (dir, db)
            },
            |(_dir, mut db)| {
                db.execute(
                    "CREATE SNAPSHOT (sg:Snap {name: 'bench100'}) FROM MATCH (x:Snap100) RETURN x",
                )
                .expect("snapshot");
            },
        );
    });
}

// Benchmark: Create a snapshot of 1000 members
fn bench_create_snapshot_1000(c: &mut Criterion) {
    c.bench_function("subgraph_create_snapshot_1000", |b| {
        b.iter_with_setup(
            || {
                let dir = tempdir().expect("tmpdir");
                let config = DatabaseConfig {
                    path: dir.path().join("bench.cyl"),
                    wal_sync_mode: SyncMode::Normal,
                    ..Default::default()
                };
                let mut db = CypherLite::open(config).expect("open");
                for i in 0..1000 {
                    let q = format!("CREATE (x:Snap1K {{idx: {}}})", i);
                    db.execute(&q).expect("create");
                }
                (dir, db)
            },
            |(_dir, mut db)| {
                db.execute(
                    "CREATE SNAPSHOT (sg:Snap {name: 'bench1k'}) FROM MATCH (x:Snap1K) RETURN x",
                )
                .expect("snapshot");
            },
        );
    });
}

// Benchmark: Forward membership lookup (list members of a subgraph)
fn bench_membership_lookup_forward(c: &mut Criterion) {
    let (dir, mut db) = setup_db_with_nodes(100);

    // Create snapshot
    db.execute("CREATE SNAPSHOT (sg:Snap {name: 'lookup-fwd'}) FROM MATCH (x:BenchSG) RETURN x")
        .expect("snapshot");

    c.bench_function("subgraph_membership_lookup_forward_100", |b| {
        b.iter(|| {
            let _ = db
                .execute("MATCH (sg:Subgraph {name: 'lookup-fwd'})-[:CONTAINS]->(x) RETURN x.idx")
                .expect("query");
        });
    });

    drop(dir);
}

// Benchmark: SubgraphScan with property filter
fn bench_subgraph_scan_with_filter(c: &mut Criterion) {
    let dir = tempdir().expect("tmpdir");
    let config = DatabaseConfig {
        path: dir.path().join("bench.cyl"),
        wal_sync_mode: SyncMode::Normal,
        ..Default::default()
    };
    let mut db = CypherLite::open(config).expect("open");

    // Create nodes and multiple subgraphs
    for i in 0..50 {
        let q = format!("CREATE (x:ScanNode {{idx: {}}})", i);
        db.execute(&q).expect("create");
    }

    for i in 0..10 {
        let q = format!(
            "CREATE SNAPSHOT (sg:Snap {{name: 'scan-{}'}}) AT TIME {} FROM MATCH (x:ScanNode) RETURN x",
            i,
            1_000_000_000_000i64 + i * 100_000_000_000i64
        );
        db.execute(&q).expect("snapshot");
    }

    c.bench_function("subgraph_scan_with_property_filter", |b| {
        b.iter(|| {
            let _ = db
                .execute("MATCH (sg:Subgraph) WHERE sg.name = 'scan-5' RETURN sg.name")
                .expect("query");
        });
    });

    drop(dir);
}

// Benchmark: SubgraphScan listing all subgraphs (no filter)
fn bench_subgraph_scan_all(c: &mut Criterion) {
    let dir = tempdir().expect("tmpdir");
    let config = DatabaseConfig {
        path: dir.path().join("bench.cyl"),
        wal_sync_mode: SyncMode::Normal,
        ..Default::default()
    };
    let mut db = CypherLite::open(config).expect("open");

    // Create a node and 10 subgraphs
    db.execute("CREATE (x:AllScanNode {idx: 0})")
        .expect("create");
    for i in 0..10 {
        let q = format!(
            "CREATE SNAPSHOT (sg:Snap {{name: 'all-{}'}}) FROM MATCH (x:AllScanNode) RETURN x",
            i
        );
        db.execute(&q).expect("snapshot");
    }

    c.bench_function("subgraph_scan_all_10", |b| {
        b.iter(|| {
            let _ = db
                .execute("MATCH (sg:Subgraph) RETURN sg.name")
                .expect("query");
        });
    });

    drop(dir);
}

criterion_group!(
    subgraph_benches,
    bench_create_snapshot_100,
    bench_create_snapshot_1000,
    bench_membership_lookup_forward,
    bench_subgraph_scan_with_filter,
    bench_subgraph_scan_all,
);
criterion_main!(subgraph_benches);
