//! First-boot asset installation tests.

use std::fs;

use rust_imager_first_boot::install::{InstallError, install};
use tempfile::tempdir;

#[test]
fn installs_enabled_idempotent_service() {
    let root = tempdir().expect("temp root");
    install(root.path()).expect("first install");
    install(root.path()).expect("second install");

    let script = root.path().join("usr/lib/rust-imager/grow-root.sh");
    let unit = root
        .path()
        .join("etc/systemd/system/rust-imager-grow-root.service");
    let enabled = root
        .path()
        .join("etc/systemd/system/multi-user.target.wants/rust-imager-grow-root.service");
    assert!(script.exists());
    assert!(unit.exists());
    assert_eq!(
        fs::read_link(enabled).expect("enabled symlink"),
        std::path::Path::new("../rust-imager-grow-root.service")
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            fs::metadata(script).expect("metadata").permissions().mode() & 0o777,
            0o755
        );
    }
}

#[test]
fn refuses_non_directory_root() {
    let root = tempdir().expect("temp root");
    let file = root.path().join("image");
    fs::write(&file, b"x").expect("write file");
    assert!(matches!(install(&file), Err(InstallError::InvalidRoot(_))));
}
