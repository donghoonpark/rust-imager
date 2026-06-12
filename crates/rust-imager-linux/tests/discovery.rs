//! Device discovery tests.

use rust_imager_linux::discovery::{DiscoveryError, discover_candidates, revalidate_identity};

const LSBLK: &str = r#"{
  "blockdevices": [
    {"name":"nvme0n1","path":"/dev/nvme0n1","type":"disk","size":1000000,"tran":"nvme","model":"SYSTEM","serial":"SYS","maj:min":"1:0","rm":false,
     "children":[{"name":"nvme0n1p1","path":"/dev/nvme0n1p1","type":"part","size":900000,"mountpoints":["/"],"maj:min":"1:1"}]},
    {"name":"sda","path":"/dev/sda","type":"disk","size":64000000,"tran":"usb","model":"eMMC Reader","serial":"ABC","maj:min":"8:0","rm":true,
     "children":[{"name":"sda1","path":"/dev/sda1","type":"part","size":100000,"mountpoints":[null],"maj:min":"8:1"}]},
    {"name":"sdb","path":"/dev/sdb","type":"disk","size":64000000,"tran":"usb","model":"Output Disk","serial":"OUT","maj:min":"8:16","rm":true,
     "children":[{"name":"sdb1","path":"/dev/sdb1","type":"part","size":63000000,"mountpoints":["/data"],"maj:min":"8:17"}]},
    {"name":"sdc","path":"/dev/sdc","type":"disk","size":64000000,"tran":"sata","model":"Internal","serial":"INT","maj:min":"8:32","rm":false}
  ]
}"#;

const FINDMNT: &str = r#"{
  "filesystems": [
    {"target":"/","source":"/dev/nvme0n1p1"},
    {"target":"/data","source":"/dev/sdb1"}
  ]
}"#;

#[test]
fn returns_only_unused_usb_sd_disks() {
    let candidates =
        discover_candidates(LSBLK, FINDMNT, Some("/data/image.img")).expect("valid fixture");
    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].path, "/dev/sda");
    assert_eq!(candidates[0].model, "eMMC Reader");
}

#[test]
fn identity_change_is_rejected() {
    let selected = discover_candidates(LSBLK, FINDMNT, None)
        .expect("valid fixture")
        .into_iter()
        .find(|device| device.path == "/dev/sda")
        .expect("sda");
    let mut changed = selected.clone();
    changed.serial = "DIFFERENT".into();
    assert_eq!(
        revalidate_identity(&selected, &changed),
        Err(DiscoveryError::IdentityChanged)
    );
}

#[test]
fn invalid_json_is_reported() {
    assert!(matches!(
        discover_candidates("{", FINDMNT, None),
        Err(DiscoveryError::InvalidLsblk(_))
    ));
}
