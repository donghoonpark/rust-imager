//! Guarded shrink transaction tests.

use std::sync::Mutex;

use rust_imager_core::mbr::{Mbr, Partition, PartitionKind};
use rust_imager_linux::inspect::Ext4Geometry;
use rust_imager_linux::shrink::{
    ShrinkBackend, ShrinkError, ShrinkRequest, ShrinkStage, execute_shrink, validate_post_shrink,
};

struct FakeBackend {
    calls: Mutex<Vec<ShrinkStage>>,
    fail_at: Option<ShrinkStage>,
}

impl ShrinkBackend for FakeBackend {
    fn perform(&self, stage: ShrinkStage, _request: &ShrinkRequest) -> Result<(), String> {
        self.calls.lock().expect("lock").push(stage);
        if self.fail_at == Some(stage) {
            Err("injected".into())
        } else {
            Ok(())
        }
    }
}

fn request() -> ShrinkRequest {
    ShrinkRequest {
        disk: "/dev/sda".into(),
        root_partition: "/dev/sda2".into(),
        partition_number: 2,
        root_start_lba: 4096,
        root_end_lba: 999_999,
        target_filesystem_kib: 123_456,
        device_size_bytes: 1_000_000 * 512,
        logical_sector_size: 512,
        original_mbr: mbr(999_999),
    }
}

fn partition(number: u8, start: u64, end: u64) -> Partition {
    Partition {
        number,
        type_code: if number == 1 { 0x0c } else { 0x83 },
        kind: if number == 1 {
            PartitionKind::Fat
        } else {
            PartitionKind::Linux
        },
        start_lba: start,
        sectors: end - start + 1,
        end_lba: end,
        bootable: number == 1,
    }
}

fn mbr(root_end: u64) -> Mbr {
    Mbr {
        partitions: vec![partition(1, 2048, 4095), partition(2, 4096, root_end)],
    }
}

fn geometry(bytes: u64) -> Ext4Geometry {
    Ext4Geometry {
        minimum_bytes: bytes / 2,
        current_bytes: bytes,
        block_size: 4096,
    }
}

#[test]
fn executes_destructive_steps_in_fixed_order() {
    let backend = FakeBackend {
        calls: Mutex::new(vec![]),
        fail_at: None,
    };
    execute_shrink(&backend, &request()).expect("transaction");
    assert_eq!(
        *backend.calls.lock().expect("lock"),
        vec![
            ShrinkStage::Unmount,
            ShrinkStage::PreflightFsck,
            ShrinkStage::InstallFirstBoot,
            ShrinkStage::UnmountAfterInstall,
            ShrinkStage::PreResizeFsck,
            ShrinkStage::ResizeFilesystem,
            ShrinkStage::PostResizeFsck,
            ShrinkStage::ResizePartition,
            ShrinkStage::RereadPartitionTable,
            ShrinkStage::Revalidate,
        ]
    );
}

#[test]
fn failure_stops_without_rollback() {
    let backend = FakeBackend {
        calls: Mutex::new(vec![]),
        fail_at: Some(ShrinkStage::ResizeFilesystem),
    };
    assert_eq!(
        execute_shrink(&backend, &request()),
        Err(ShrinkError {
            stage: ShrinkStage::ResizeFilesystem,
            detail: "injected".into(),
        })
    );
    assert!(
        !backend
            .calls
            .lock()
            .expect("lock")
            .contains(&ShrinkStage::ResizePartition)
    );
}

#[test]
fn post_shrink_rejects_changed_non_root_partition() {
    let request = request();
    let mut changed = mbr(request.root_end_lba);
    changed.partitions[0].end_lba += 1;
    changed.partitions[0].sectors += 1;
    assert!(validate_post_shrink(&request, &changed, geometry(120 * 1024 * 1024)).is_err());
}

#[test]
fn post_shrink_rejects_changed_root_start() {
    let request = request();
    let mut changed = mbr(request.root_end_lba);
    changed.partitions[1].start_lba += 1;
    assert!(validate_post_shrink(&request, &changed, geometry(120 * 1024 * 1024)).is_err());
}

#[test]
fn post_shrink_rejects_filesystem_larger_than_partition() {
    let request = request();
    let changed = mbr(request.root_end_lba);
    let partition_bytes = (request.root_end_lba - request.root_start_lba + 1) * 512;
    assert!(validate_post_shrink(&request, &changed, geometry(partition_bytes + 4096)).is_err());
}

#[test]
fn post_shrink_accepts_exact_planned_geometry() {
    let request = request();
    let changed = mbr(request.root_end_lba);
    validate_post_shrink(
        &request,
        &changed,
        geometry(request.target_filesystem_kib * 1024),
    )
    .expect("valid post-shrink state");
}
