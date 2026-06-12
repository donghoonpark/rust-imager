//! Raw disk, filesystem, and ext4 geometry inspection.

use std::ffi::OsString;
use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use rust_imager_core::filesystem::{FilesystemEvidence, PartitionEvidence};
use rust_imager_core::mbr::{Mbr, MbrError, parse_mbr};
use thiserror::Error;

use crate::command::{CommandSpec, Runner};

/// ext4 geometry calculated from e2fsprogs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ext4Geometry {
    /// Minimum filesystem bytes.
    pub minimum_bytes: u64,
    /// Current filesystem bytes.
    pub current_bytes: u64,
    /// ext4 block size.
    pub block_size: u64,
}

/// Inspection failure.
#[derive(Debug, Error)]
pub enum InspectError {
    /// Raw device access failed.
    #[error("failed to inspect {path}: {source}")]
    Io {
        /// Affected path.
        path: PathBuf,
        /// Operating system error.
        source: std::io::Error,
    },
    /// MBR validation failed.
    #[error(transparent)]
    Mbr(#[from] MbrError),
    /// An external inspection command failed.
    #[error("inspection command failed: {0}")]
    Command(String),
    /// e2fsprogs output was missing required numeric fields.
    #[error("unable to parse ext4 geometry")]
    InvalidExt4Geometry,
    /// Multiplication of block count and size overflowed.
    #[error("ext4 geometry overflow")]
    GeometryOverflow,
}

/// Read LBA 0 and the GPT signature area from a disk or image.
///
/// # Errors
///
/// Returns [`InspectError`] for I/O or unsupported/malformed MBR layouts.
pub fn read_mbr_from_disk(path: &Path, total_sectors: u64) -> Result<Mbr, InspectError> {
    let mut file = File::open(path).map_err(|source| io_error(path, source))?;
    let mut sector_zero = [0_u8; 512];
    file.read_exact(&mut sector_zero)
        .map_err(|source| io_error(path, source))?;
    file.seek(SeekFrom::Start(512))
        .map_err(|source| io_error(path, source))?;
    let mut lba1 = [0_u8; 8];
    file.read_exact(&mut lba1)
        .map_err(|source| io_error(path, source))?;
    parse_mbr(&sector_zero, Some(&lba1), total_sectors).map_err(InspectError::from)
}

/// Parse `resize2fs -P` and `tune2fs -l` output.
///
/// # Errors
///
/// Returns [`InspectError`] when fields are absent, non-numeric, or overflow.
pub fn parse_ext4_geometry(
    minimum_output: &str,
    tune2fs_output: &str,
) -> Result<Ext4Geometry, InspectError> {
    let minimum_blocks = last_number(minimum_output).ok_or(InspectError::InvalidExt4Geometry)?;
    let block_count = value_after_label(tune2fs_output, "Block count:")
        .ok_or(InspectError::InvalidExt4Geometry)?;
    let block_size = value_after_label(tune2fs_output, "Block size:")
        .ok_or(InspectError::InvalidExt4Geometry)?;
    Ok(Ext4Geometry {
        minimum_bytes: minimum_blocks
            .checked_mul(block_size)
            .ok_or(InspectError::GeometryOverflow)?,
        current_bytes: block_count
            .checked_mul(block_size)
            .ok_or(InspectError::GeometryOverflow)?,
        block_size,
    })
}

/// Query ext4 geometry for an offline partition.
///
/// # Errors
///
/// Returns [`InspectError`] when either e2fsprogs command or parsing fails.
pub fn ext4_geometry(runner: &dyn Runner, partition: &str) -> Result<Ext4Geometry, InspectError> {
    let minimum = runner
        .run(&spec("resize2fs", &["-P", partition]))
        .map_err(|error| InspectError::Command(error.to_string()))?;
    let details = runner
        .run(&spec("tune2fs", &["-l", partition]))
        .map_err(|error| InspectError::Command(error.to_string()))?;
    parse_ext4_geometry(&minimum.stdout, &details.stdout)
}

/// Gather filesystem and file evidence using `blkid` and temporary read-only mounts.
///
/// # Errors
///
/// Returns [`InspectError`] for command, mount, directory, or cleanup failures.
pub fn inspect_filesystems(
    runner: &dyn Runner,
    disk: &str,
    mbr: &Mbr,
    mount_base: &Path,
) -> Result<Vec<PartitionEvidence>, InspectError> {
    fs::create_dir_all(mount_base).map_err(|source| io_error(mount_base, source))?;
    let mut evidence = Vec::with_capacity(mbr.partitions.len());
    for partition in &mbr.partitions {
        let path = partition_path(disk, partition.number);
        let result = runner
            .run(&spec("blkid", &["-o", "value", "-s", "TYPE", &path]))
            .map_err(|error| InspectError::Command(error.to_string()))?;
        let filesystem = match result.stdout.trim() {
            "vfat" | "fat" | "msdos" => FilesystemEvidence::Fat,
            "ext4" => FilesystemEvidence::Ext4,
            "" => FilesystemEvidence::Unknown,
            other => FilesystemEvidence::Other(other.to_owned()),
        };
        let mountpoint = mount_base.join(format!("p{}", partition.number));
        fs::create_dir_all(&mountpoint).map_err(|source| io_error(&mountpoint, source))?;
        let options = if filesystem == FilesystemEvidence::Ext4 {
            "ro,noload"
        } else {
            "ro"
        };
        runner
            .run(&spec(
                "mount",
                &["-o", options, &path, &mountpoint.to_string_lossy()],
            ))
            .map_err(|error| InspectError::Command(error.to_string()))?;
        let inspected = inspect_mount(partition.number, filesystem, &mountpoint);
        let unmount = runner.run(&spec("umount", &[&mountpoint.to_string_lossy()]));
        let inspected = inspected?;
        unmount.map_err(|error| InspectError::Command(error.to_string()))?;
        evidence.push(inspected);
    }
    Ok(evidence)
}

/// Build a conventional partition path for `/dev/sdX`.
#[must_use]
pub fn partition_path(disk: &str, number: u8) -> String {
    if disk.as_bytes().last().is_some_and(u8::is_ascii_digit) {
        format!("{disk}p{number}")
    } else {
        format!("{disk}{number}")
    }
}

fn inspect_mount(
    number: u8,
    filesystem: FilesystemEvidence,
    mountpoint: &Path,
) -> Result<PartitionEvidence, InspectError> {
    let entries = fs::read_dir(mountpoint).map_err(|source| io_error(mountpoint, source))?;
    let mut names = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|source| io_error(mountpoint, source))?;
        if let Some(name) = entry.file_name().to_str() {
            names.push(name.to_owned());
        }
    }
    names.sort_unstable();
    Ok(PartitionEvidence {
        number,
        root_directories: if filesystem == FilesystemEvidence::Ext4 {
            names.clone()
        } else {
            vec![]
        },
        boot_files: if filesystem == FilesystemEvidence::Fat {
            names
        } else {
            vec![]
        },
        filesystem,
    })
}

fn value_after_label(output: &str, label: &str) -> Option<u64> {
    output
        .lines()
        .find_map(|line| line.strip_prefix(label))
        .and_then(|value| value.trim().parse().ok())
}

fn last_number(output: &str) -> Option<u64> {
    output
        .split_whitespace()
        .rev()
        .find_map(|value| value.parse().ok())
}

fn spec(program: &str, args: &[&str]) -> CommandSpec {
    CommandSpec::new(program, args.iter().map(OsString::from))
}

fn io_error(path: &Path, source: std::io::Error) -> InspectError {
    InspectError::Io {
        path: path.to_path_buf(),
        source,
    }
}
