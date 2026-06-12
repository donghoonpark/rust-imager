//! Filesystem evidence gathered without trusting partition type bytes alone.

use serde::{Deserialize, Serialize};

/// Supported or observed filesystem type.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FilesystemEvidence {
    /// FAT12/16/32.
    Fat,
    /// ext4.
    Ext4,
    /// A known but unsupported filesystem.
    Other(String),
    /// No filesystem signature was established.
    Unknown,
}

/// Evidence associated with a primary MBR partition.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PartitionEvidence {
    /// One-based MBR partition number.
    pub number: u8,
    /// Filesystem detected by `blkid`.
    pub filesystem: FilesystemEvidence,
    /// Top-level directory names observed from a read-only mount.
    pub root_directories: Vec<String>,
    /// Relevant files observed on a boot filesystem.
    pub boot_files: Vec<String>,
}

impl PartitionEvidence {
    /// Whether this partition has the minimum Linux root directory structure.
    #[must_use]
    pub fn looks_like_linux_root(&self) -> bool {
        ["etc", "usr", "var"]
            .iter()
            .all(|required| self.root_directories.iter().any(|item| item == required))
    }
}
