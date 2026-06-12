//! Versioned image sidecar metadata.

use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::plan::{Compression, VerificationLevel};

/// Partition geometry captured in the sidecar.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PartitionMetadata {
    /// One-based MBR partition number.
    pub number: u8,
    /// Raw MBR partition type.
    pub type_code: u8,
    /// First LBA.
    pub start_lba: u64,
    /// Inclusive final LBA in the extracted image.
    pub end_lba: u64,
}

/// Verification outcome.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VerificationStatus {
    /// User selected no verification.
    NotPerformed,
    /// Requested verification completed successfully.
    Passed,
}

/// Detailed verification result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerificationResult {
    /// Requested verification level.
    pub level: VerificationLevel,
    /// Outcome.
    pub status: VerificationStatus,
    /// Raw SHA-256 when calculated.
    pub raw_sha256: Option<String>,
    /// Raw bytes decoded or reread.
    pub raw_bytes: u64,
}

/// Stable JSON metadata written next to a successful image.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImageMetadata {
    /// Sidecar schema version.
    pub schema_version: u32,
    /// rust-imager package version.
    pub tool_version: String,
    /// Completion timestamp in seconds since Unix epoch.
    pub completed_unix_seconds: u64,
    /// Captured source device path.
    pub source_device: String,
    /// Source capacity before shrinking.
    pub source_size_bytes: Option<u64>,
    /// Logical sector size.
    pub sector_size: Option<u64>,
    /// Extracted MBR partition geometry.
    pub partitions: Vec<PartitionMetadata>,
    /// Raw image range size.
    pub raw_bytes: u64,
    /// Compressed file size.
    pub compressed_bytes: u64,
    /// Compression policy.
    pub compression: Compression,
    /// Compressed file SHA-256.
    pub compressed_sha256: String,
    /// Verification result.
    pub verification: VerificationResult,
    /// Installed first-boot asset version.
    pub first_boot_version: u32,
}

impl ImageMetadata {
    /// Construct current-schema metadata.
    #[must_use]
    pub fn new(
        source_device: String,
        raw_bytes: u64,
        compressed_bytes: u64,
        compression: Compression,
        verification: VerificationResult,
        compressed_sha256: String,
    ) -> Self {
        Self {
            schema_version: 1,
            tool_version: env!("CARGO_PKG_VERSION").into(),
            completed_unix_seconds: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_or(0, |duration| duration.as_secs()),
            source_device,
            source_size_bytes: None,
            sector_size: None,
            partitions: vec![],
            raw_bytes,
            compressed_bytes,
            compression,
            compressed_sha256,
            verification,
            first_boot_version: 1,
        }
    }

    /// Attach captured source and final partition geometry.
    #[must_use]
    pub fn with_source_layout(
        mut self,
        source_size_bytes: u64,
        sector_size: u64,
        partitions: Vec<PartitionMetadata>,
    ) -> Self {
        self.source_size_bytes = Some(source_size_bytes);
        self.sector_size = Some(sector_size);
        self.partitions = partitions;
        self
    }
}
