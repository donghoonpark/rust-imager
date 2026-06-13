//! Real command timeout and termination classification tests.

use std::time::Duration;

use rust_imager_linux::command::CommandSpec;

#[cfg(target_os = "linux")]
use rust_imager_linux::command::{CommandError, ProcessRunner, Runner};
#[cfg(target_os = "linux")]
use std::ffi::OsString;

#[test]
fn records_explicit_timeout() {
    let spec = CommandSpec::new("true", []).with_timeout(Duration::from_secs(7));
    assert_eq!(spec.timeout, Duration::from_secs(7));
}

#[cfg(target_os = "linux")]
#[test]
fn reports_command_timeout() {
    let error = ProcessRunner
        .run(
            &CommandSpec::new("sleep", [OsString::from("5")])
                .with_timeout(Duration::from_millis(50)),
        )
        .expect_err("sleep must time out");
    assert!(matches!(error, CommandError::TimedOut { .. }));
}

#[cfg(target_os = "linux")]
#[test]
fn reports_signal_termination() {
    let error = ProcessRunner
        .run(&CommandSpec::new(
            "sh",
            [OsString::from("-c"), OsString::from("kill -TERM $$")],
        ))
        .expect_err("child must terminate by signal");
    assert!(matches!(
        error,
        CommandError::TerminatedBySignal { signal: 15, .. }
    ));
}
