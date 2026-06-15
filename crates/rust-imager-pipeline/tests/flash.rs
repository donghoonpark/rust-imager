//! Flash image inspection tests.

use std::fs;

use rust_imager_core::flash::{ImageFormat, PostVerify, PreVerify};
use rust_imager_pipeline::flash::{FlashRequest, InspectRequest, flash_image, inspect_image};
use tempfile::tempdir;

#[test]
fn inspects_raw_image_without_sidecar() {
    let dir = tempdir().expect("tempdir");
    let image = dir.path().join("board.img");
    let mut bytes = vec![0_u8; 1024 * 1024];
    bytes[510] = 0x55;
    bytes[511] = 0xaa;
    bytes[446 + 4] = 0x83;
    bytes[446 + 8..446 + 12].copy_from_slice(&1_u32.to_le_bytes());
    bytes[446 + 12..446 + 16].copy_from_slice(&128_u32.to_le_bytes());
    fs::write(&image, &bytes).expect("image");

    let inspected = inspect_image(&InspectRequest {
        image,
        format: ImageFormat::Raw,
        level: PreVerify::Basic,
    })
    .expect("inspect");

    assert_eq!(inspected.raw_bytes, bytes.len() as u64);
    assert!(!inspected.sidecar_present);
    assert!(inspected.raw_sha256.is_some());
}

#[test]
fn rejects_corrupt_compressed_image() {
    let dir = tempdir().expect("tempdir");
    let image = dir.path().join("board.img.zst");
    fs::write(&image, b"not zstd").expect("image");

    assert!(
        inspect_image(&InspectRequest {
            image,
            format: ImageFormat::Zstandard,
            level: PreVerify::Basic,
        })
        .is_err()
    );
}

#[test]
fn full_verification_rejects_mismatched_sidecar_hash() {
    let dir = tempdir().expect("tempdir");
    let image = dir.path().join("board.img");
    let mut bytes = vec![0_u8; 4096];
    bytes[510] = 0x55;
    bytes[511] = 0xaa;
    fs::write(&image, &bytes).expect("image");
    fs::write(
        image.with_extension("img.json"),
        format!(
            r#"{{
  "schema_version": 1,
  "tool_version": "0.2.7",
  "completed_unix_seconds": 0,
  "source_device": "/dev/sdz",
  "source_size_bytes": 4096,
  "sector_size": 512,
  "partitions": [],
  "raw_bytes": 4096,
  "compressed_bytes": 4096,
  "compression": {{"Zstandard": {{"level": 3}}}},
  "compressed_sha256": "{}",
  "verification": {{
    "level": "decode",
    "status": "passed",
    "raw_sha256": null,
    "raw_bytes": 4096
  }},
  "first_boot_version": 1
}}"#,
            "00".repeat(32)
        ),
    )
    .expect("sidecar");

    assert!(
        inspect_image(&InspectRequest {
            image,
            format: ImageFormat::Raw,
            level: PreVerify::Full,
        })
        .is_err()
    );
}

#[test]
fn streams_exact_image_and_rereads_target() {
    let dir = tempdir().expect("tempdir");
    let image = dir.path().join("board.img");
    let target = dir.path().join("target.bin");
    let bytes = (0..512 * 1024)
        .map(|value| (value % 251) as u8)
        .collect::<Vec<_>>();
    fs::write(&image, &bytes).expect("image");
    fs::write(&target, vec![0xaa; bytes.len() * 2]).expect("target");
    let mut updates = Vec::new();

    let result = flash_image(
        &FlashRequest {
            image,
            format: ImageFormat::Raw,
            target: target.clone(),
            target_capacity: (bytes.len() * 2) as u64,
            expected_raw_bytes: bytes.len() as u64,
            expected_raw_sha256: None,
            post_verify: PostVerify::Full,
        },
        |written, total| updates.push((written, total)),
    )
    .expect("flash");

    assert_eq!(result.bytes_written, bytes.len() as u64);
    assert!(result.post_verified);
    assert_eq!(&fs::read(target).expect("read")[..bytes.len()], bytes);
    assert_eq!(updates.last(), Some(&(bytes.len() as u64, bytes.len() as u64)));
}

#[test]
fn rejects_target_smaller_than_inspected_image() {
    let dir = tempdir().expect("tempdir");
    let image = dir.path().join("board.img");
    let target = dir.path().join("target.bin");
    fs::write(&image, vec![0x42; 4096]).expect("image");
    fs::write(&target, vec![0; 2048]).expect("target");

    assert!(
        flash_image(
            &FlashRequest {
                image,
                format: ImageFormat::Raw,
                target,
                target_capacity: 2048,
                expected_raw_bytes: 4096,
                expected_raw_sha256: None,
                post_verify: PostVerify::None,
            },
            |_, _| {},
        )
        .is_err()
    );
}
