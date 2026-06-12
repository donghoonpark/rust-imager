//! MBR parser tests.

use proptest::prelude::*;
use rust_imager_core::mbr::{MbrError, PartitionKind, parse_mbr};

fn sector(entries: &[(u8, u32, u32)]) -> [u8; 512] {
    let mut bytes = [0_u8; 512];
    for (index, (kind, start, sectors)) in entries.iter().enumerate() {
        let offset = 446 + index * 16;
        bytes[offset + 4] = *kind;
        bytes[offset + 8..offset + 12].copy_from_slice(&start.to_le_bytes());
        bytes[offset + 12..offset + 16].copy_from_slice(&sectors.to_le_bytes());
    }
    bytes[510] = 0x55;
    bytes[511] = 0xaa;
    bytes
}

#[test]
fn parses_non_overlapping_primary_partitions() {
    let parsed = parse_mbr(
        &sector(&[(0x0c, 2048, 1024), (0x83, 4096, 8192)]),
        None,
        20_000,
    )
    .expect("valid MBR");
    assert_eq!(parsed.partitions.len(), 2);
    assert_eq!(parsed.partitions[1].kind, PartitionKind::Linux);
    assert_eq!(parsed.partitions[1].end_lba, 12_287);
}

#[test]
fn rejects_bad_signature() {
    let mut bytes = sector(&[]);
    bytes[511] = 0;
    assert_eq!(
        parse_mbr(&bytes, None, 10_000),
        Err(MbrError::InvalidSignature)
    );
}

#[test]
fn rejects_protective_mbr_and_gpt_header() {
    assert_eq!(
        parse_mbr(&sector(&[(0xee, 1, 999)]), None, 1_000),
        Err(MbrError::GptDetected)
    );
    assert_eq!(
        parse_mbr(&sector(&[]), Some(b"EFI PART"), 1_000),
        Err(MbrError::GptDetected)
    );
}

#[test]
fn rejects_extended_and_overlapping_partitions() {
    assert_eq!(
        parse_mbr(&sector(&[(0x0f, 2048, 100)]), None, 10_000),
        Err(MbrError::ExtendedPartition)
    );
    assert_eq!(
        parse_mbr(
            &sector(&[(0x0c, 2048, 4096), (0x83, 4096, 4096)]),
            None,
            10_000
        ),
        Err(MbrError::OverlappingPartitions)
    );
}

#[test]
fn rejects_partition_outside_device() {
    assert_eq!(
        parse_mbr(&sector(&[(0x83, 900, 200)]), None, 1_000),
        Err(MbrError::PartitionOutOfBounds)
    );
}

proptest! {
    #[test]
    fn arbitrary_sectors_never_panic(bytes in any::<[u8; 512]>(), total in 1_u64..u64::from(u32::MAX)) {
        let _ = parse_mbr(&bytes, None, total);
    }
}
