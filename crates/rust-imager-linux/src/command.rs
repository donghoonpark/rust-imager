//! Shell-free process execution.

use std::ffi::OsString;
use std::process::{Command, Output};

use thiserror::Error;

/// A fully specified external command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandSpec {
    /// Executable name or path.
    pub program: OsString,
    /// Exact argument vector.
    pub args: Vec<OsString>,
}

impl CommandSpec {
    /// Construct a command specification.
    pub fn new(program: impl Into<OsString>, args: impl IntoIterator<Item = OsString>) -> Self {
        Self {
            program: program.into(),
            args: args.into_iter().collect(),
        }
    }
}

/// Process execution failure.
#[derive(Debug, Error)]
pub enum CommandError {
    /// The process could not be started.
    #[error("failed to execute {program:?}: {source}")]
    Spawn {
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

/// Execute a command directly, with a deterministic locale.
///
/// # Errors
///
/// Returns [`CommandError`] when spawning fails or the process exits with a
/// non-zero status.
pub fn run(spec: &CommandSpec) -> Result<Output, CommandError> {
    let output = Command::new(&spec.program)
        .args(&spec.args)
        .env("LC_ALL", "C")
        .output()
        .map_err(|source| CommandError::Spawn {
            program: spec.program.clone(),
            source,
        })?;
    if output.status.success() {
        Ok(output)
    } else {
        Err(CommandError::Failed {
            program: spec.program.clone(),
            status: output.status.code().unwrap_or(-1),
            stderr: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        })
    }
}
