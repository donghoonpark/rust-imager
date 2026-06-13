//! Shell-free process execution.

use std::ffi::OsString;
use std::io::Write;
use std::process::{Command, Stdio};
use std::time::Duration;

#[cfg(unix)]
use std::os::unix::process::ExitStatusExt;
use thiserror::Error;

/// A fully specified external command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandSpec {
    /// Executable name or path.
    pub program: OsString,
    /// Exact argument vector.
    pub args: Vec<OsString>,
    /// Optional bytes supplied to standard input.
    pub stdin: Option<Vec<u8>>,
    /// Exit statuses considered successful.
    pub accepted_statuses: Vec<i32>,
    /// Maximum wall-clock runtime.
    pub timeout: Duration,
}

impl CommandSpec {
    /// Construct a command specification without standard input.
    pub fn new(program: impl Into<OsString>, args: impl IntoIterator<Item = OsString>) -> Self {
        Self {
            program: program.into(),
            args: args.into_iter().collect(),
            stdin: None,
            accepted_statuses: vec![0],
            timeout: Duration::from_secs(120),
        }
    }

    /// Attach exact standard input bytes.
    #[must_use]
    pub fn with_stdin(mut self, stdin: impl Into<Vec<u8>>) -> Self {
        self.stdin = Some(stdin.into());
        self
    }

    /// Replace the accepted exit-status set.
    #[must_use]
    pub fn accepting(mut self, statuses: impl IntoIterator<Item = i32>) -> Self {
        self.accepted_statuses = statuses.into_iter().collect();
        self
    }

    /// Set a finite wall-clock timeout.
    #[must_use]
    pub const fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Use the long timeout reserved for destructive storage operations.
    #[must_use]
    pub const fn destructive(self) -> Self {
        self.with_timeout(Duration::from_secs(6 * 60 * 60))
    }
}

/// Captured successful command output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandResult {
    /// Numeric exit status.
    pub status: i32,
    /// UTF-8-lossy standard output.
    pub stdout: String,
    /// UTF-8-lossy standard error.
    pub stderr: String,
}

/// Process execution failure.
#[derive(Debug, Error)]
pub enum CommandError {
    /// The process could not be started or communicated with.
    #[error("failed to execute {program:?}: {source}")]
    Io {
        /// Program that failed.
        program: OsString,
        /// Operating system error.
        source: std::io::Error,
    },
    /// The process exited unsuccessfully.
    #[error("command {program:?} failed with status {status}: {stderr}")]
    Failed {
        /// Program that failed.
        program: OsString,
        /// Numeric exit status, or -1 for signal termination.
        status: i32,
        /// Lossy stderr text.
        stderr: String,
    },
    /// GNU `timeout` stopped a command after its deadline.
    #[error("command {program:?} timed out after {timeout:?}")]
    TimedOut {
        /// Program that exceeded its deadline.
        program: OsString,
        /// Configured timeout.
        timeout: Duration,
    },
    /// A child command terminated because of a Unix signal.
    #[error("command {program:?} terminated by signal {signal}")]
    TerminatedBySignal {
        /// Program that was terminated.
        program: OsString,
        /// Unix signal number.
        signal: i32,
    },
}

/// Replaceable command executor.
pub trait Runner: Send + Sync {
    /// Execute one exact command.
    ///
    /// # Errors
    ///
    /// Returns [`CommandError`] for process or exit-status failures.
    fn run(&self, spec: &CommandSpec) -> Result<CommandResult, CommandError>;
}

/// Real operating-system command runner.
#[derive(Debug, Default, Clone, Copy)]
pub struct ProcessRunner;

impl Runner for ProcessRunner {
    fn run(&self, spec: &CommandSpec) -> Result<CommandResult, CommandError> {
        #[cfg(target_os = "linux")]
        let mut command = {
            let mut command = Command::new("setsid");
            command.args([
                OsString::from("timeout"),
                OsString::from("--signal=TERM"),
                OsString::from("--kill-after=10s"),
                OsString::from(gnu_timeout_duration(spec.timeout)),
                spec.program.clone(),
            ]);
            command.args(&spec.args);
            command
        };
        #[cfg(not(target_os = "linux"))]
        let mut command = {
            let mut command = Command::new(&spec.program);
            command.args(&spec.args);
            command
        };
        let mut child = command
            .env("LC_ALL", "C")
            .stdin(if spec.stdin.is_some() {
                Stdio::piped()
            } else {
                Stdio::null()
            })
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|source| CommandError::Io {
                program: spec.program.clone(),
                source,
            })?;
        if let Some(input) = &spec.stdin {
            child
                .stdin
                .take()
                .ok_or_else(|| CommandError::Io {
                    program: spec.program.clone(),
                    source: std::io::Error::other("stdin pipe unavailable"),
                })?
                .write_all(input)
                .map_err(|source| CommandError::Io {
                    program: spec.program.clone(),
                    source,
                })?;
        }
        let output = child
            .wait_with_output()
            .map_err(|source| CommandError::Io {
                program: spec.program.clone(),
                source,
            })?;
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_owned();
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
        #[cfg(unix)]
        if let Some(signal) = output.status.signal() {
            return Err(CommandError::TerminatedBySignal {
                program: spec.program.clone(),
                signal,
            });
        }
        let status = output.status.code().unwrap_or(-1);
        if status == 124 || status == 137 {
            Err(CommandError::TimedOut {
                program: spec.program.clone(),
                timeout: spec.timeout,
            })
        } else if status > 128 {
            Err(CommandError::TerminatedBySignal {
                program: spec.program.clone(),
                signal: status - 128,
            })
        } else if spec.accepted_statuses.contains(&status) {
            Ok(CommandResult {
                status,
                stdout,
                stderr,
            })
        } else {
            Err(CommandError::Failed {
                program: spec.program.clone(),
                status,
                stderr,
            })
        }
    }
}

#[cfg(target_os = "linux")]
fn gnu_timeout_duration(duration: Duration) -> String {
    let milliseconds = duration.as_millis().max(1);
    let seconds = milliseconds / 1_000;
    let fractional = milliseconds % 1_000;
    format!("{seconds}.{fractional:03}s")
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::gnu_timeout_duration;
    use std::time::Duration;

    #[test]
    fn formats_gnu_timeout_fractional_seconds() {
        assert_eq!(gnu_timeout_duration(Duration::from_millis(50)), "0.050s");
        assert_eq!(gnu_timeout_duration(Duration::from_millis(1_250)), "1.250s");
        assert_eq!(gnu_timeout_duration(Duration::ZERO), "0.001s");
    }
}
