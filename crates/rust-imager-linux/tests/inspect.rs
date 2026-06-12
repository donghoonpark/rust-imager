//! Raw layout and e2fsprogs parsing tests.

use std::fs;

use rust_imager_linux::inspect::{parse_ext4_geometry, read_mbr_from_disk};
use tempfile::tempdir;

#[test]
fn reads_sector_zero_and_detects_layout() {
    let dir = tempdir().expect("tempdir");
    let image = dir.path().join("disk.img");
    let mut bytes = vec![0_u8; 4096 * 512];
    bytes[510] = 0x55;
    bytes[511] = 0xaa;
    bytes[446 + 4] = 0x0c;
    bytes[446 + 8..446 + 12].copy_from_slice(&2048_u32.to_le_bytes());
    bytes[446 + 12..446 + 16].copy_from_slice(&1000_u32.to_le_bytes());
    fs::write(&image, bytes).expect("image");
    let mbr = read_mbr_from_disk(&image, 4096).expect("mbr");
    assert_eq!(mbr.partitions[0].start_lba, 2048);
}

#[test]
fn parses_ext4_block_geometry() {
    let geometry = parse_ext4_geometry(
        "Estimated minimum size of the filesystem: 1000",
        "Block count:              8000\nBlock size:               4096",
    )
    .expect("geometry");
    assert_eq!(geometry.minimum_bytes, 4_096_000);
    assert_eq!(geometry.current_bytes, 32_768_000);
}
