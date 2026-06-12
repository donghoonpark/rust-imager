//! Bounded threaded extraction and compression.

use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, SyncSender, sync_channel};
use std::thread;

use rust_imager_core::plan::Compression;
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::progress::Progress;

/// Extraction pipeline configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExtractOptions {
    /// Exact raw byte count to read from offset zero.
    pub bytes: u64,
    /// Streaming compressor.
    pub compression: Compression,
    /// Whether to hash raw bytes in the reader.
    pub raw_hash: bool,
    /// Per-buffer size.
    pub buffer_bytes: usize,
    /// Maximum queued buffers.
    pub queue_depth: usize,
}

/// Successful extraction statistics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtractResult {
    /// Raw bytes read.
    pub raw_bytes: u64,
    /// Optional raw SHA-256.
    pub raw_sha256: Option<String>,
    /// SHA-256 of the compressed file.
    pub compressed_sha256: String,
    /// Final compressed file size.
    pub compressed_bytes: u64,
}

/// Extraction failure.
#[derive(Debug, Error)]
pub enum ExtractError {
    /// Invalid buffer or byte configuration.
    #[error("invalid extraction options")]
    InvalidOptions,
    /// XZ is implemented by the compression module in the next feature stage.
    #[error("compression format is not available")]
    UnsupportedCompression,
    /// A source, output, sync, hash, or rename operation failed.
    #[error("I/O failure at {path}: {source}")]
    Io {
        /// Affected path.
        path: PathBuf,
        /// Operating system error.
        source: std::io::Error,
    },
    /// Source ended before the planned range.
    #[error("source ended after {actual} of {expected} bytes")]
    UnexpectedEof {
        /// Planned bytes.
        expected: u64,
        /// Bytes actually read.
        actual: u64,
    },
    /// Reader thread terminated unexpectedly.
    #[error("reader thread failed")]
    ReaderThread,
}

enum Message {
    Data(Vec<u8>),
    Done(Option<String>),
    Failed(ExtractError),
}

/// Extract an exact path range into an atomically finalized compressed image.
///
/// The reader owns a fixed number of reusable-sized buffers through a bounded
/// channel. Compression and output therefore apply backpressure without memory
/// growth proportional to device size.
///
/// # Errors
///
/// Returns [`ExtractError`] for invalid options, short reads, unsupported
/// compression, thread failure, compression failure, filesystem failure, or
/// finalization failure. The `.partial` file is retained on failure.
pub fn extract_path(
    source: &Path,
    output: &Path,
    options: &ExtractOptions,
    mut on_progress: impl FnMut(Progress),
) -> Result<ExtractResult, ExtractError> {
    if options.bytes == 0 || options.buffer_bytes == 0 || options.queue_depth == 0 {
        return Err(ExtractError::InvalidOptions);
    }
    let Compression::Zstandard { level } = options.compression else {
        return Err(ExtractError::UnsupportedCompression);
    };

    let partial = partial_path(output);
    let output_file = File::create(&partial).map_err(|source| io_error(&partial, source))?;
    let (sender, receiver) = sync_channel(options.queue_depth);
    let source_path = source.to_path_buf();
    let bytes = options.bytes;
    let buffer_bytes = options.buffer_bytes;
    let raw_hash = options.raw_hash;
    let reader =
        thread::spawn(move || read_source(&source_path, bytes, buffer_bytes, raw_hash, &sender));

    let mut encoder =
        zstd::Encoder::new(output_file, level).map_err(|source| io_error(&partial, source))?;
    let (raw_bytes, raw_sha256) =
        consume(&receiver, &mut encoder, options.bytes, &mut on_progress)?;
    let output_file = encoder
        .finish()
        .map_err(|source| io_error(&partial, source))?;
    output_file
        .sync_all()
        .map_err(|source| io_error(&partial, source))?;
    reader.join().map_err(|_| ExtractError::ReaderThread)?;

    let compressed_sha256 = hash_file(&partial)?;
    let compressed_bytes = fs::metadata(&partial)
        .map_err(|source| io_error(&partial, source))?
        .len();
    fs::rename(&partial, output).map_err(|source| io_error(output, source))?;
    sync_parent(output)?;
    Ok(ExtractResult {
        raw_bytes,
        raw_sha256,
        compressed_sha256,
        compressed_bytes,
    })
}

fn read_source(
    path: &Path,
    expected: u64,
    buffer_bytes: usize,
    raw_hash: bool,
    sender: &SyncSender<Message>,
) {
    let result = (|| -> Result<Option<String>, ExtractError> {
        let mut file = File::open(path).map_err(|source| io_error(path, source))?;
        let mut remaining = expected;
        let mut actual = 0_u64;
        let mut hasher = raw_hash.then(Sha256::new);
        while remaining > 0 {
            let buffer_limit =
                u64::try_from(buffer_bytes).map_err(|_| ExtractError::InvalidOptions)?;
            let wanted = usize::try_from(remaining.min(buffer_limit))
                .map_err(|_| ExtractError::InvalidOptions)?;
            let mut buffer = vec![0_u8; wanted];
            let read = file
                .read(&mut buffer)
                .map_err(|source| io_error(path, source))?;
            if read == 0 {
                return Err(ExtractError::UnexpectedEof { expected, actual });
            }
            buffer.truncate(read);
            if let Some(hasher) = &mut hasher {
                hasher.update(&buffer);
            }
            actual = actual
                .checked_add(u64::try_from(read).map_err(|_| ExtractError::InvalidOptions)?)
                .ok_or(ExtractError::InvalidOptions)?;
            remaining -= u64::try_from(read).map_err(|_| ExtractError::InvalidOptions)?;
            if sender.send(Message::Data(buffer)).is_err() {
                return Ok(None);
            }
        }
        Ok(hasher.map(|hasher| hex(&hasher.finalize())))
    })();
    let message = match result {
        Ok(hash) => Message::Done(hash),
        Err(error) => Message::Failed(error),
    };
    let _ = sender.send(message);
}

fn consume(
    receiver: &Receiver<Message>,
    writer: &mut impl Write,
    total: u64,
    on_progress: &mut impl FnMut(Progress),
) -> Result<(u64, Option<String>), ExtractError> {
    let mut consumed = 0_u64;
    loop {
        match receiver.recv().map_err(|_| ExtractError::ReaderThread)? {
            Message::Data(buffer) => {
                writer
                    .write_all(&buffer)
                    .map_err(|source| ExtractError::Io {
                        path: PathBuf::from("<compressor>"),
                        source,
                    })?;
                consumed = consumed
                    .checked_add(
                        u64::try_from(buffer.len()).map_err(|_| ExtractError::InvalidOptions)?,
                    )
                    .ok_or(ExtractError::InvalidOptions)?;
                on_progress(Progress {
                    bytes_read: consumed,
                    total_bytes: total,
                });
            }
            Message::Done(hash) => return Ok((consumed, hash)),
            Message::Failed(error) => return Err(error),
        }
    }
}

fn hash_file(path: &Path) -> Result<String, ExtractError> {
    let mut file = File::open(path).map_err(|source| io_error(path, source))?;
    let mut hasher = Sha256::new();
    std::io::copy(&mut file, &mut hasher_writer(&mut hasher))
        .map_err(|source| io_error(path, source))?;
    Ok(hex(&hasher.finalize()))
}

fn hasher_writer(hasher: &mut Sha256) -> impl Write + '_ {
    struct Writer<'a>(&'a mut Sha256);
    impl Write for Writer<'_> {
        fn write(&mut self, buffer: &[u8]) -> std::io::Result<usize> {
            self.0.update(buffer);
            Ok(buffer.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    Writer(hasher)
}

fn partial_path(output: &Path) -> PathBuf {
    let extension = output.extension().map_or_else(
        || "partial".into(),
        |value| format!("{}.partial", value.to_string_lossy()),
    );
    output.with_extension(extension)
}

fn sync_parent(output: &Path) -> Result<(), ExtractError> {
    if let Some(parent) = output.parent() {
        File::open(parent)
            .and_then(|directory| directory.sync_all())
            .map_err(|source| io_error(parent, source))?;
    }
    Ok(())
}

fn io_error(path: &Path, source: std::io::Error) -> ExtractError {
    ExtractError::Io {
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
