//! Whole-disk quiescence tests.

use std::ffi::OsString;
use std::sync::Mutex;

use rust_imager_linux::command::{CommandError, CommandResult, CommandSpec, Runner};
use rust_imager_linux::quiescence::{QuiescenceError, quiesce_disk};

struct FakeRunner {
    calls: Mutex<Vec<CommandSpec>>,
    lsblk: Mutex<Vec<String>>,
    swaps: String,
}

impl Runner for FakeRunner {
    fn run(&self, spec: &CommandSpec) -> Result<CommandResult, CommandError> {
        self.calls.lock().expect("lock").push(spec.clone());
        let stdout = match spec.program.to_string_lossy().as_ref() {
            "lsblk" => self.lsblk.lock().expect("lsblk lock").remove(0),
            "swapon" => self.swaps.clone(),
            _ => String::new(),
        };
        Ok(CommandResult {
            status: 0,
            stdout,
            stderr: String::new(),
        })
    }
}

fn topology() -> String {
    r#"{
      "blockdevices": [{
        "name": "/dev/sda",
        "mountpoint": null,
        "children": [
          {"name":"/dev/sda1","mountpoint":"/mnt/boot"},
          {"name":"/dev/sda2","mountpoint":"/mnt/root/sub"}
        ]
      }]
    }"#
    .into()
}

fn unmounted_topology() -> String {
    topology()
        .replace(r#""/mnt/boot""#, "null")
        .replace(r#""/mnt/root/sub""#, "null")
}

#[test]
fn unmounts_every_child_deepest_first() {
    let runner = FakeRunner {
        calls: Mutex::new(Vec::new()),
        lsblk: Mutex::new(vec![topology(), unmounted_topology()]),
        swaps: String::new(),
    };
    quiesce_disk(&runner, "/dev/sda", |_| Ok(Vec::new())).expect("quiescent disk");

    let calls = runner.calls.lock().expect("lock");
    let unmounts: Vec<_> = calls
        .iter()
        .filter(|call| call.program == "umount")
        .map(|call| call.args.clone())
        .collect();
    assert_eq!(
        unmounts,
        vec![
            vec![OsString::from("/mnt/root/sub")],
            vec![OsString::from("/mnt/boot")],
        ]
    );
    assert_eq!(
        calls[0].args,
        vec![
            OsString::from("--json"),
            OsString::from("--paths"),
            OsString::from("--output"),
            OsString::from("NAME,MOUNTPOINT"),
            OsString::from("/dev/sda"),
        ]
    );
}

#[test]
fn rejects_swap_on_any_child() {
    let runner = FakeRunner {
        calls: Mutex::new(Vec::new()),
        lsblk: Mutex::new(vec![topology()]),
        swaps: "/dev/sda2\n".into(),
    };
    assert!(matches!(
        quiesce_disk(&runner, "/dev/sda", |_| Ok(Vec::new())),
        Err(QuiescenceError::ActiveSwap(path)) if path == "/dev/sda2"
    ));
}

#[test]
fn rejects_kernel_holders() {
    let runner = FakeRunner {
        calls: Mutex::new(Vec::new()),
        lsblk: Mutex::new(vec![topology()]),
        swaps: String::new(),
    };
    assert!(matches!(
        quiesce_disk(&runner, "/dev/sda", |name| {
            Ok(if name == "sda2" {
                vec!["dm-0".into()]
            } else {
                Vec::new()
            })
        }),
        Err(QuiescenceError::Holders { device, holders })
            if device == "/dev/sda2" && holders == vec!["dm-0"]
    ));
}
