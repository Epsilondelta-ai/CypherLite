// Criterion benchmarks: concurrent read throughput and read-write contention.
//
// REQ-B-001: Measure multi-threaded access patterns on StorageEngine
// wrapped in Arc<Mutex> for thread safety.

use criterion::{criterion_group, criterion_main, Criterion};
use cypherlite_core::{DatabaseConfig, NodeId, PropertyValue, SyncMode};
use cypherlite_storage::StorageEngine;
use parking_lot::Mutex;
use std::sync::Arc;
use std::thread;
use tempfile::tempdir;

/// Create a pre-populated StorageEngine behind Arc<Mutex> with `node_count` nodes.
fn setup_engine(node_count: u64) -> (tempfile::TempDir, Arc<Mutex<StorageEngine>>) {
    let dir = tempdir().expect("tempdir");
    let config = DatabaseConfig {
        path: dir.path().join("bench.cyl"),
        wal_sync_mode: SyncMode::Normal,
        ..Default::default()
    };
    let mut engine = StorageEngine::open(config).expect("open");
    for i in 0..node_count {
        engine.create_node(vec![1], vec![(1, PropertyValue::Int64(i as i64))]);
    }
    (dir, Arc::new(Mutex::new(engine)))
}

/// Benchmark 4 threads performing concurrent node reads.
///
/// Each thread reads a disjoint range of 2500 nodes from a 10 000-node store.
/// Measures aggregate throughput under read-only contention.
fn bench_concurrent_read_4t(c: &mut Criterion) {
    let (_dir, engine) = setup_engine(10_000);

    c.bench_function("concurrent_read_4t", |b| {
        b.iter(|| {
            let handles: Vec<_> = (0..4u64)
                .map(|t| {
                    let engine = Arc::clone(&engine);
                    thread::spawn(move || {
                        let start = t * 2500 + 1;
                        let end = start + 2500;
                        for i in start..end {
                            let eng = engine.lock();
                            let _ = eng.get_node(NodeId(i));
                        }
                    })
                })
                .collect();
            for h in handles {
                h.join().expect("thread join");
            }
        });
    });
}

/// Benchmark 3 reader threads + 1 writer thread operating concurrently.
///
/// Readers each scan 1000 nodes; the writer creates 100 new nodes.
/// Measures throughput degradation under mixed read-write contention.
fn bench_read_write_contention(c: &mut Criterion) {
    let (_dir, engine) = setup_engine(1_000);

    c.bench_function("read_write_contention_3r1w", |b| {
        b.iter(|| {
            let handles: Vec<_> = (0..4u64)
                .map(|t| {
                    let engine = Arc::clone(&engine);
                    thread::spawn(move || {
                        if t < 3 {
                            // Reader thread: scan existing nodes.
                            for i in 1..=1000u64 {
                                let eng = engine.lock();
                                let _ = eng.get_node(NodeId(i));
                            }
                        } else {
                            // Writer thread: create new nodes.
                            for _ in 0..100u64 {
                                let mut eng = engine.lock();
                                eng.create_node(vec![1], vec![(1, PropertyValue::Int64(42))]);
                            }
                        }
                    })
                })
                .collect();
            for h in handles {
                h.join().expect("thread join");
            }
        });
    });
}

criterion_group!(
    benches,
    bench_concurrent_read_4t,
    bench_read_write_contention
);
criterion_main!(benches);
