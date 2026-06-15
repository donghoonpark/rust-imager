//! Flash image inspection and bounded streaming writes.

use std::fs::{self, File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use rust_imager_core::flash::{ImageFormat, PostVerify, PreVerify};
use rust_imager_core::mbr::{Mbr, parse_mbr};
use rust_imager_core::metadata::ImageMetadata;
use sha2::{Digest, Sha256};
use thiserror::Error;

/// Input inspection request.
#[derive(Debug, Clone)]
pub struct InspectRequest {
    /// Input image path.
    pub image: PathBuf,
    /// Input encoding.
    pub format: ImageFormat,
    /// Requested verification level.
    pub level: PreVerify,
}

/// Facts established before writing.
#[derive(Debug, Clone)]
pub struct InspectedImage {
    /// Decoded image length.
    pub raw_bytes: u64,
    /// SHA-256 of decoded bytes.
    pub raw_sha256: Option<String>,
    /// Whether adjacent metadata was found.
    pub sidecar_present: bool,
    /// Validated MBR for basic or full verification.
    pub mbr: Option<Mbr>,
}

/// Streaming write request.
#[derive(Debug, Clone)]
pub struct FlashRequest {
    /// Input image path.
    pub image: PathBuf,
    /// Input encoding.
    pub format: ImageFormat,
    /// Target block device or test file.
    pub target: PathBuf,
    /// Capacity reported by trusted target discovery.
    pub target_capacity: u64,
    /// Exact decoded length established before writing.
    pub expected_raw_bytes: u64,
    /// Expected decoded hash when available.
    pub expected_raw_sha256: Option<String>,
    /// Optional target reread policy.
    pub post_verify: PostVerify,
}

/// Successful write facts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlashResult {
    /// Bytes durably written.
    pub bytes_written: u64,
    /// SHA-256 calculated while writing.
    pub raw_sha256: String,
    /// Whether target reread verification passed.
    pub post_verified: bool,
}

/// Input inspection, writing, or verification failure.
#[derive(Debug, Error)]
pub enum FlashError {
    /// Filesystem or decoder failure.
    #[error("flash I/O failure at {path}: {source}")]
    Io {
        /// Affected path.
        path: PathBuf,
        /// Underlying error.
        source: std::io::Error,
    },
    /// Sidecar JSON is invalid.
    #[error("image metadata failure: {0}")]
    Metadata(serde_json::Error),
    /// Decoded disk layout is invalid.
    #[error("image MBR validation failed: {0}")]
    Mbr(#[from] rust_imager_core::mbr::MbrError),
    /// Decoded size differs from established metadata.
    #[error("raw size mismatch: expected {expected}, actual {actual}")]
    SizeMismatch {
        /// Expected bytes.
        expected: u64,
        /// Actual bytes.
        actual: u64,
    },
    /// A calculated hash differs from metadata or target reread.
    #[error("SHA-256 mismatch")]
    HashMismatch,
    /// Target cannot contain the decoded image.
    #[error("target capacity {capacity} is smaller than image {image}")]
    TargetTooSmall {
        /// Target capacity.
        capacity: u64,
        /// Decoded image bytes.
        image: u64,
    },
}

/// Decode and validate an image before target mutation.
///
/// # Errors
///
/// Returns [`FlashError`] for I/O, decoder, metadata, hash, or MBR failures.
pub fn inspect_image(request: &InspectRequest) -> Result<InspectedImage, FlashError> {
    let sidecar_path = sidecar_path(&request.image);
    let metadata = if sidecar_path.exists() {
        Some(
            serde_json::from_slice::<ImageMetadata>(
                &fs::read(&sidecar_path).map_err(|source| io_error(&sidecar_path, source))?,
            )
            .map_err(FlashError::Metadata)?,
        )
    } else {
        None
    };
    let compressed_hash = (request.level == PreVerify::Full)
        .then(|| hash_file(&request.image))
        .transpose()?;
    if let (Some(metadata), Some(hash)) = (&metadata, &compressed_hash)
        && metadata.compressed_sha256 != *hash
    {
        return Err(FlashError::HashMismatch);
    }

    let mut reader = decoder(&request.image, request.format)?;
    let mut first = [0_u8; 1024];
    let mut first_len = 0;
    let mut count = 0_u64;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0_u8; 1024 * 1024];
    loop {
        let read = reader
            .read(&mut buffer)
            .map_err(|source| io_error(&request.image, source))?;
        if read == 0 {
            break;
        }
        let copy = (first.len() - first_len).min(read);
        first[first_len..first_len + copy].copy_from_slice(&buffer[..copy]);
        first_len += copy;
        hasher.update(&buffer[..read]);
        count += read as u64;
    }
    if let Some(metadata) = &metadata
        && metadata.raw_bytes != count
    {
        return Err(FlashError::SizeMismatch {
            expected: metadata.raw_bytes,
            actual: count,
        });
    }
    let raw_hash = hex(&hasher.finalize());
    if request.level == PreVerify::Full
        && let Some(expected) = metadata
            .as_ref()
            .and_then(|value| value.verification.raw_sha256.as_ref())
        && expected != &raw_hash
    {
        return Err(FlashError::HashMismatch);
    }
    let mbr = if request.level == PreVerify::None {
        None
    } else {
        let mut sector = [0_u8; 512];
        if first_len < sector.len() {
            return Err(FlashError::SizeMismatch {
                expected: 512,
                actual: count,
            });
        }
        sector.copy_from_slice(&first[..512]);
        Some(parse_mbr(
            &sector,
            (first_len >= 520).then_some(&first[512..520]),
            count / 512,
        )?)
    };
    Ok(InspectedImage {
        raw_bytes: count,
        raw_sha256: Some(raw_hash),
        sidecar_present: metadata.is_some(),
        mbr,
    })
}

/// Decode and durably stream an image to a target.
///
/// # Errors
///
/// Returns [`FlashError`] for target capacity, I/O, size, or hash failures.
pub fn flash_image(
    request: &FlashRequest,
    mut progress: impl FnMut(u64, u64),
) -> Result<FlashResult, FlashError> {
    let capacity = request.target_capacity;
    if capacity < request.expected_raw_bytes {
        return Err(FlashError::TargetTooSmall {
            capacity,
            image: request.expected_raw_bytes,
        });
    }
    let mut reader = decoder(&request.image, request.format)?;
    let mut target = OpenOptions::new()
        .write(true)
        .open(&request.target)
        .map_err(|source| io_error(&request.target, source))?;
    let mut hasher = Sha256::new();
    let mut written = 0_u64;
    let mut buffer = vec![0_u8; 1024 * 1024];
    loop {
        let read = reader
            .read(&mut buffer)
            .map_err(|source| io_error(&request.image, source))?;
        if read == 0 {
            break;
        }
        written += read as u64;
        if written > request.expected_raw_bytes {
            return Err(FlashError::SizeMismatch {
                expected: request.expected_raw_bytes,
                actual: written,
            });
        }
        target
            .write_all(&buffer[..read])
            .map_err(|source| io_error(&request.target, source))?;
        hasher.update(&buffer[..read]);
        progress(written, request.expected_raw_bytes);
    }
    if written != request.expected_raw_bytes {
        return Err(FlashError::SizeMismatch {
            expected: request.expected_raw_bytes,
            actual: written,
        });
    }
    target
        .flush()
        .and_then(|()| target.sync_all())
        .map_err(|source| io_error(&request.target, source))?;
    let hash = hex(&hasher.finalize());
    if request
        .expected_raw_sha256
        .as_ref()
        .is_some_and(|expected| expected != &hash)
    {
        return Err(FlashError::HashMismatch);
    }
    if request.post_verify == PostVerify::Full {
        target
            .seek(SeekFrom::Start(0))
            .map_err(|source| io_error(&request.target, source))?;
        drop(target);
        let reread = hash_prefix(&request.target, written)?;
        if reread != hash {
            return Err(FlashError::HashMismatch);
        }
    }
    Ok(FlashResult {
        bytes_written: written,
        raw_sha256: hash,
        post_verified: request.post_verify == PostVerify::Full,
    })
}

/// Return the adjacent metadata path.
#[must_use]
pub fn sidecar_path(image: &Path) -> PathBuf {
    let mut value = image.as_os_str().to_os_string();
    value.push(".json");
    PathBuf::from(value)
}

fn decoder(path: &Path, format: ImageFormat) -> Result<Box<dyn Read>, FlashError> {
    let file = File::open(path).map_err(|source| io_error(path, source))?;
    match format {
        ImageFormat::Raw => Ok(Box::new(file)),
        ImageFormat::Zstandard => Ok(Box::new(
            zstd::Decoder::new(file).map_err(|source| io_error(path, source))?,
        )),
        ImageFormat::Xz => Ok(Box::new(xz2::read::XzDecoder::new(file))),
    }
}

fn hash_file(path: &Path) -> Result<String, FlashError> {
    hash_prefix(path, u64::MAX)
}

fn hash_prefix(path: &Path, limit: u64) -> Result<String, FlashError> {
    let mut file = File::open(path).map_err(|source| io_error(path, source))?;
    let mut hasher = Sha256::new();
    let mut remaining = limit;
    let mut buffer = vec![0_u8; 1024 * 1024];
    while remaining > 0 {
        let wanted = buffer
            .len()
            .min(usize::try_from(remaining).unwrap_or(usize::MAX));
        let read = file
            .read(&mut buffer[..wanted])
            .map_err(|source| io_error(path, source))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
        remaining -= read as u64;
    }
    Ok(hex(&hasher.finalize()))
}

fn io_error(path: &Path, source: std::io::Error) -> FlashError {
    FlashError::Io {
        path: path.to_path_buf(),
        source,
    }
}

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    bytes.iter().fold(String::new(), |mut output, byte| {
        let _ = write!(output, "{byte:02x}");
        output
    })
}
