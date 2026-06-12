//! XZ extraction tests.

use std::fs;
use std::io::Read;

use rust_imager_core::plan::Compression;
use rust_imager_pipeline::extract::{ExtractOptions, extract_path};
use tempfile::tempdir;
use xz2::read::XzDecoder;

#[test]
fn xz_round_trip_preserves_exact_range() {
    let dir = tempdir().expect("tempdir");
    let source = dir.path().join("source.bin");
    let output = dir.path().join("image.img.xz");
    let data = vec![0x5a; 256 * 1024];
    fs::write(&source, &data).expect("source");
    extract_path(
        &source,
        &output,
        &ExtractOptions {
            bytes: 128 * 1024,
            compression: Compression::Xz { level: 3 },
            raw_hash: true,
            buffer_bytes: 16 * 1024,
            queue_depth: 2,
        },
        |_| {},
    )
    .expect("extract xz");
    let mut restored = Vec::new();
    XzDecoder::new(fs::File::open(output).expect("output"))
        .read_to_end(&mut restored)
        .expect("decode");
    assert_eq!(restored, data[..128 * 1024]);
}
