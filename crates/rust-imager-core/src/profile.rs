//! Conservative SBC layout classification.

use serde::{Deserialize, Serialize};

use crate::filesystem::{FilesystemEvidence, PartitionEvidence};
use crate::mbr::{Mbr, PartitionKind};

/// A recognized board image family.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Profile {
    /// Raspberry Pi OS-style FAT boot files.
    RaspberryPi,
    /// Hardkernel ODROID-style FAT boot files.
    Odroid,
}

/// Result of mandatory layout checks and optional profile matching.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum LayoutClass {
    /// Mandatory checks pass and a known profile matched.
    Recognized(Profile),
    /// Mandatory checks pass, but no known profile matched.
    WarningUnknown,
    /// The layout must not be modified.
    Unsupported(String),
}

/// Classify an MBR layout using filesystem and file evidence.
#[must_use]
pub fn classify_layout(mbr: &Mbr, evidence: &[PartitionEvidence]) -> LayoutClass {
    let Some(last) = mbr
        .partitions
        .iter()
        .max_by_key(|partition| partition.end_lba)
    else {
        return LayoutClass::Unsupported("no partitions".into());
    };
    let Some(root) = evidence.iter().find(|item| item.number == last.number) else {
        return LayoutClass::Unsupported("last partition was not inspected".into());
    };
    if last.kind != PartitionKind::Linux
        || root.filesystem != FilesystemEvidence::Ext4
        || !root.looks_like_linux_root()
    {
        return LayoutClass::Unsupported(
            "last primary partition must be an ext4 Linux root".into(),
        );
    }

    let Some(boot) = evidence
        .iter()
        .find(|item| item.filesystem == FilesystemEvidence::Fat)
    else {
        return LayoutClass::Unsupported("FAT boot partition not found".into());
    };
    let has = |name: &str| boot.boot_files.iter().any(|file| file == name);
    if has("config.txt") && boot.boot_files.iter().any(|file| file.starts_with("start")) {
        LayoutClass::Recognized(Profile::RaspberryPi)
    } else if has("boot.ini") {
        LayoutClass::Recognized(Profile::Odroid)
    } else {
        LayoutClass::WarningUnknown
    }
}
