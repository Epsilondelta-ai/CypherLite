// Criterion benchmarks: memory scaling and node-load footprint.
//
// REQ-B-002: Measure performance at different node scales (1K, 10K, 100K)
// and profile memory footprint for typical workloads.

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use cypherlite_core::{DatabaseConfig, PropertyValue, SyncMode};
use cypherlite_storage::StorageEngine;
use tempfile::tempdir;

/// Benchmark node creation throughput at increasing scales.
///
/// Measures the time to create and populate a fresh database with N nodes.
/// Includes engine open, node creation, and implicit page allocation.
fn bench_memory_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_scaling");
    group.sample_size(10); // fewer samples for expensive benchmarks

    for &count in &[1_000u64, 10_000, 100_000] {
        group.bench_with_input(BenchmarkId::new("node_load", count), &count, |b, &count| {
            b.iter(|| {
                let dir = tempdir().expect("tempdir");
                let config = DatabaseConfig {
                    path: dir.path().join("bench.cyl"),
                    wal_sync_mode: SyncMode::Normal,
                    ..Default::default()
                };
                let mut engine = StorageEngine::open(config).expect("open");
                for i in 0..count {
                    engine.create_node(vec![1], vec![(1, PropertyValue::Int64(i as i64))]);
                }
            });
        });
    }
    group.finish();
}

/// Benchmark memory footprint for 10K nodes with multi-property payloads.
///
/// Each node has an Int64 and a String property to simulate realistic
/// per-node memory overhead. Measures creation + single trailing read
/// to keep the engine alive through the iteration.
fn bench_memory_footprint(c: &mut Criterion) {
    c.bench_function("memory_footprint_10k_nodes", |b| {
        b.iter(|| {
            let dir = tempdir().expect("tempdir");
            let config = DatabaseConfig {
                path: dir.path().join("bench.cyl"),
                wal_sync_mode: SyncMode::Normal,
                ..Default::default()
            };
            let mut engine = StorageEngine::open(config).expect("open");
            for i in 0..10_000u64 {
                engine.create_node(
                    vec![1],
                    vec![
                        (1, PropertyValue::Int64(i as i64)),
                        (2, PropertyValue::String(format!("node_{i}"))),
                    ],
                );
            }
            // Keep engine alive for measurement.
            let _ = engine.get_node(cypherlite_core::NodeId(1));
        });
    });
}

criterion_group!(benches, bench_memory_scaling, bench_memory_footprint);
criterion_main!(benches);
