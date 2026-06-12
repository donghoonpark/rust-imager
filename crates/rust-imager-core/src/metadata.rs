//! Versioned image sidecar metadata.

use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::plan::{Compression, VerificationLevel};

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
            raw_bytes,
            compressed_bytes,
            compression,
            compressed_sha256,
            verification,
            first_boot_version: 1,
        }
    }
}
