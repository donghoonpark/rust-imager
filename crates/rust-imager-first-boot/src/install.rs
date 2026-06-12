//! Install first-boot expansion assets into an offline Linux root.

use std::fs;
use std::path::{Path, PathBuf};

use thiserror::Error;

const SCRIPT: &[u8] = include_bytes!("../assets/rust-imager-grow-root.sh");
const UNIT: &[u8] = include_bytes!("../assets/rust-imager-grow-root.service");

/// Asset installation error.
#[derive(Debug, Error)]
pub enum InstallError {
    /// The supplied offline root is not a directory.
    #[error("offline root is not a directory: {0}")]
    InvalidRoot(PathBuf),
    /// A filesystem operation failed.
    #[error("failed to install first-boot assets at {path}: {source}")]
    Io {
        /// Affected path.
        path: PathBuf,
        /// Operating system error.
        source: std::io::Error,
    },
}

/// Install and enable the idempotent grow-root service.
///
/// # Errors
///
/// Returns [`InstallError`] if the offline root is invalid or any asset,
/// directory, permission, or symlink operation fails.
pub fn install(root: &Path) -> Result<(), InstallError> {
    if !root.is_dir() {
        return Err(InstallError::InvalidRoot(root.to_path_buf()));
    }
    let script = root.join("usr/lib/rust-imager/grow-root.sh");
    let unit = root.join("etc/systemd/system/rust-imager-grow-root.service");
    let wants = root.join("etc/systemd/system/multi-user.target.wants");
    create_parent(&script)?;
    create_parent(&unit)?;
    fs::create_dir_all(&wants).map_err(|source| io_error(&wants, source))?;
    write_if_changed(&script, SCRIPT)?;
    write_if_changed(&unit, UNIT)?;
    set_executable(&script)?;

    let enabled = wants.join("rust-imager-grow-root.service");
    if enabled.symlink_metadata().is_ok() {
        fs::remove_file(&enabled).map_err(|source| io_error(&enabled, source))?;
    }
    std::os::unix::fs::symlink("../rust-imager-grow-root.service", &enabled)
        .map_err(|source| io_error(&enabled, source))?;
    Ok(())
}

fn create_parent(path: &Path) -> Result<(), InstallError> {
    let parent = path
        .parent()
        .ok_or_else(|| InstallError::InvalidRoot(path.to_path_buf()))?;
    fs::create_dir_all(parent).map_err(|source| io_error(parent, source))
}

fn write_if_changed(path: &Path, contents: &[u8]) -> Result<(), InstallError> {
    if fs::read(path).ok().as_deref() == Some(contents) {
        return Ok(());
    }
    fs::write(path, contents).map_err(|source| io_error(path, source))
}

fn set_executable(path: &Path) -> Result<(), InstallError> {
    use std::os::unix::fs::PermissionsExt;
    let mut permissions = fs::metadata(path)
        .map_err(|source| io_error(path, source))?
        .permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions).map_err(|source| io_error(path, source))
}

fn io_error(path: &Path, source: std::io::Error) -> InstallError {
    InstallError::Io {
        path: path.to_path_buf(),
        source,
    }
}
