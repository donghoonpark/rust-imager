//! Zstandard extraction pipeline tests.

use std::fs;
use std::io::Read;

use rust_imager_core::plan::Compression;
use rust_imager_pipeline::extract::{ExtractOptions, extract_path};
use tempfile::tempdir;

#[test]
fn extracts_exact_range_atomically_with_hashes() {
    let dir = tempdir().expect("tempdir");
    let source = dir.path().join("source.bin");
    let output = dir.path().join("image.img.zst");
    let data: Vec<u8> = (0_u32..2_000_000)
        .map(|value| value.wrapping_mul(31).to_le_bytes()[0])
        .collect();
    fs::write(&source, &data).expect("source");

    let mut progress = Vec::new();
    let result = extract_path(
        &source,
        &output,
        &ExtractOptions {
            bytes: 1_500_000,
            compression: Compression::Zstandard { level: 3 },
            raw_hash: true,
            buffer_bytes: 64 * 1024,
            queue_depth: 2,
        },
        |event| progress.push(event),
    )
    .expect("extract");

    assert!(output.exists());
    assert!(!output.with_extension("zst.partial").exists());
    assert_eq!(result.raw_bytes, 1_500_000);
    assert!(result.raw_sha256.is_some());
    assert_eq!(progress.last().expect("progress").bytes_read, 1_500_000);

    let file = fs::File::open(output).expect("compressed");
    let mut decoder = zstd::Decoder::new(file).expect("decoder");
    let mut restored = Vec::new();
    decoder.read_to_end(&mut restored).expect("decode");
    assert_eq!(restored, data[..1_500_000]);
}

#[test]
fn source_shorter_than_requested_keeps_partial_file() {
    let dir = tempdir().expect("tempdir");
    let source = dir.path().join("short.bin");
    let output = dir.path().join("image.img.zst");
    fs::write(&source, b"short").expect("source");
    assert!(
        extract_path(
            &source,
            &output,
            &ExtractOptions {
                bytes: 10,
                compression: Compression::Zstandard { level: 1 },
                raw_hash: false,
                buffer_bytes: 4,
                queue_depth: 1,
            },
            |_| {},
        )
        .is_err()
    );
    assert!(!output.exists());
    assert!(output.with_extension("zst.partial").exists());
}

#[test]
fn refuses_to_overwrite_existing_output() {
    let dir = tempdir().expect("tempdir");
    let source = dir.path().join("source.bin");
    let output = dir.path().join("image.img.zst");
    fs::write(&source, b"source").expect("source");
    fs::write(&output, b"keep").expect("output");
    assert!(
        extract_path(
            &source,
            &output,
            &ExtractOptions {
                bytes: 6,
                compression: Compression::Zstandard { level: 1 },
                raw_hash: false,
                buffer_bytes: 4,
                queue_depth: 1,
            },
            |_| {},
        )
        .is_err()
    );
    assert_eq!(fs::read(output).expect("preserved"), b"keep");
}
