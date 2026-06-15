//! Parse Linux block topology and select safe external candidates.

use std::collections::{HashMap, HashSet};
use std::path::Path;

use rust_imager_core::device::DeviceIdentity;
use serde::Deserialize;
use thiserror::Error;

/// Discovery and identity validation failures.
#[derive(Debug, Error)]
pub enum DiscoveryError {
    /// `lsblk` JSON could not be parsed.
    #[error("invalid lsblk JSON: {0}")]
    InvalidLsblk(serde_json::Error),
    /// `findmnt` JSON could not be parsed.
    #[error("invalid findmnt JSON: {0}")]
    InvalidFindmnt(serde_json::Error),
    /// The selected device no longer has the captured identity.
    #[error("selected device identity changed")]
    IdentityChanged,
    /// Output-device filtering requires a normalized absolute path.
    #[error("output path must be absolute before device discovery")]
    OutputPathNotAbsolute,
}

impl PartialEq for DiscoveryError {
    fn eq(&self, other: &Self) -> bool {
        matches!(
            (self, other),
            (Self::InvalidLsblk(_), Self::InvalidLsblk(_))
                | (Self::InvalidFindmnt(_), Self::InvalidFindmnt(_))
                | (Self::IdentityChanged, Self::IdentityChanged)
                | (Self::OutputPathNotAbsolute, Self::OutputPathNotAbsolute)
        )
    }
}

#[derive(Debug, Deserialize)]
struct Lsblk {
    blockdevices: Vec<BlockDevice>,
}

#[derive(Debug, Deserialize)]
struct BlockDevice {
    #[serde(rename = "name")]
    path: String,
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    size: u64,
    #[serde(rename = "log-sec", default = "default_sector_size")]
    logical_sector_size: u64,
    #[serde(default)]
    tran: Option<String>,
    #[serde(default)]
    model: Option<String>,
    #[serde(default)]
    serial: Option<String>,
    #[serde(rename = "maj:min", default)]
    major_minor: String,
    #[serde(default)]
    rm: bool,
    #[serde(default)]
    fstype: Option<String>,
    #[serde(default)]
    children: Vec<BlockDevice>,
}

#[derive(Debug, Deserialize)]
struct Findmnt {
    #[serde(default)]
    filesystems: Vec<Filesystem>,
}

#[derive(Debug, Deserialize)]
struct Filesystem {
    target: String,
    source: String,
}

/// Select unused whole USB disks with `/dev/sdX` names.
///
/// # Errors
///
/// Returns [`DiscoveryError`] when either machine-readable input is invalid.
pub fn discover_candidates(
    lsblk_json: &str,
    findmnt_json: &str,
    output_path: Option<&str>,
) -> Result<Vec<DeviceIdentity>, DiscoveryError> {
    if output_path.is_some_and(|output| !Path::new(output).is_absolute()) {
        return Err(DiscoveryError::OutputPathNotAbsolute);
    }
    let topology: Lsblk = serde_json::from_str(lsblk_json).map_err(DiscoveryError::InvalidLsblk)?;
    let mounts: Findmnt =
        serde_json::from_str(findmnt_json).map_err(DiscoveryError::InvalidFindmnt)?;

    let mut parent_by_child = HashMap::new();
    let mut excluded = HashSet::new();
    for disk in &topology.blockdevices {
        index_children(disk, &disk.path, &mut parent_by_child);
        if contains_swap(disk) {
            excluded.insert(disk.path.clone());
        }
    }

    for filesystem in &mounts.filesystems {
        if filesystem.target == "/"
            || filesystem.target == "/boot"
            || filesystem.target == "/boot/efi"
            || output_path.is_some_and(|output| path_contains(&filesystem.target, output))
        {
            excluded.insert(
                parent_by_child
                    .get(filesystem.source.as_str())
                    .cloned()
                    .unwrap_or_else(|| filesystem.source.clone()),
            );
        }
    }

    let mut candidates: Vec<_> = topology
        .blockdevices
        .into_iter()
        .filter(|device| {
            device.kind == "disk"
                && valid_sd_path(&device.path)
                && device.tran.as_deref() == Some("usb")
                && device.logical_sector_size == 512
                && !excluded.contains(&device.path)
        })
        .map(|device| DeviceIdentity {
            path: device.path,
            major_minor: device.major_minor,
            serial: device.serial.unwrap_or_default().trim().to_owned(),
            model: device.model.unwrap_or_default().trim().to_owned(),
            size_bytes: device.size,
            logical_sector_size: device.logical_sector_size,
            transport: device.tran.unwrap_or_default(),
            removable: device.rm,
        })
        .collect();
    candidates.sort_unstable_by(|left, right| left.path.cmp(&right.path));
    Ok(candidates)
}

fn contains_swap(device: &BlockDevice) -> bool {
    device.fstype.as_deref() == Some("swap") || device.children.iter().any(contains_swap)
}

const fn default_sector_size() -> u64 {
    512
}

/// Verify that a newly read identity is exactly the selected device.
///
/// # Errors
///
/// Returns [`DiscoveryError::IdentityChanged`] on any mismatch.
pub fn revalidate_identity(
    selected: &DeviceIdentity,
    current: &DeviceIdentity,
) -> Result<(), DiscoveryError> {
    if selected == current {
        Ok(())
    } else {
        Err(DiscoveryError::IdentityChanged)
    }
}

fn index_children(device: &BlockDevice, disk: &str, output: &mut HashMap<String, String>) {
    output.insert(device.path.clone(), disk.to_owned());
    for child in &device.children {
        index_children(child, disk, output);
    }
}

fn valid_sd_path(path: &str) -> bool {
    let Some(name) = Path::new(path).file_name().and_then(|name| name.to_str()) else {
        return false;
    };
    name.len() >= 3
        && name.starts_with("sd")
        && name[2..].bytes().all(|byte| byte.is_ascii_lowercase())
}

fn path_contains(mountpoint: &str, output: &str) -> bool {
    let mount = Path::new(mountpoint);
    Path::new(output).starts_with(mount)
}
