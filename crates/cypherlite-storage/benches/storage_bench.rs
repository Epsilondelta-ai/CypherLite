// criterion benchmarks: node write throughput, read latency, WAL fsync, checkpoint, crash recovery

use criterion::{criterion_group, criterion_main, Criterion};
use cypherlite_core::{DatabaseConfig, PageId, PropertyValue, SyncMode};
use cypherlite_storage::page::PAGE_SIZE;
use cypherlite_storage::StorageEngine;
use tempfile::tempdir;

fn bench_node_write(c: &mut Criterion) {
    c.bench_function("node_write_100", |b| {
        b.iter(|| {
            let dir = tempdir().expect("tempdir");
            let config = DatabaseConfig {
                path: dir.path().join("bench.cyl"),
                wal_sync_mode: SyncMode::Normal,
                ..Default::default()
            };
            let mut engine = StorageEngine::open(config).expect("open");
            for i in 0..100 {
                engine.create_node(vec![1], vec![(1, PropertyValue::Int64(i))]);
            }
        });
    });
}

fn bench_node_read(c: &mut Criterion) {
    let dir = tempdir().expect("tempdir");
    let config = DatabaseConfig {
        path: dir.path().join("bench.cyl"),
        wal_sync_mode: SyncMode::Normal,
        ..Default::default()
    };
    let mut engine = StorageEngine::open(config).expect("open");
    for i in 0..1000 {
        engine.create_node(vec![1], vec![(1, PropertyValue::Int64(i))]);
    }

    c.bench_function("node_read_1000", |b| {
        b.iter(|| {
            for i in 1..=1000u64 {
                let _ = engine.get_node(cypherlite_core::NodeId(i));
            }
        });
    });
}

fn bench_wal_write_commit(c: &mut Criterion) {
    c.bench_function("wal_write_commit_10", |b| {
        b.iter(|| {
            let dir = tempdir().expect("tempdir");
            let config = DatabaseConfig {
                path: dir.path().join("bench.cyl"),
                wal_sync_mode: SyncMode::Normal,
                ..Default::default()
            };
            let mut engine = StorageEngine::open(config).expect("open");
            for i in 2..12u32 {
                engine
                    .wal_write_page(PageId(i), &[0xAB; PAGE_SIZE])
                    .expect("w");
            }
            engine.wal_commit().expect("commit");
        });
    });
}

fn bench_checkpoint(c: &mut Criterion) {
    c.bench_function("checkpoint_10_frames", |b| {
        b.iter(|| {
            let dir = tempdir().expect("tempdir");
            let config = DatabaseConfig {
                path: dir.path().join("bench.cyl"),
                wal_sync_mode: SyncMode::Normal,
                ..Default::default()
            };
            let mut engine = StorageEngine::open(config).expect("open");
            for i in 2..12u32 {
                engine
                    .wal_write_page(PageId(i), &[0xAB; PAGE_SIZE])
                    .expect("w");
            }
            engine.wal_commit().expect("commit");
            engine.checkpoint().expect("checkpoint");
        });
    });
}

fn bench_node_read_uncached(c: &mut Criterion) {
    // Create more nodes than cache_capacity (default 256) to force cache misses.
    let node_count = 500u64;
    let dir = tempdir().expect("tempdir");
    let config = DatabaseConfig {
        path: dir.path().join("bench.cyl"),
        wal_sync_mode: SyncMode::Normal,
        cache_capacity: 64, // small cache to guarantee misses
        ..Default::default()
    };
    let mut engine = StorageEngine::open(config).expect("open");
    for i in 0..node_count {
        engine.create_node(vec![1], vec![(1, PropertyValue::Int64(i as i64))]);
    }

    // Read nodes that are unlikely to be in the buffer pool (early IDs evicted by later inserts).
    c.bench_function("node_read_uncached_500", |b| {
        b.iter(|| {
            for i in 1..=100u64 {
                let _ = engine.get_node(cypherlite_core::NodeId(i));
            }
        });
    });
}

fn bench_crash_recovery(c: &mut Criterion) {
    // Measure recovery time: write N committed WAL frames, close, then reopen.
    let frame_count = 1000u32;

    c.bench_function("crash_recovery_1000_frames", |b| {
        b.iter(|| {
            let dir = tempdir().expect("tempdir");
            let db_path = dir.path().join("bench.cyl");
            let config = DatabaseConfig {
                path: db_path.clone(),
                wal_sync_mode: SyncMode::Normal,
                ..Default::default()
            };

            // Phase 1: Write WAL frames and commit (no checkpoint).
            {
                let mut engine = StorageEngine::open(config.clone()).expect("open");
                for i in 2..(2 + frame_count) {
                    engine
                        .wal_write_page(PageId(i), &[0xCD; PAGE_SIZE])
                        .expect("w");
                }
                engine.wal_commit().expect("commit");
                // Drop engine without checkpoint -- simulates crash.
            }

            // Phase 2: Reopen and measure recovery (WAL replay).
            let config2 = DatabaseConfig {
                path: db_path,
                wal_sync_mode: SyncMode::Normal,
                ..Default::default()
            };
            let _engine = StorageEngine::open(config2).expect("recovery open");
        });
    });
}

fn bench_edge_traversal(c: &mut Criterion) {
    // Create a small graph: 100 nodes, 500 edges, then benchmark adjacency traversal.
    let dir = tempdir().expect("tempdir");
    let config = DatabaseConfig {
        path: dir.path().join("bench.cyl"),
        wal_sync_mode: SyncMode::Normal,
        ..Default::default()
    };
    let mut engine = StorageEngine::open(config).expect("open");

    let node_count = 100u64;
    for i in 0..node_count {
        engine.create_node(vec![1], vec![(1, PropertyValue::Int64(i as i64))]);
    }
    // Create 500 edges: 5 outgoing edges per node to pseudo-random targets.
    for i in 0..node_count {
        for j in 1..=5u64 {
            let target = (i * 7 + j * 13) % node_count + 1; // deterministic pseudo-random
            let start = cypherlite_core::NodeId(i + 1);
            let end = cypherlite_core::NodeId(target);
            let _ = engine.create_edge(start, end, 1, vec![]);
        }
    }

    c.bench_function("edge_traversal_100n_500e", |b| {
        b.iter(|| {
            for i in 1..=node_count {
                let _ = engine.get_edges_for_node(cypherlite_core::NodeId(i));
            }
        });
    });
}

criterion_group!(
    benches,
    bench_node_write,
    bench_node_read,
    bench_wal_write_commit,
    bench_checkpoint,
    bench_node_read_uncached,
    bench_crash_recovery,
    bench_edge_traversal
);
criterion_main!(benches);
