//! Privileged validation against pinned, real SBC images.

use std::env;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

use rust_imager_core::plan::{Compression, PlanInput, VerificationLevel, build_plan};
use rust_imager_core::profile::{LayoutClass, Profile, classify_layout};
use rust_imager_linux::command::{ProcessRunner, Runner};
use rust_imager_linux::inspect::{
    ext4_geometry, inspect_filesystems, partition_path, read_mbr_from_disk,
};
use rust_imager_linux::shrink::{LinuxShrinkBackend, ShrinkRequest, execute_shrink};
use rust_imager_pipeline::extract::{ExtractOptions, extract_path};
use rust_imager_pipeline::verify::{VerifyRequest, verify_image};

#[test]
#[ignore = "requires root, Linux loop devices, and a pinned SBC image"]
fn shrinks_extracts_and_verifies_real_sbc_image() -> Result<(), Box<dyn std::error::Error>> {
    let disk = required_env("RUST_IMAGER_REAL_DISK")?;
    let family = required_env("RUST_IMAGER_REAL_FAMILY")?;
    let output = PathBuf::from(required_env("RUST_IMAGER_REAL_OUTPUT")?);
    std::fs::create_dir_all(&output)?;

    let size = required_env("RUST_IMAGER_REAL_SIZE")?.parse::<u64>()?;
    let sectors = size / 512;
    let mbr = read_mbr_from_disk(Path::new(&disk), sectors)?;
    let runner = ProcessRunner;
    let inspect_mount = output.join("inspect");
    let evidence = inspect_filesystems(&runner, &disk, &mbr, &inspect_mount)?;
    let layout = classify_layout(&mbr, &evidence);
    match family.as_str() {
        "raspberry-pi" => assert_eq!(layout, LayoutClass::Recognized(Profile::RaspberryPi)),
        "odroid" => assert!(matches!(
            layout,
            LayoutClass::Recognized(Profile::Odroid) | LayoutClass::WarningUnknown
        )),
        other => panic!("unsupported fixture family: {other}"),
    }

    let root = mbr
        .partitions
        .iter()
        .max_by_key(|partition| partition.end_lba)
        .ok_or("real image has no root partition")?;
    let root_path = partition_path(&disk, root.number);
    let ext4 = ext4_geometry(&runner, &root_path)?;
    let plan = build_plan(PlanInput {
        device_path: disk.clone(),
        device_size_bytes: size,
        sector_size: 512,
        root_start_lba: root.start_lba,
        ext4_minimum_bytes: ext4.minimum_bytes,
        ext4_current_bytes: ext4.current_bytes,
        output_path: output.display().to_string(),
        output_available_bytes: u64::MAX,
        output_is_local: true,
        compression: Compression::Zstandard { level: 1 },
        verification: VerificationLevel::Decode,
        alignment_bytes: 1024 * 1024,
    })?;
    execute_shrink(
        &LinuxShrinkBackend::new(&runner, output.join("root")),
        &ShrinkRequest {
            disk: disk.clone(),
            root_partition: root_path.clone(),
            partition_number: root.number,
            root_start_lba: root.start_lba,
            root_end_lba: plan.shrink.root_end_lba,
            target_filesystem_kib: plan.shrink.target_filesystem_bytes / 1024,
            device_size_bytes: size,
            logical_sector_size: 512,
            original_mbr: mbr.clone(),
        },
    )?;

    let changed = read_mbr_from_disk(Path::new(&disk), sectors)?;
    let changed_root = changed
        .partitions
        .iter()
        .find(|partition| partition.number == root.number)
        .ok_or("root partition disappeared")?;
    assert_eq!(changed_root.end_lba, plan.shrink.root_end_lba);
    runner.run(&rust_imager_linux::command::CommandSpec::new(
        "e2fsck",
        [OsString::from("-fn"), OsString::from(&root_path)],
    ))?;

    for compression in [
        Compression::Zstandard { level: 1 },
        Compression::Xz { level: 1 },
    ] {
        let name = match compression {
            Compression::Zstandard { .. } => "real.img.zst",
            Compression::Xz { .. } => "real.img.xz",
        };
        let image = output.join(name);
        let extracted = extract_path(
            Path::new(&disk),
            &image,
            &ExtractOptions {
                bytes: plan.image_bytes,
                compression,
                raw_hash: true,
                buffer_bytes: 1024 * 1024,
                queue_depth: 4,
            },
            |_| {},
        )?;
        verify_image(&VerifyRequest {
            image,
            compression,
            level: VerificationLevel::SourceReread,
            expected_raw_bytes: plan.image_bytes,
            expected_raw_sha256: extracted.raw_sha256,
            source: Some(PathBuf::from(&disk)),
        })?;
    }
    Ok(())
}

fn required_env(name: &str) -> Result<String, Box<dyn std::error::Error>> {
    env::var(name).map_err(|_| format!("{name} is required").into())
}
