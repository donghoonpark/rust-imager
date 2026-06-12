//! Execution planner tests.

use rust_imager_core::plan::{Compression, PlanError, PlanInput, VerificationLevel, build_plan};

fn input() -> PlanInput {
    PlanInput {
        device_path: "/dev/sda".into(),
        device_size_bytes: 64 * 1024 * 1024 * 1024,
        sector_size: 512,
        root_start_lba: 1_048_576,
        ext4_minimum_bytes: 2 * 1024 * 1024 * 1024,
        ext4_current_bytes: 16 * 1024 * 1024 * 1024,
        output_path: "/var/lib/rust-imager/backup.img.zst".into(),
        output_available_bytes: 10 * 1024 * 1024 * 1024,
        output_is_local: true,
        compression: Compression::Zstandard { level: 3 },
        verification: VerificationLevel::Decode,
        alignment_bytes: 1024 * 1024,
    }
}

#[test]
fn applies_larger_of_fixed_and_percentage_margin() {
    let plan = build_plan(input()).expect("valid plan");
    assert_eq!(plan.shrink.safety_margin_bytes, 256 * 1024 * 1024);
    assert_eq!(plan.image_bytes % (1024 * 1024), 0);
    assert!(plan.image_bytes < plan.device.size_bytes);
}

#[test]
fn rejects_remote_or_insufficient_output() {
    let mut remote = input();
    remote.output_is_local = false;
    assert_eq!(build_plan(remote), Err(PlanError::OutputNotLocal));

    let mut small = input();
    small.output_available_bytes = 1;
    assert_eq!(build_plan(small), Err(PlanError::InsufficientOutputSpace));
}

#[test]
fn rejects_invalid_geometry_without_overflow() {
    let mut invalid = input();
    invalid.root_start_lba = u64::MAX;
    assert_eq!(build_plan(invalid), Err(PlanError::GeometryOverflow));
}
