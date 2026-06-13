//! Ordered, fail-closed filesystem and MBR shrink transaction.

use std::path::PathBuf;

use rust_imager_core::mbr::Mbr;
use rust_imager_first_boot::install::install;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::command::Runner;
use crate::inspect::{Ext4Geometry, ext4_geometry, read_mbr_from_disk};
use crate::quiescence::{quiesce_disk, sysfs_holders};
use crate::tools::{E2fsTools, MountTools, PartitionTools};

/// Inputs required by the destructive shrink transaction.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShrinkRequest {
    /// Whole disk path.
    pub disk: String,
    /// Root partition path.
    pub root_partition: String,
    /// Primary MBR partition number.
    pub partition_number: u8,
    /// Preserved root start LBA.
    pub root_start_lba: u64,
    /// New inclusive root end LBA.
    pub root_end_lba: u64,
    /// New ext4 size in KiB.
    pub target_filesystem_kib: u64,
    /// Original device capacity used for MBR bounds validation.
    pub device_size_bytes: u64,
    /// Supported logical sector size.
    pub logical_sector_size: u64,
    /// Original validated MBR whose non-root entries must not change.
    pub original_mbr: Mbr,
}

/// Mutation stage, in required execution order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ShrinkStage {
    /// Ensure all target filesystems are offline.
    Unmount,
    /// Initial consistency and repair check.
    PreflightFsck,
    /// Mount root and install first-boot assets.
    InstallFirstBoot,
    /// Unmount after asset installation.
    UnmountAfterInstall,
    /// Check consistency after offline root modification.
    PreResizeFsck,
    /// Shrink ext4.
    ResizeFilesystem,
    /// Verify the shrunken ext4.
    PostResizeFsck,
    /// Shrink the primary MBR entry.
    ResizePartition,
    /// Ask the kernel to reread the table.
    RereadPartitionTable,
    /// Confirm resulting geometry and identity.
    Revalidate,
}

const STAGES: [ShrinkStage; 10] = [
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
];

/// A stage executor, replaceable for tests and platform composition.
pub trait ShrinkBackend {
    /// Perform exactly one stage.
    ///
    /// # Errors
    ///
    /// Returns a diagnostic string. The transaction stops immediately and does
    /// not attempt rollback.
    fn perform(&self, stage: ShrinkStage, request: &ShrinkRequest) -> Result<(), String>;
}

/// A failed shrink stage.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("shrink failed during {stage:?}: {detail}")]
pub struct ShrinkError {
    /// Stage that failed.
    pub stage: ShrinkStage,
    /// Backend diagnostic.
    pub detail: String,
}

/// Execute all mutation stages in their fixed order.
///
/// # Errors
///
/// Returns [`ShrinkError`] at the first failed stage. Completed destructive
/// stages are never automatically reversed.
pub fn execute_shrink(
    backend: &dyn ShrinkBackend,
    request: &ShrinkRequest,
) -> Result<(), ShrinkError> {
    for stage in STAGES {
        backend
            .perform(stage, request)
            .map_err(|detail| ShrinkError { stage, detail })?;
    }
    Ok(())
}

/// Concrete Linux stage executor.
pub struct LinuxShrinkBackend<'a> {
    runner: &'a dyn Runner,
    mount_root: PathBuf,
}

impl<'a> LinuxShrinkBackend<'a> {
    /// Construct a Linux backend with a private mount directory.
    #[must_use]
    pub fn new(runner: &'a dyn Runner, mount_root: PathBuf) -> Self {
        Self { runner, mount_root }
    }
}

impl ShrinkBackend for LinuxShrinkBackend<'_> {
    fn perform(&self, stage: ShrinkStage, request: &ShrinkRequest) -> Result<(), String> {
        let e2fs = E2fsTools::new(self.runner);
        let mount = MountTools::new(self.runner);
        let partition = PartitionTools::new(self.runner);
        match stage {
            ShrinkStage::Unmount => quiesce_disk(self.runner, &request.disk, sysfs_holders)
                .map_err(|error| error.to_string()),
            ShrinkStage::UnmountAfterInstall => mount
                .unmount(&request.root_partition)
                .map(|_| ())
                .map_err(|error| error.to_string()),
            ShrinkStage::PreflightFsck
            | ShrinkStage::PreResizeFsck
            | ShrinkStage::PostResizeFsck => e2fs
                .check(&request.root_partition, false)
                .map(|_| ())
                .map_err(|error| error.to_string()),
            ShrinkStage::InstallFirstBoot => {
                std::fs::create_dir_all(&self.mount_root).map_err(|error| error.to_string())?;
                mount
                    .mount_read_write(&request.root_partition, &self.mount_root.to_string_lossy())
                    .map_err(|error| error.to_string())?;
                if let Err(error) = install(&self.mount_root) {
                    let _ = mount.unmount(&request.root_partition);
                    return Err(error.to_string());
                }
                Ok(())
            }
            ShrinkStage::ResizeFilesystem => e2fs
                .resize(&request.root_partition, request.target_filesystem_kib)
                .map(|_| ())
                .map_err(|error| error.to_string()),
            ShrinkStage::ResizePartition => partition
                .resize_primary(
                    &request.disk,
                    request.partition_number,
                    request.root_start_lba,
                    request.root_end_lba,
                )
                .map(|_| ())
                .map_err(|error| error.to_string()),
            ShrinkStage::RereadPartitionTable => partition
                .reread(&request.disk)
                .map(|_| ())
                .map_err(|error| error.to_string()),
            ShrinkStage::Revalidate => {
                let total_sectors = request
                    .device_size_bytes
                    .checked_div(request.logical_sector_size)
                    .ok_or_else(|| "invalid logical sector size".to_owned())?;
                let changed =
                    read_mbr_from_disk(std::path::Path::new(&request.disk), total_sectors)
                        .map_err(|error| error.to_string())?;
                let geometry = ext4_geometry(self.runner, &request.root_partition)
                    .map_err(|error| error.to_string())?;
                validate_post_shrink(request, &changed, geometry)?;
                e2fs.check_read_only(&request.root_partition)
                    .map(|_| ())
                    .map_err(|error| error.to_string())
            }
        }
    }
}

/// Validate MBR and ext4 invariants after destructive shrink operations.
///
/// # Errors
///
/// Returns a diagnostic when any original non-root entry changed, root identity
/// fields changed, the planned end was not applied, or ext4 does not fit.
pub fn validate_post_shrink(
    request: &ShrinkRequest,
    changed: &Mbr,
    geometry: Ext4Geometry,
) -> Result<(), String> {
    if request.logical_sector_size == 0 {
        return Err("invalid logical sector size".into());
    }
    let original_root = request
        .original_mbr
        .partitions
        .iter()
        .find(|partition| partition.number == request.partition_number)
        .ok_or_else(|| "original root partition is missing".to_owned())?;
    let changed_root = changed
        .partitions
        .iter()
        .find(|partition| partition.number == request.partition_number)
        .ok_or_else(|| "root partition is missing after shrink".to_owned())?;
    for original in request
        .original_mbr
        .partitions
        .iter()
        .filter(|partition| partition.number != request.partition_number)
    {
        let current = changed
            .partitions
            .iter()
            .find(|partition| partition.number == original.number)
            .ok_or_else(|| format!("partition {} disappeared", original.number))?;
        if current != original {
            return Err(format!(
                "partition {} changed unexpectedly",
                original.number
            ));
        }
    }
    if changed.partitions.len() != request.original_mbr.partitions.len() {
        return Err("partition count changed unexpectedly".into());
    }
    if changed_root.start_lba != original_root.start_lba
        || changed_root.start_lba != request.root_start_lba
        || changed_root.type_code != original_root.type_code
        || changed_root.kind != original_root.kind
        || changed_root.bootable != original_root.bootable
    {
        return Err("root partition identity fields changed".into());
    }
    if changed_root.end_lba != request.root_end_lba {
        return Err("root partition end does not match the plan".into());
    }
    let partition_bytes = changed_root
        .sectors
        .checked_mul(request.logical_sector_size)
        .ok_or_else(|| "partition byte size overflow".to_owned())?;
    if geometry.current_bytes > partition_bytes {
        return Err("ext4 filesystem exceeds the root partition".into());
    }
    let target_bytes = request
        .target_filesystem_kib
        .checked_mul(1024)
        .ok_or_else(|| "target filesystem size overflow".to_owned())?;
    if geometry.current_bytes > target_bytes
        || target_bytes.saturating_sub(geometry.current_bytes) >= geometry.block_size
    {
        return Err("ext4 size does not match the requested target".into());
    }
    Ok(())
}
