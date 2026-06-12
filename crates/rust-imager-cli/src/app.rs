//! End-to-end imaging application engine.

use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail, ensure};
use nix::unistd::Uid;
use rust_imager_core::device::DeviceIdentity;
use rust_imager_core::metadata::ImageMetadata;
use rust_imager_core::plan::ExecutionPlan;
use rust_imager_core::plan::{Compression, PlanInput, VerificationLevel, build_plan};
use rust_imager_core::profile::{LayoutClass, classify_layout};
use rust_imager_linux::command::{CommandSpec, ProcessRunner, Runner};
use rust_imager_linux::discovery::{discover_candidates, revalidate_identity};
use rust_imager_linux::inspect::{
    ext4_geometry, inspect_filesystems, partition_path, read_mbr_from_disk,
};
use rust_imager_linux::shrink::{LinuxShrinkBackend, ShrinkRequest, execute_shrink};
use rust_imager_pipeline::extract::{ExtractOptions, extract_path};
use rust_imager_pipeline::verify::{VerifyRequest, verify_image, write_sidecar};

/// Fully confirmed imaging request.
#[derive(Debug, Clone)]
pub struct ImageRequest {
    /// Selected whole disk.
    pub device: String,
    /// Local compressed image path.
    pub output: PathBuf,
    /// Exact device model confirmation.
    pub confirm_model: String,
    /// Compression policy.
    pub compression: Compression,
    /// Verification policy.
    pub verification: VerificationLevel,
}

struct Prepared {
    selected: DeviceIdentity,
    plan: ExecutionPlan,
    root_number: u8,
    root_start_lba: u64,
    root_path: String,
    total_sectors: u64,
}

/// Refuse unsupported platforms and non-root execution.
pub fn require_linux_root() -> Result<()> {
    ensure!(cfg!(target_os = "linux"), "rust-imager only runs on Linux");
    ensure!(
        Uid::effective().is_root(),
        "run rust-imager with sudo/root privileges"
    );
    Ok(())
}

/// Discover candidates from current Linux topology.
pub fn discover(output: Option<&Path>) -> Result<Vec<DeviceIdentity>> {
    let runner = ProcessRunner;
    let lsblk = run_text(
        &runner,
        "lsblk",
        &[
            "--json",
            "--bytes",
            "--output",
            "NAME,PATH,TYPE,SIZE,LOG-SEC,TRAN,MODEL,SERIAL,MAJ:MIN,RM,MOUNTPOINTS",
        ],
    )?;
    let findmnt = run_text(
        &runner,
        "findmnt",
        &["--json", "--real", "--output", "TARGET,SOURCE"],
    )?;
    discover_candidates(&lsblk, &findmnt, output.and_then(Path::to_str)).map_err(Into::into)
}

/// Analyze, shrink, extract, verify, and write metadata.
pub fn run_image(request: &ImageRequest) -> Result<()> {
    let prepared = prepare(request)?;
    execute(request, prepared)
}

fn prepare(request: &ImageRequest) -> Result<Prepared> {
    let candidates = discover(Some(&request.output))?;
    let selected = candidates
        .iter()
        .find(|device| device.path == request.device)
        .cloned()
        .context("selected device is not currently a safe USB /dev/sdX candidate")?;
    ensure!(
        selected.model == request.confirm_model.trim(),
        "device model confirmation does not match"
    );
    let runner = ProcessRunner;
    let total_sectors = selected
        .size_bytes
        .checked_div(selected.logical_sector_size)
        .context("invalid device sector size")?;
    let mbr = read_mbr_from_disk(Path::new(&selected.path), total_sectors)?;
    let mount_base = PathBuf::from(format!("/run/rust-imager/inspect-{}", std::process::id()));
    let evidence = inspect_filesystems(&runner, &selected.path, &mbr, &mount_base)?;
    let _ = fs::remove_dir_all(&mount_base);
    match classify_layout(&mbr, &evidence) {
        LayoutClass::Unsupported(reason) => bail!("unsupported layout: {reason}"),
        LayoutClass::Recognized(_) | LayoutClass::WarningUnknown => {}
    }
    let root = mbr
        .partitions
        .iter()
        .max_by_key(|partition| partition.end_lba)
        .context("no root partition")?;
    let root_path = partition_path(&selected.path, root.number);
    let ext4 = ext4_geometry(&runner, &root_path)?;

    let output_parent = request
        .output
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(output_parent)?;
    ensure_local_output(&runner, output_parent)?;
    let plan = build_plan(PlanInput {
        device_path: selected.path.clone(),
        device_size_bytes: selected.size_bytes,
        sector_size: selected.logical_sector_size,
        root_start_lba: root.start_lba,
        ext4_minimum_bytes: ext4.minimum_bytes,
        ext4_current_bytes: ext4.current_bytes,
        output_path: request.output.to_string_lossy().into_owned(),
        output_available_bytes: available_bytes(&runner, output_parent)?,
        output_is_local: true,
        compression: request.compression,
        verification: request.verification,
        alignment_bytes: 1024 * 1024,
    })?;

    let current = discover(Some(&request.output))?
        .into_iter()
        .find(|device| device.path == selected.path)
        .context("device disappeared before mutation")?;
    revalidate_identity(&selected, &current)?;

    Ok(Prepared {
        selected,
        plan,
        root_number: root.number,
        root_start_lba: root.start_lba,
        root_path,
        total_sectors,
    })
}

fn execute(request: &ImageRequest, prepared: Prepared) -> Result<()> {
    let runner = ProcessRunner;
    let current = discover(Some(&request.output))?
        .into_iter()
        .find(|device| device.path == prepared.selected.path)
        .context("device disappeared before mutation")?;
    revalidate_identity(&prepared.selected, &current)?;

    let shrink = ShrinkRequest {
        disk: prepared.selected.path.clone(),
        root_partition: prepared.root_path,
        partition_number: prepared.root_number,
        root_start_lba: prepared.root_start_lba,
        root_end_lba: prepared.plan.shrink.root_end_lba,
        target_filesystem_kib: prepared.plan.shrink.target_filesystem_bytes / 1024,
    };
    let mutation_mount = PathBuf::from(format!("/run/rust-imager/root-{}", std::process::id()));
    execute_shrink(
        &LinuxShrinkBackend::new(&runner, mutation_mount.clone()),
        &shrink,
    )?;
    let _ = fs::remove_dir_all(mutation_mount);
    let changed = read_mbr_from_disk(Path::new(&prepared.selected.path), prepared.total_sectors)?;
    let changed_root = changed
        .partitions
        .iter()
        .find(|partition| partition.number == prepared.root_number)
        .context("root partition missing after resize")?;
    ensure!(
        changed_root.end_lba == prepared.plan.shrink.root_end_lba,
        "partition geometry did not match the immutable plan"
    );

    let extracted = extract_path(
        Path::new(&prepared.selected.path),
        &request.output,
        &ExtractOptions {
            bytes: prepared.plan.image_bytes,
            compression: request.compression,
            raw_hash: request.verification != VerificationLevel::None,
            buffer_bytes: 1024 * 1024,
            queue_depth: 4,
        },
        |progress| {
            eprintln!(
                "extracting: {} / {} bytes",
                progress.bytes_read, progress.total_bytes
            );
        },
    )?;
    let verification = verify_image(&VerifyRequest {
        image: request.output.clone(),
        compression: request.compression,
        level: request.verification,
        expected_raw_bytes: prepared.plan.image_bytes,
        expected_raw_sha256: extracted.raw_sha256.clone(),
        source: (request.verification == VerificationLevel::SourceReread)
            .then(|| PathBuf::from(&prepared.selected.path)),
    })?;
    let metadata = ImageMetadata::new(
        prepared.selected.path,
        extracted.raw_bytes,
        extracted.compressed_bytes,
        request.compression,
        verification,
        extracted.compressed_sha256,
    );
    let sidecar = write_sidecar(&request.output, &metadata)?;
    println!(
        "completed: {} (metadata: {})",
        request.output.display(),
        sidecar.display()
    );
    Ok(())
}

fn ensure_local_output(runner: &dyn Runner, path: &Path) -> Result<()> {
    let path_text = path.to_string_lossy();
    let details = run_text(
        runner,
        "findmnt",
        &["-T", &path_text, "-n", "-o", "FSTYPE,SOURCE"],
    )?;
    let mut fields = details.split_whitespace();
    let filesystem = fields.next().unwrap_or_default();
    let source = fields.next().unwrap_or_default();
    ensure!(
        !matches!(filesystem, "nfs" | "nfs4" | "cifs" | "fuse.sshfs" | "9p")
            && !source.starts_with("//"),
        "output must be on a local filesystem"
    );
    Ok(())
}

fn available_bytes(runner: &dyn Runner, path: &Path) -> Result<u64> {
    let path_text = path.to_string_lossy();
    let output = run_text(runner, "df", &["--output=avail", "-B1", &path_text])?;
    output
        .lines()
        .rev()
        .find_map(|line| line.trim().parse().ok())
        .context("unable to determine output free space")
}

fn run_text(runner: &dyn Runner, program: &str, args: &[&str]) -> Result<String> {
    let spec = CommandSpec::new(program, args.iter().map(OsString::from));
    Ok(runner
        .run(&spec)
        .with_context(|| format!("failed to run {program}"))?
        .stdout)
}
