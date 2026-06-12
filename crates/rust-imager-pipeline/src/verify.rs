//! Compressed image verification and atomic sidecar writing.

use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use rust_imager_core::metadata::{ImageMetadata, VerificationResult, VerificationStatus};
use rust_imager_core::plan::{Compression, VerificationLevel};
use sha2::{Digest, Sha256};
use thiserror::Error;

/// Verification inputs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifyRequest {
    /// Compressed image path.
    pub image: PathBuf,
    /// Compression format.
    pub compression: Compression,
    /// Requested verification level.
    pub level: VerificationLevel,
    /// Expected raw range size.
    pub expected_raw_bytes: u64,
    /// Hash calculated during extraction, when enabled.
    pub expected_raw_sha256: Option<String>,
    /// Source device or fixture for highest-level reread.
    pub source: Option<PathBuf>,
}

/// Verification or sidecar failure.
#[derive(Debug, Error)]
pub enum VerifyError {
    /// Filesystem or decoder failure.
    #[error("verification I/O failure at {path}: {source}")]
    Io {
        /// Affected path.
        path: PathBuf,
        /// Operating system error.
        source: std::io::Error,
    },
    /// Decoded size differs from the planned range.
    #[error("raw size mismatch: expected {expected}, actual {actual}")]
    SizeMismatch {
        /// Expected bytes.
        expected: u64,
        /// Actual bytes.
        actual: u64,
    },
    /// A calculated raw hash differs from extraction or source.
    #[error("raw SHA-256 mismatch")]
    HashMismatch,
    /// Source reread was requested without a source path.
    #[error("source reread requires a source path")]
    MissingSource,
    /// Metadata serialization failed.
    #[error("metadata serialization failed: {0}")]
    Json(serde_json::Error),
    /// Existing sidecar or partial metadata would be overwritten.
    #[error("metadata output already exists: {0}")]
    OutputExists(PathBuf),
}

/// Perform the requested verification.
///
/// # Errors
///
/// Returns [`VerifyError`] for malformed compressed streams, size/hash
/// mismatches, missing source input, or I/O failures.
pub fn verify_image(request: &VerifyRequest) -> Result<VerificationResult, VerifyError> {
    if request.level == VerificationLevel::None {
        return Ok(VerificationResult {
            level: request.level,
            status: VerificationStatus::NotPerformed,
            raw_sha256: None,
            raw_bytes: 0,
        });
    }
    if request.level == VerificationLevel::StreamingHash {
        return Ok(VerificationResult {
            level: request.level,
            status: VerificationStatus::Passed,
            raw_sha256: request.expected_raw_sha256.clone(),
            raw_bytes: request.expected_raw_bytes,
        });
    }

    let (decoded_bytes, decoded_hash) = hash_decoded(request)?;
    if decoded_bytes != request.expected_raw_bytes {
        return Err(VerifyError::SizeMismatch {
            expected: request.expected_raw_bytes,
            actual: decoded_bytes,
        });
    }
    if request
        .expected_raw_sha256
        .as_ref()
        .is_some_and(|expected| expected != &decoded_hash)
    {
        return Err(VerifyError::HashMismatch);
    }
    if request.level == VerificationLevel::SourceReread {
        let source = request
            .source
            .as_deref()
            .ok_or(VerifyError::MissingSource)?;
        let (source_bytes, source_hash) = hash_prefix(source, request.expected_raw_bytes)?;
        if source_bytes != decoded_bytes {
            return Err(VerifyError::SizeMismatch {
                expected: decoded_bytes,
                actual: source_bytes,
            });
        }
        if source_hash != decoded_hash {
            return Err(VerifyError::HashMismatch);
        }
    }
    Ok(VerificationResult {
        level: request.level,
        status: VerificationStatus::Passed,
        raw_sha256: Some(decoded_hash),
        raw_bytes: decoded_bytes,
    })
}

/// Atomically write `<image-extension>.json` metadata.
///
/// # Errors
///
/// Returns [`VerifyError`] for JSON serialization, write, sync, or rename
/// failures.
pub fn write_sidecar(image: &Path, metadata: &ImageMetadata) -> Result<PathBuf, VerifyError> {
    let extension = image.extension().map_or_else(
        || "json".into(),
        |value| format!("{}.json", value.to_string_lossy()),
    );
    let sidecar = image.with_extension(extension);
    let partial = sidecar.with_extension("json.partial");
    if sidecar.exists() {
        return Err(VerifyError::OutputExists(sidecar));
    }
    if partial.exists() {
        return Err(VerifyError::OutputExists(partial));
    }
    let json = serde_json::to_vec_pretty(metadata).map_err(VerifyError::Json)?;
    let mut file = File::create(&partial).map_err(|source| io_error(&partial, source))?;
    file.write_all(&json)
        .map_err(|source| io_error(&partial, source))?;
    file.write_all(b"\n")
        .map_err(|source| io_error(&partial, source))?;
    file.sync_all()
        .map_err(|source| io_error(&partial, source))?;
    fs::rename(&partial, &sidecar).map_err(|source| io_error(&sidecar, source))?;
    Ok(sidecar)
}

fn hash_decoded(request: &VerifyRequest) -> Result<(u64, String), VerifyError> {
    let file = File::open(&request.image).map_err(|source| io_error(&request.image, source))?;
    let mut reader: Box<dyn Read> = match request.compression {
        Compression::Zstandard { .. } => {
            Box::new(zstd::Decoder::new(file).map_err(|source| io_error(&request.image, source))?)
        }
        Compression::Xz { .. } => Box::new(xz2::read::XzDecoder::new(file)),
    };
    hash_reader(&mut reader, &request.image, None)
}

fn hash_prefix(path: &Path, bytes: u64) -> Result<(u64, String), VerifyError> {
    let mut file = File::open(path).map_err(|source| io_error(path, source))?;
    hash_reader(&mut file, path, Some(bytes))
}

fn hash_reader(
    reader: &mut dyn Read,
    path: &Path,
    limit: Option<u64>,
) -> Result<(u64, String), VerifyError> {
    let mut hasher = Sha256::new();
    let mut count = 0_u64;
    let mut buffer = vec![0_u8; 1024 * 1024];
    loop {
        let wanted = limit.map_or(buffer.len(), |limit| {
            usize::try_from(limit.saturating_sub(count))
                .unwrap_or(usize::MAX)
                .min(buffer.len())
        });
        if wanted == 0 {
            break;
        }
        let read = reader
            .read(&mut buffer[..wanted])
            .map_err(|source| io_error(path, source))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
        count = count
            .checked_add(
                u64::try_from(read)
                    .map_err(|_| io_error(path, std::io::Error::other("read count overflow")))?,
            )
            .ok_or_else(|| io_error(path, std::io::Error::other("byte count overflow")))?;
    }
    Ok((count, hex(&hasher.finalize())))
}

fn io_error(path: &Path, source: std::io::Error) -> VerifyError {
    VerifyError::Io {
        path: path.to_path_buf(),
        source,
    }
}

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    bytes.iter().fold(
        String::with_capacity(bytes.len() * 2),
        |mut output, byte| {
            let _ = write!(output, "{byte:02x}");
            output
        },
    )
}
