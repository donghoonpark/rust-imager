//! Shell-free process execution.

use std::ffi::OsString;
use std::io::Write;
use std::process::{Command, Stdio};

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
}

impl CommandSpec {
    /// Construct a command specification without standard input.
    pub fn new(program: impl Into<OsString>, args: impl IntoIterator<Item = OsString>) -> Self {
        Self {
            program: program.into(),
            args: args.into_iter().collect(),
            stdin: None,
            accepted_statuses: vec![0],
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
        let mut child = Command::new(&spec.program)
            .args(&spec.args)
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
        let status = output.status.code().unwrap_or(-1);
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_owned();
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
        if spec.accepted_statuses.contains(&status) {
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
