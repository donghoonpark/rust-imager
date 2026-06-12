//! SBC layout profile tests.

use rust_imager_core::filesystem::{FilesystemEvidence, PartitionEvidence};
use rust_imager_core::mbr::{Mbr, Partition, PartitionKind};
use rust_imager_core::profile::{LayoutClass, Profile, classify_layout};

fn partition(number: u8, kind: PartitionKind, start: u64, end: u64) -> Partition {
    Partition {
        number,
        type_code: match kind {
            PartitionKind::Fat => 0x0c,
            PartitionKind::Linux => 0x83,
            PartitionKind::Other(code) => code,
        },
        kind,
        start_lba: start,
        sectors: end - start + 1,
        end_lba: end,
        bootable: number == 1,
    }
}

fn evidence(boot_files: &[&str]) -> Vec<PartitionEvidence> {
    vec![
        PartitionEvidence {
            number: 1,
            filesystem: FilesystemEvidence::Fat,
            root_directories: vec![],
            boot_files: boot_files.iter().map(ToString::to_string).collect(),
        },
        PartitionEvidence {
            number: 2,
            filesystem: FilesystemEvidence::Ext4,
            root_directories: vec!["etc".into(), "usr".into(), "var".into()],
            boot_files: vec![],
        },
    ]
}

#[test]
fn recognizes_raspberry_pi_and_odroid() {
    let mbr = Mbr {
        partitions: vec![
            partition(1, PartitionKind::Fat, 2048, 4095),
            partition(2, PartitionKind::Linux, 4096, 99_999),
        ],
    };
    assert_eq!(
        classify_layout(&mbr, &evidence(&["config.txt", "start4.elf"])),
        LayoutClass::Recognized(Profile::RaspberryPi)
    );
    assert_eq!(
        classify_layout(&mbr, &evidence(&["boot.ini", "Image"])),
        LayoutClass::Recognized(Profile::Odroid)
    );
}

#[test]
fn compatible_unknown_layout_requires_warning() {
    let mbr = Mbr {
        partitions: vec![
            partition(1, PartitionKind::Fat, 2048, 4095),
            partition(2, PartitionKind::Linux, 4096, 99_999),
        ],
    };
    assert_eq!(
        classify_layout(&mbr, &evidence(&["Image"])),
        LayoutClass::WarningUnknown
    );
}

#[test]
fn rejects_non_ext4_last_partition() {
    let mbr = Mbr {
        partitions: vec![
            partition(1, PartitionKind::Fat, 2048, 4095),
            partition(2, PartitionKind::Other(0x07), 4096, 99_999),
        ],
    };
    assert!(matches!(
        classify_layout(&mbr, &evidence(&["Image"])),
        LayoutClass::Unsupported(_)
    ));
}
