//! Relative raw-copy and compression throughput benchmark.

use std::fs;

use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use rust_imager_core::plan::Compression;
use rust_imager_pipeline::extract::{ExtractOptions, extract_path};
use tempfile::tempdir;

const BYTES: usize = 64 * 1024 * 1024;

fn throughput(c: &mut Criterion) {
    let dir = tempdir().expect("temporary benchmark directory");
    let source = dir.path().join("source.bin");
    let data: Vec<u8> = (0..BYTES)
        .map(|index| u8::try_from(index % 251).expect("bounded"))
        .collect();
    fs::write(&source, data).expect("benchmark source");

    let mut group = c.benchmark_group("sequential-imaging");
    group.throughput(Throughput::Bytes(u64::try_from(BYTES).expect("bounded")));
    group.bench_function("raw-copy", |bencher| {
        let output = dir.path().join("raw.img");
        bencher.iter(|| fs::copy(&source, &output).expect("raw copy"));
    });
    group.bench_function("zstd-3", |bencher| {
        let output = dir.path().join("image.img.zst");
        bencher.iter(|| {
            if output.exists() {
                fs::remove_file(&output).expect("remove previous benchmark output");
            }
            extract_path(
                &source,
                &output,
                &ExtractOptions {
                    bytes: u64::try_from(BYTES).expect("bounded"),
                    compression: Compression::Zstandard { level: 3 },
                    raw_hash: false,
                    buffer_bytes: 1024 * 1024,
                    queue_depth: 4,
                },
                |_| {},
            )
            .expect("zstd extraction");
        });
    });
    group.finish();
}

criterion_group!(benches, throughput);
criterion_main!(benches);
