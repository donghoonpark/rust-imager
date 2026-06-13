//! Whole-disk mount, swap, and holder checks before destructive work.

use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use thiserror::Error;

use crate::command::{CommandError, CommandSpec, Runner};

/// A target disk is still in use or could not be inspected safely.
#[derive(Debug, Error)]
pub enum QuiescenceError {
    /// A machine-readable utility failed.
    #[error(transparent)]
    Command(#[from] CommandError),
    /// `lsblk` returned invalid JSON.
    #[error("invalid lsblk JSON: {0}")]
    InvalidLsblk(#[from] serde_json::Error),
    /// A selected disk child is active swap.
    #[error("active swap prevents imaging: {0}")]
    ActiveSwap(String),
    /// A selected disk or child has kernel holders.
    #[error("{device} has kernel holders: {holders:?}")]
    Holders {
        /// Block device path.
        device: String,
        /// Holder block names.
        holders: Vec<String>,
    },
    /// A mount remained after the unmount attempt.
    #[error("mount remains after unmount attempt: {0}")]
    Mounted(String),
    /// sysfs holder inspection failed.
    #[error("failed to inspect holders for {device}: {source}")]
    HolderIo {
        /// Block name.
        device: String,
        /// Filesystem error.
        source: std::io::Error,
    },
}

#[derive(Debug, Deserialize)]
struct Topology {
    blockdevices: Vec<Node>,
}

#[derive(Debug, Deserialize)]
struct Node {
    path: String,
    name: String,
    #[serde(default)]
    mountpoints: Vec<Option<String>>,
    #[serde(default)]
    children: Vec<Node>,
}

/// Unmount every selected-disk filesystem and reject swap or holders.
///
/// `holders` is injectable so topology policy can be tested without host sysfs.
///
/// # Errors
///
/// Returns [`QuiescenceError`] when topology cannot be read or any selected
/// block device remains in use.
pub fn quiesce_disk(
    runner: &dyn Runner,
    disk: &str,
    holders: impl Fn(&str) -> Result<Vec<String>, std::io::Error>,
) -> Result<(), QuiescenceError> {
    let topology = read_topology(runner, disk)?;
    let mut nodes = Vec::new();
    for root in &topology.blockdevices {
        flatten(root, &mut nodes);
    }
    let paths: Vec<_> = nodes.iter().map(|node| node.path.as_str()).collect();

    let swaps = runner.run(&spec("swapon", &["--show=NAME", "--noheadings", "--raw"]))?;
    for swap in swaps
        .stdout
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
    {
        if paths.contains(&swap) {
            return Err(QuiescenceError::ActiveSwap(swap.to_owned()));
        }
    }
    for node in &nodes {
        let active = holders(&node.name).map_err(|source| QuiescenceError::HolderIo {
            device: node.name.clone(),
            source,
        })?;
        if !active.is_empty() {
            return Err(QuiescenceError::Holders {
                device: node.path.clone(),
                holders: active,
            });
        }
    }

    let mut mounts: Vec<_> = nodes
        .iter()
        .flat_map(|node| node.mountpoints.iter().flatten().cloned())
        .collect();
    mounts.sort_unstable_by(|left, right| {
        path_depth(right)
            .cmp(&path_depth(left))
            .then_with(|| left.cmp(right))
    });
    mounts.dedup();
    for mount in mounts {
        runner.run(&spec("umount", &[&mount]).destructive())?;
    }

    let remaining = read_topology(runner, disk)?;
    let mut remaining_nodes = Vec::new();
    for root in &remaining.blockdevices {
        flatten(root, &mut remaining_nodes);
    }
    if let Some(mount) = remaining_nodes
        .iter()
        .flat_map(|node| node.mountpoints.iter().flatten())
        .next()
    {
        return Err(QuiescenceError::Mounted(mount.clone()));
    }
    Ok(())
}

/// Read holder names from Linux sysfs.
///
/// # Errors
///
/// Returns an I/O error when the block holder directory cannot be inspected.
pub fn sysfs_holders(name: &str) -> Result<Vec<String>, std::io::Error> {
    let directory = PathBuf::from("/sys/class/block").join(name).join("holders");
    let mut holders = Vec::new();
    for entry in fs::read_dir(directory)? {
        holders.push(entry?.file_name().to_string_lossy().into_owned());
    }
    holders.sort_unstable();
    Ok(holders)
}

fn read_topology(runner: &dyn Runner, disk: &str) -> Result<Topology, QuiescenceError> {
    let output = runner.run(&spec(
        "lsblk",
        &["--json", "--output", "PATH,NAME,MOUNTPOINTS", disk],
    ))?;
    serde_json::from_str(&output.stdout).map_err(QuiescenceError::from)
}

fn flatten<'a>(node: &'a Node, output: &mut Vec<&'a Node>) {
    output.push(node);
    for child in &node.children {
        flatten(child, output);
    }
}

fn path_depth(path: &str) -> usize {
    Path::new(path).components().count()
}

fn spec(program: &str, args: &[&str]) -> CommandSpec {
    CommandSpec::new(program, args.iter().map(OsString::from))
}
