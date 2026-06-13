//! Privileged whole-disk quiescence validation.

use std::env;

use rust_imager_linux::command::ProcessRunner;
use rust_imager_linux::quiescence::{quiesce_disk, sysfs_holders};

#[test]
#[ignore = "requires root and a mounted Linux loop disk"]
fn unmounts_all_real_loop_partitions() {
    let disk = env::var("RUST_IMAGER_QUIESCENCE_DISK").expect("quiescence disk");
    quiesce_disk(&ProcessRunner, &disk, sysfs_holders).expect("quiesce loop disk");
}
