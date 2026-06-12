//! Typed adapters for Linux storage utilities.

use std::ffi::OsString;

use crate::command::{CommandError, CommandResult, CommandSpec, Runner};

/// ext4 filesystem utilities.
pub struct E2fsTools<'a> {
    runner: &'a dyn Runner,
}

impl<'a> E2fsTools<'a> {
    /// Construct an adapter.
    #[must_use]
    pub fn new(runner: &'a dyn Runner) -> Self {
        Self { runner }
    }

    /// Check and optionally repair an ext filesystem.
    ///
    /// # Errors
    ///
    /// Returns [`CommandError`] when `e2fsck` fails.
    pub fn check(&self, device: &str, force_repair: bool) -> Result<CommandResult, CommandError> {
        let mode = if force_repair { "-y" } else { "-p" };
        self.runner
            .run(&spec("e2fsck", &["-f", mode, device]).accepting([0, 1, 2]))
    }

    /// Resize ext4 to an explicit KiB target.
    ///
    /// # Errors
    ///
    /// Returns [`CommandError`] when `resize2fs` fails.
    pub fn resize(&self, device: &str, kibibytes: u64) -> Result<CommandResult, CommandError> {
        self.runner
            .run(&spec("resize2fs", &[device, &format!("{kibibytes}K")]))
    }
}

/// DOS partition table utilities.
pub struct PartitionTools<'a> {
    runner: &'a dyn Runner,
}

impl<'a> PartitionTools<'a> {
    /// Construct an adapter.
    #[must_use]
    pub fn new(runner: &'a dyn Runner) -> Self {
        Self { runner }
    }

    /// Rewrite one primary partition while preserving its start.
    ///
    /// # Errors
    ///
    /// Returns [`CommandError`] for invalid geometry or `sfdisk` failure.
    pub fn resize_primary(
        &self,
        disk: &str,
        number: u8,
        start_lba: u64,
        end_lba: u64,
    ) -> Result<CommandResult, CommandError> {
        let size = end_lba
            .checked_sub(start_lba)
            .and_then(|value| value.checked_add(1))
            .ok_or_else(|| CommandError::Io {
                program: OsString::from("sfdisk"),
                source: std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "partition end precedes start",
                ),
            })?;
        self.runner.run(
            &spec(
                "sfdisk",
                &["--no-reread", "--force", "-N", &number.to_string(), disk],
            )
            .with_stdin(format!("start={start_lba}, size={size}\n")),
        )
    }

    /// Ask the kernel to reread partition geometry.
    ///
    /// # Errors
    ///
    /// Returns [`CommandError`] when `partprobe` fails.
    pub fn reread(&self, disk: &str) -> Result<CommandResult, CommandError> {
        self.runner.run(&spec("partprobe", &[disk]))
    }
}

/// Mount utilities.
pub struct MountTools<'a> {
    runner: &'a dyn Runner,
}

impl<'a> MountTools<'a> {
    /// Construct an adapter.
    #[must_use]
    pub fn new(runner: &'a dyn Runner) -> Self {
        Self { runner }
    }

    /// Mount a filesystem read/write.
    ///
    /// # Errors
    ///
    /// Returns [`CommandError`] when mount fails.
    pub fn mount_read_write(
        &self,
        device: &str,
        target: &str,
    ) -> Result<CommandResult, CommandError> {
        self.runner
            .run(&spec("mount", &["-o", "rw", device, target]))
    }

    /// Unmount a filesystem.
    ///
    /// # Errors
    ///
    /// Returns [`CommandError`] when unmount fails.
    pub fn unmount(&self, device: &str) -> Result<CommandResult, CommandError> {
        self.runner.run(&spec("umount", &[device]))
    }
}

fn spec(program: &str, args: &[&str]) -> CommandSpec {
    CommandSpec::new(program, args.iter().map(OsString::from))
}
