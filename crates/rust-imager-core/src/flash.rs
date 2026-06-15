//! Image flashing policies.

use std::path::Path;

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Supported flash image encodings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImageFormat {
    /// Uncompressed raw disk image.
    Raw,
    /// Zstandard-compressed raw image.
    Zstandard,
    /// XZ-compressed raw image.
    Xz,
}

/// Input verification performed before target mutation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PreVerify {
    /// Decode only as required to determine the safe write length.
    None,
    /// Decode and validate DOS/MBR geometry.
    Basic,
    /// Basic verification plus available sidecar hashes.
    Full,
}

/// Optional verification after writing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PostVerify {
    /// Do not reread the target.
    None,
    /// Hash the complete written target range.
    Full,
}

/// Unsupported image filename.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[error("image must end in .img, .img.zst, or .img.xz")]
pub struct ImageFormatError;

impl ImageFormat {
    /// Infer the format from a supported filename.
    pub fn from_path(path: &Path) -> Result<Self, ImageFormatError> {
        let name = path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or_default();
        if name.ends_with(".img.zst") {
            Ok(Self::Zstandard)
        } else if name.ends_with(".img.xz") {
            Ok(Self::Xz)
        } else if name.ends_with(".img") {
            Ok(Self::Raw)
        } else {
            Err(ImageFormatError)
        }
    }
}
