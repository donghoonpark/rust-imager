//! Verification and sidecar tests.

use std::fs;

use rust_imager_core::metadata::{ImageMetadata, VerificationStatus};
use rust_imager_core::plan::{Compression, VerificationLevel};
use rust_imager_pipeline::extract::{ExtractOptions, extract_path};
use rust_imager_pipeline::verify::{VerifyRequest, verify_image, write_sidecar};
use tempfile::tempdir;

#[test]
fn verifies_decode_and_source_reread_then_writes_sidecar() {
    let dir = tempdir().expect("tempdir");
    let source = dir.path().join("source.bin");
    let image = dir.path().join("image.img.zst");
    fs::write(&source, vec![0x42; 256 * 1024]).expect("source");
    let extracted = extract_path(
        &source,
        &image,
        &ExtractOptions {
            bytes: 128 * 1024,
            compression: Compression::Zstandard { level: 3 },
            raw_hash: true,
            buffer_bytes: 8192,
            queue_depth: 2,
        },
        |_| {},
    )
    .expect("extract");
    let result = verify_image(&VerifyRequest {
        image: image.clone(),
        compression: Compression::Zstandard { level: 3 },
        level: VerificationLevel::SourceReread,
        expected_raw_bytes: extracted.raw_bytes,
        expected_raw_sha256: extracted.raw_sha256.clone(),
        source: Some(source),
    })
    .expect("verify");
    assert_eq!(result.status, VerificationStatus::Passed);

    let metadata = ImageMetadata::new(
        "/dev/sda".into(),
        extracted.raw_bytes,
        extracted.compressed_bytes,
        Compression::Zstandard { level: 3 },
        result,
        extracted.compressed_sha256,
    );
    let sidecar = write_sidecar(&image, &metadata).expect("sidecar");
    let decoded: ImageMetadata =
        serde_json::from_slice(&fs::read(sidecar).expect("read")).expect("json");
    assert_eq!(decoded.schema_version, 1);
}

#[test]
fn corrupted_stream_fails_decode_verification() {
    let dir = tempdir().expect("tempdir");
    let image = dir.path().join("bad.img.xz");
    fs::write(&image, b"not xz").expect("bad image");
    assert!(
        verify_image(&VerifyRequest {
            image,
            compression: Compression::Xz { level: 3 },
            level: VerificationLevel::Decode,
            expected_raw_bytes: 10,
            expected_raw_sha256: None,
            source: None,
        })
        .is_err()
    );
}
