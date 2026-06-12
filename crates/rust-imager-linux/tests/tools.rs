//! Typed storage tool tests.

use std::ffi::OsString;
use std::sync::Mutex;

use rust_imager_linux::command::{CommandError, CommandResult, CommandSpec, Runner};
use rust_imager_linux::tools::{E2fsTools, MountTools, PartitionTools};

#[derive(Default)]
struct FakeRunner {
    calls: Mutex<Vec<CommandSpec>>,
}

impl Runner for FakeRunner {
    fn run(&self, spec: &CommandSpec) -> Result<CommandResult, CommandError> {
        self.calls.lock().expect("lock").push(spec.clone());
        Ok(CommandResult {
            status: 0,
            stdout: String::new(),
            stderr: String::new(),
        })
    }
}

#[test]
fn emits_exact_filesystem_and_partition_commands() {
    let runner = FakeRunner::default();
    E2fsTools::new(&runner)
        .check("/dev/sda2", false)
        .expect("fsck");
    E2fsTools::new(&runner)
        .resize("/dev/sda2", 123_456)
        .expect("resize");
    PartitionTools::new(&runner)
        .resize_primary("/dev/sda", 2, 4096, 999_999)
        .expect("sfdisk");

    let calls = runner.calls.lock().expect("lock");
    assert_eq!(calls[0].program, OsString::from("e2fsck"));
    assert_eq!(calls[0].args, vec!["-f", "-p", "/dev/sda2"]);
    assert_eq!(
        calls[1].args,
        vec!["/dev/sda2", "123456K"],
        "resize unit is explicit"
    );
    assert_eq!(
        calls[2].args,
        vec!["--no-reread", "--force", "-N", "2", "/dev/sda"]
    );
    assert_eq!(
        calls[2].stdin.as_deref(),
        Some("start=4096, size=995904\n".as_bytes())
    );
}

#[test]
fn mount_commands_are_shell_free() {
    let runner = FakeRunner::default();
    MountTools::new(&runner)
        .mount_read_write("/dev/sda2", "/run/rust-imager/root")
        .expect("mount");
    MountTools::new(&runner)
        .unmount("/dev/sda2")
        .expect("unmount");
    let calls = runner.calls.lock().expect("lock");
    assert_eq!(
        calls[0].args,
        vec!["-o", "rw", "/dev/sda2", "/run/rust-imager/root"]
    );
    assert_eq!(calls[1].args, vec!["/dev/sda2"]);
}
