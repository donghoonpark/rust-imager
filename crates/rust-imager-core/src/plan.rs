//! Immutable preflight execution planning.

use serde::{Deserialize, Serialize};
use thiserror::Error;

const MIB: u64 = 1024 * 1024;
const FIXED_MARGIN: u64 = 256 * MIB;

/// Supported streaming compression policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Compression {
    /// Zstandard compression level.
    Zstandard {
        /// Encoder level.
        level: i32,
    },
    /// XZ compression level.
    Xz {
        /// Encoder level from zero through nine.
        level: u32,
    },
}

/// Requested post-extraction verification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VerificationLevel {
    /// Only successful stream finalization and file sync.
    None,
    /// Calculate raw SHA-256 while reading.
    StreamingHash,
    /// Decode the compressed image and compare its raw hash.
    Decode,
    /// Decode and compare against a second source-device read.
    SourceReread,
}

/// Inputs gathered during preflight analysis.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanInput {
    /// Selected whole-disk path.
    pub device_path: String,
    /// Device capacity in bytes.
    pub device_size_bytes: u64,
    /// Logical sector size.
    pub sector_size: u64,
    /// First sector of the root partition.
    pub root_start_lba: u64,
    /// Smallest ext4 size reported by e2fsprogs.
    pub ext4_minimum_bytes: u64,
    /// Current ext4 size.
    pub ext4_current_bytes: u64,
    /// Final compressed output path.
    pub output_path: String,
    /// Available output filesystem bytes.
    pub output_available_bytes: u64,
    /// Whether the output filesystem passed the local-storage policy.
    pub output_is_local: bool,
    /// Compression policy.
    pub compression: Compression,
    /// Verification policy.
    pub verification: VerificationLevel,
    /// Final image alignment.
    pub alignment_bytes: u64,
}

/// Selected device facts recorded in the plan.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlannedDevice {
    /// Device path.
    pub path: String,
    /// Capacity in bytes.
    pub size_bytes: u64,
    /// Logical sector size.
    pub sector_size: u64,
}

/// Filesystem and partition shrink geometry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShrinkPlan {
    /// Root partition first LBA, preserved exactly.
    pub root_start_lba: u64,
    /// ext4 target size including safety margin.
    pub target_filesystem_bytes: u64,
    /// Safety margin added to the e2fsprogs minimum.
    pub safety_margin_bytes: u64,
    /// Inclusive final LBA after shrinking.
    pub root_end_lba: u64,
}

/// Fully checked immutable plan.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionPlan {
    /// Device facts.
    pub device: PlannedDevice,
    /// Shrink geometry.
    pub shrink: ShrinkPlan,
    /// Number of source bytes to stream from offset zero.
    pub image_bytes: u64,
    /// Final output path.
    pub output_path: String,
    /// Compression policy.
    pub compression: Compression,
    /// Verification policy.
    pub verification: VerificationLevel,
}

/// Planning failures.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum PlanError {
    /// Output is not on an accepted local filesystem.
    #[error("output must be on a local filesystem")]
    OutputNotLocal,
    /// Conservative output-space preflight failed.
    #[error("insufficient output space")]
    InsufficientOutputSpace,
    /// Geometry is zero, inconsistent, or overflows.
    #[error("invalid or overflowing storage geometry")]
    GeometryOverflow,
    /// The minimum ext4 size exceeds the current filesystem.
    #[error("ext4 minimum exceeds current filesystem size")]
    InvalidFilesystemSize,
}

/// Build a checked execution plan.
///
/// # Errors
///
/// Returns [`PlanError`] if output policy, available space, filesystem sizes,
/// sector arithmetic, or alignment are invalid.
pub fn build_plan(input: PlanInput) -> Result<ExecutionPlan, PlanError> {
    if !input.output_is_local {
        return Err(PlanError::OutputNotLocal);
    }
    if input.ext4_minimum_bytes > input.ext4_current_bytes {
        return Err(PlanError::InvalidFilesystemSize);
    }
    if input.sector_size == 0 || input.alignment_bytes == 0 {
        return Err(PlanError::GeometryOverflow);
    }

    let percentage_margin = input
        .ext4_minimum_bytes
        .checked_mul(5)
        .ok_or(PlanError::GeometryOverflow)?
        / 100;
    let safety_margin = FIXED_MARGIN.max(percentage_margin);
    let target_filesystem_bytes = input
        .ext4_minimum_bytes
        .checked_add(safety_margin)
        .ok_or(PlanError::GeometryOverflow)?
        .min(input.ext4_current_bytes);
    let root_start_bytes = input
        .root_start_lba
        .checked_mul(input.sector_size)
        .ok_or(PlanError::GeometryOverflow)?;
    let unaligned_image_bytes = root_start_bytes
        .checked_add(target_filesystem_bytes)
        .ok_or(PlanError::GeometryOverflow)?;
    let image_bytes = align_up(unaligned_image_bytes, input.alignment_bytes)?;
    if image_bytes > input.device_size_bytes {
        return Err(PlanError::GeometryOverflow);
    }
    if input.output_available_bytes < image_bytes {
        return Err(PlanError::InsufficientOutputSpace);
    }
    let sectors = image_bytes
        .checked_add(input.sector_size - 1)
        .ok_or(PlanError::GeometryOverflow)?
        / input.sector_size;
    let root_end_lba = sectors.checked_sub(1).ok_or(PlanError::GeometryOverflow)?;

    Ok(ExecutionPlan {
        device: PlannedDevice {
            path: input.device_path,
            size_bytes: input.device_size_bytes,
            sector_size: input.sector_size,
        },
        shrink: ShrinkPlan {
            root_start_lba: input.root_start_lba,
            target_filesystem_bytes,
            safety_margin_bytes: safety_margin,
            root_end_lba,
        },
        image_bytes,
        output_path: input.output_path,
        compression: input.compression,
        verification: input.verification,
    })
}

fn align_up(value: u64, alignment: u64) -> Result<u64, PlanError> {
    let remainder = value % alignment;
    if remainder == 0 {
        Ok(value)
    } else {
        value
            .checked_add(alignment - remainder)
            .ok_or(PlanError::GeometryOverflow)
    }
}
