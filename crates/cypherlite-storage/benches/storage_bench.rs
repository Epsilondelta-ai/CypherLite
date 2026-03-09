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

criterion_group!(
    benches,
    bench_node_write,
    bench_node_read,
    bench_wal_write_commit,
    bench_checkpoint
);
criterion_main!(benches);
