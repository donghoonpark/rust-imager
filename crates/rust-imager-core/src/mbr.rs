//! Defensive DOS/MBR partition table parsing.

use serde::{Deserialize, Serialize};
use thiserror::Error;

const MBR_SIGNATURE: [u8; 2] = [0x55, 0xaa];
const GPT_SIGNATURE: &[u8; 8] = b"EFI PART";

/// A supported primary partition type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PartitionKind {
    /// FAT12/16/32 partition.
    Fat,
    /// Linux native partition (`0x83`).
    Linux,
    /// An otherwise valid primary partition type.
    Other(u8),
}

/// A non-empty primary MBR partition.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Partition {
    /// One-based partition number.
    pub number: u8,
    /// Raw MBR type byte.
    pub type_code: u8,
    /// Classified partition type.
    pub kind: PartitionKind,
    /// Inclusive first logical block address.
    pub start_lba: u64,
    /// Number of sectors.
    pub sectors: u64,
    /// Inclusive final logical block address.
    pub end_lba: u64,
    /// Whether the bootable flag is present.
    pub bootable: bool,
}

/// A validated MBR layout.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Mbr {
    /// Non-empty primary partitions in table order.
    pub partitions: Vec<Partition>,
}

/// Reasons an MBR layout is unsafe or unsupported.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum MbrError {
    /// Sector 0 lacks the DOS signature.
    #[error("invalid MBR signature")]
    InvalidSignature,
    /// A protective entry or GPT header was found.
    #[error("GPT partitioning is unsupported")]
    GptDetected,
    /// An extended partition would require logical partition handling.
    #[error("extended/logical partitions are unsupported")]
    ExtendedPartition,
    /// A partition has zero start LBA.
    #[error("partition starts at reserved LBA zero")]
    StartsAtZero,
    /// Partition end arithmetic overflowed.
    #[error("partition geometry overflow")]
    GeometryOverflow,
    /// A partition extends beyond the device.
    #[error("partition exceeds device size")]
    PartitionOutOfBounds,
    /// Two primary entries overlap.
    #[error("partitions overlap")]
    OverlappingPartitions,
}

/// Parse and validate a sector-zero MBR.
///
/// `lba1_prefix` should contain at least the first eight bytes of LBA 1 when
/// available. Passing it detects GPT even when a malformed protective entry is
/// absent.
///
/// # Errors
///
/// Returns [`MbrError`] when the table is malformed, outside the device, uses
/// GPT or extended partitions, or contains overlapping primary partitions.
pub fn parse_mbr(
    sector_zero: &[u8; 512],
    lba1_prefix: Option<&[u8]>,
    total_sectors: u64,
) -> Result<Mbr, MbrError> {
    if sector_zero[510..512] != MBR_SIGNATURE {
        return Err(MbrError::InvalidSignature);
    }
    if lba1_prefix.is_some_and(|prefix| prefix.starts_with(GPT_SIGNATURE)) {
        return Err(MbrError::GptDetected);
    }

    let mut partitions = Vec::with_capacity(4);
    for (number, index) in (1_u8..=4).zip(0..4) {
        let offset = 446 + index * 16;
        let entry = &sector_zero[offset..offset + 16];
        let type_code = entry[4];
        let mut start_bytes = [0_u8; 4];
        start_bytes.copy_from_slice(&entry[8..12]);
        let start = u64::from(u32::from_le_bytes(start_bytes));
        let mut sector_bytes = [0_u8; 4];
        sector_bytes.copy_from_slice(&entry[12..16]);
        let sectors = u64::from(u32::from_le_bytes(sector_bytes));
        if type_code == 0 && sectors == 0 {
            continue;
        }
        if type_code == 0xee {
            return Err(MbrError::GptDetected);
        }
        if matches!(type_code, 0x05 | 0x0f | 0x85) {
            return Err(MbrError::ExtendedPartition);
        }
        if start == 0 {
            return Err(MbrError::StartsAtZero);
        }
        let end_lba = start
            .checked_add(sectors)
            .and_then(|end| end.checked_sub(1))
            .ok_or(MbrError::GeometryOverflow)?;
        if sectors == 0 || end_lba >= total_sectors {
            return Err(MbrError::PartitionOutOfBounds);
        }
        partitions.push(Partition {
            number,
            type_code,
            kind: classify(type_code),
            start_lba: start,
            sectors,
            end_lba,
            bootable: entry[0] == 0x80,
        });
    }

    let mut by_start: Vec<_> = partitions.iter().collect();
    by_start.sort_unstable_by_key(|partition| partition.start_lba);
    if by_start
        .windows(2)
        .any(|pair| pair[0].end_lba >= pair[1].start_lba)
    {
        return Err(MbrError::OverlappingPartitions);
    }

    Ok(Mbr { partitions })
}

fn classify(type_code: u8) -> PartitionKind {
    match type_code {
        0x01 | 0x04 | 0x06 | 0x0b | 0x0c | 0x0e => PartitionKind::Fat,
        0x83 => PartitionKind::Linux,
        other => PartitionKind::Other(other),
    }
}
