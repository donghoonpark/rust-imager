//! Guarded shrink transaction tests.

use std::sync::Mutex;

use rust_imager_linux::shrink::{
    ShrinkBackend, ShrinkError, ShrinkRequest, ShrinkStage, execute_shrink,
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
