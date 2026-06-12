//! Stable block-device identity types.

use serde::{Deserialize, Serialize};

/// Identity captured before a destructive operation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceIdentity {
    /// Whole-disk device path.
    pub path: String,
    /// Kernel major:minor identifier.
    pub major_minor: String,
    /// Device-reported serial number.
    pub serial: String,
    /// Device-reported model name.
    pub model: String,
    /// Capacity in bytes.
    pub size_bytes: u64,
    /// Kernel transport name.
    pub transport: String,
    /// Whether the kernel marks the device removable.
    pub removable: bool,
}
