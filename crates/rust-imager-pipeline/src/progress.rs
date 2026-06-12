//! Pipeline progress events.

/// Monotonic extraction progress.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Progress {
    /// Raw input bytes consumed.
    pub bytes_read: u64,
    /// Planned raw input bytes.
    pub total_bytes: u64,
}
