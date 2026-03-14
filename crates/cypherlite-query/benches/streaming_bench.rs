// Criterion benchmarks: large result set streaming, 2-hop traversal, filtered queries.
//
// REQ-B-003: Measure query performance on large graphs and multi-hop patterns.

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
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

/// Create a graph with `node_count` Person nodes and `edges_per_node` KNOWS edges each.
///
/// Edge targets are deterministic pseudo-random to ensure reproducible graph topology.
fn setup_graph(node_count: u64, edges_per_node: u64) -> (tempfile::TempDir, CypherLite) {
    let dir = tempdir().expect("tempdir");
    let mut db = CypherLite::open(test_config(dir.path())).expect("open");

    // Create nodes.
    for i in 0..node_count {
        db.execute(&format!(
            "CREATE (n:Person {{name: 'person_{i}', age: {age}}})",
            age = i % 100
        ))
        .expect("create node");
    }

    // Create edges (deterministic pseudo-random targets).
    for i in 0..node_count {
        for j in 1..=edges_per_node {
            let target = (i * 7 + j * 13) % node_count + 1;
            let source = i + 1;
            db.execute(&format!(
                "MATCH (a:Person), (b:Person) \
                 WHERE id(a) = {source} AND id(b) = {target} \
                 CREATE (a)-[:KNOWS]->(b)"
            ))
            .expect("create edge");
        }
    }

    (dir, db)
}

/// Benchmark scanning all nodes with increasing graph sizes.
///
/// Measures full MATCH (n) RETURN n execution including result materialisation.
fn bench_large_result_set(c: &mut Criterion) {
    let mut group = c.benchmark_group("large_result_set");
    group.sample_size(10);

    for &count in &[1_000u64, 10_000] {
        let (_dir, mut db) = setup_graph(count, 0);
        group.bench_with_input(
            BenchmarkId::new("match_all_nodes", count),
            &count,
            |b, _| {
                b.iter(|| {
                    let result = db.execute("MATCH (n) RETURN n").expect("query");
                    assert_eq!(result.rows.len() as u64, count);
                });
            },
        );
    }
    group.finish();
}

/// Benchmark 2-hop path traversal on a medium-sized graph.
///
/// Graph: 1000 nodes, 5 edges per node (5000 total edges).
/// Query: 2-hop pattern with LIMIT to cap result explosion.
fn bench_two_hop_pattern(c: &mut Criterion) {
    let mut group = c.benchmark_group("two_hop_pattern");
    group.sample_size(10);

    let (_dir, mut db) = setup_graph(1_000, 5);

    group.bench_function("two_hop_1k_nodes_5k_edges", |b| {
        b.iter(|| {
            let _ = db
                .execute(
                    "MATCH (a:Person)-[:KNOWS]->(b:Person)-[:KNOWS]->(c:Person) \
                     RETURN a, b, c LIMIT 1000",
                )
                .expect("query");
        });
    });
    group.finish();
}

/// Benchmark filtered queries on a 10K-node graph.
///
/// Tests WHERE clause evaluation overhead with property comparison.
fn bench_filtered_query(c: &mut Criterion) {
    let mut group = c.benchmark_group("filtered_query");
    group.sample_size(10);

    let (_dir, mut db) = setup_graph(10_000, 0);

    group.bench_function("match_with_where_10k", |b| {
        b.iter(|| {
            let result = db
                .execute("MATCH (n:Person) WHERE n.age > 50 RETURN n.name")
                .expect("query");
            assert!(!result.rows.is_empty());
        });
    });
    group.finish();
}

criterion_group!(
    benches,
    bench_large_result_set,
    bench_two_hop_pattern,
    bench_filtered_query
);
criterion_main!(benches);
