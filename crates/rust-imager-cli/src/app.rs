//! End-to-end imaging application engine.

use std::ffi::OsString;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::thread;

use anyhow::{Context, Result, bail, ensure};
use nix::unistd::Uid;
use rust_imager_core::device::DeviceIdentity;
use rust_imager_core::metadata::{ImageMetadata, PartitionMetadata, VerificationStatus};
use rust_imager_core::plan::ExecutionPlan;
use rust_imager_core::plan::{Compression, PlanInput, VerificationLevel, build_plan};
use rust_imager_core::profile::{LayoutClass, classify_layout};
use rust_imager_linux::command::{CommandSpec, ProcessRunner, Runner};
use rust_imager_linux::discovery::{discover_candidates, revalidate_identity};
use rust_imager_linux::inspect::{
    ext4_geometry, inspect_filesystems, partition_path, read_mbr_from_disk,
};
use rust_imager_linux::shrink::{LinuxShrinkBackend, ShrinkRequest, execute_shrink};
use rust_imager_pipeline::extract::{ExtractOptions, extract_path, partial_path};
use rust_imager_pipeline::verify::{
    VerifyRequest, sidecar_partial_path, sidecar_path, verify_image, write_sidecar,
};
use signal_hook::consts::signal::{SIGINT, SIGTERM};
use signal_hook::iterator::Signals;

// Keep this list compatible with util-linux 2.31 (Ubuntu 18.04). PATH and
// MOUNTPOINTS were added later. The --paths option makes NAME contain the
// absolute device path, and mounted devices are identified via findmnt below.
const LSBLK_DISCOVERY_COLUMNS: &str = "NAME,TYPE,SIZE,LOG-SEC,TRAN,MODEL,SERIAL,MAJ:MIN,RM,FSTYPE";

// Keep discovery compatible with findmnt from util-linux 2.31 (Ubuntu 18.04).
// --real was added later; findmnt already reports /dev paths for block-backed
// filesystems without it.
const FINDMNT_DISCOVERY_ARGS: &[&str] = &["--json", "--output", "TARGET,SOURCE"];

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

/// Read-only facts calculated before the destructive operation begins.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImagePreview {
    /// Original whole-device capacity.
    pub source_size_bytes: u64,
    /// Current ext4 filesystem size.
    pub current_filesystem_bytes: u64,
    /// Planned ext4 filesystem size.
    pub target_filesystem_bytes: u64,
    /// Raw byte range that will be extracted.
    pub image_bytes: u64,
    /// Available bytes on the output filesystem.
    pub output_available_bytes: u64,
    /// Root partition that will be modified.
    pub root_partition: String,
}

/// Observable engine state for terminal and non-interactive frontends.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EngineEvent {
    /// Device and filesystem analysis is running.
    Preparing,
    /// The layout is safe enough to continue but does not match a known profile.
    WarningUnknownLayout,
    /// The source filesystem and partition are being modified.
    Mutating,
    /// Raw source bytes are being extracted.
    Extracting {
        /// Bytes read from the source.
        bytes: u64,
        /// Total planned bytes.
        total: u64,
    },
    /// The completed image is being verified.
    Verifying,
    /// Image and metadata finalization completed.
    Complete {
        /// Final compressed image path.
        output: PathBuf,
        /// Raw byte range extracted.
        raw_bytes: u64,
        /// Compressed file size.
        compressed_bytes: u64,
        /// SHA-256 of the compressed stream.
        compressed_sha256: String,
        /// Verification outcome.
        verification: VerificationStatus,
        /// JSON metadata path.
        metadata_path: PathBuf,
        /// Durable operation log path.
        log_path: PathBuf,
    },
}

struct Prepared {
    selected: DeviceIdentity,
    plan: ExecutionPlan,
    root_number: u8,
    root_start_lba: u64,
    root_path: String,
    partitions: Vec<PartitionMetadata>,
    original_mbr: rust_imager_core::mbr::Mbr,
    unknown_layout_warning: bool,
    current_filesystem_bytes: u64,
    output_available_bytes: u64,
}

struct Analysis {
    mbr: rust_imager_core::mbr::Mbr,
    layout: LayoutClass,
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
    let normalized_output = output.map(normalize_output_path).transpose()?;
    let lsblk = run_text(
        &runner,
        "lsblk",
        &[
            "--json",
            "--bytes",
            "--paths",
            "--output",
            LSBLK_DISCOVERY_COLUMNS,
        ],
    )?;
    let findmnt = run_text(&runner, "findmnt", FINDMNT_DISCOVERY_ARGS)?;
    discover_candidates(
        &lsblk,
        &findmnt,
        normalized_output.as_deref().and_then(Path::to_str),
    )
    .map_err(Into::into)
}

/// Analyze, shrink, extract, verify, and write metadata.
pub fn run_image(request: &ImageRequest) -> Result<()> {
    let result = run_image_with_progress(request, |event| match event {
        EngineEvent::Preparing => eprintln!("preparing imaging plan"),
        EngineEvent::WarningUnknownLayout => {
            eprintln!("STRONG WARNING: layout does not match a known Raspberry Pi/ODROID profile");
        }
        EngineEvent::Mutating => eprintln!("shrinking source filesystem and partition"),
        EngineEvent::Extracting { bytes, total } => {
            eprintln!("extracting: {bytes} / {total} bytes");
        }
        EngineEvent::Verifying => eprintln!("verifying image"),
        EngineEvent::Complete { .. } => {}
    });
    if result.is_ok() {
        println!(
            "completed: {} (metadata: {})",
            request.output.display(),
            sidecar_path(&request.output).display()
        );
    }
    result
}

/// Run imaging while reporting high-level state and byte progress.
pub fn run_image_with_progress(
    request: &ImageRequest,
    mut on_event: impl FnMut(EngineEvent),
) -> Result<()> {
    let parent = request.output.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)?;
    validate_request(request)?;
    on_event(EngineEvent::Preparing);
    let log_path = append_suffix(&request.output, ".log");
    let mut log = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&log_path)
        .with_context(|| format!("refusing to overwrite log {}", log_path.display()))?;
    writeln!(
        log,
        "rust-imager {} source={} output={}",
        env!("CARGO_PKG_VERSION"),
        request.device,
        request.output.display()
    )?;
    let mut stage = "preparing";
    let mut source_modified = false;
    let result = prepare(request).and_then(|prepared| {
        if prepared.unknown_layout_warning {
            on_event(EngineEvent::WarningUnknownLayout);
        }
        execute(request, prepared, &mut |event| {
            match event {
                EngineEvent::Mutating => {
                    stage = "mutating";
                    source_modified = true;
                }
                EngineEvent::Extracting { .. } => stage = "extracting",
                EngineEvent::Verifying => stage = "verifying",
                EngineEvent::Complete { .. } => stage = "complete",
                EngineEvent::Preparing | EngineEvent::WarningUnknownLayout => {}
            }
            on_event(event);
        })
    });
    match &result {
        Ok(()) => writeln!(
            log,
            "result=success stage={stage} source_modified={source_modified}"
        )?,
        Err(error) => writeln!(
            log,
            "result=failure stage={stage} source_modified={source_modified} error={error:#}"
        )?,
    }
    log.sync_all()?;
    result
}

/// Calculate the immutable execution plan without mutating the source device.
pub fn preview_image(request: &ImageRequest) -> Result<ImagePreview> {
    validate_request(request)?;
    let prepared = prepare(request)?;
    Ok(ImagePreview {
        source_size_bytes: prepared.selected.size_bytes,
        current_filesystem_bytes: prepared.current_filesystem_bytes,
        target_filesystem_bytes: prepared.plan.shrink.target_filesystem_bytes,
        image_bytes: prepared.plan.image_bytes,
        output_available_bytes: prepared.output_available_bytes,
        root_partition: prepared.root_path,
    })
}

fn validate_request(request: &ImageRequest) -> Result<()> {
    let expected_extension = match request.compression {
        Compression::Zstandard { level } => {
            ensure!(
                (-7..=22).contains(&level),
                "zstd level must be between -7 and 22"
            );
            "zst"
        }
        Compression::Xz { level } => {
            ensure!(level <= 9, "xz level must be between 0 and 9");
            "xz"
        }
    };
    ensure!(
        request.output.extension().and_then(|value| value.to_str()) == Some(expected_extension),
        "output extension must be .{expected_extension}"
    );
    if let Some(path) = output_conflict(&request.output)? {
        bail!("output already exists: {}", path.display());
    }
    Ok(())
}

/// Return the first output artifact that would be overwritten.
pub fn output_conflict(output: &Path) -> Result<Option<PathBuf>> {
    for path in [
        output.to_path_buf(),
        partial_path(output),
        sidecar_path(output),
        sidecar_partial_path(output),
        append_suffix(output, ".log"),
    ] {
        if path_occupied(&path)? {
            return Ok(Some(path));
        }
    }
    Ok(None)
}

fn prepare(request: &ImageRequest) -> Result<Prepared> {
    let candidates = discover(Some(&request.output))?;
    let selected = candidates
        .iter()
        .find(|device| device.path == request.device)
        .cloned()
        .context("selected device is not currently a safe USB /dev/sdX candidate")?;
    ensure!(
        selected.logical_sector_size == 512,
        "only 512-byte logical-sector devices are supported"
    );
    ensure!(
        selected.model == request.confirm_model.trim(),
        "device model confirmation does not match"
    );
    let runner = ProcessRunner;
    let analysis = analyze(&selected)?;
    match &analysis.layout {
        LayoutClass::Unsupported(reason) => bail!("unsupported layout: {reason}"),
        LayoutClass::Recognized(_) | LayoutClass::WarningUnknown => {}
    }
    let unknown_layout_warning = analysis.layout == LayoutClass::WarningUnknown;
    let mbr = analysis.mbr;
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
    let output_available_bytes = available_bytes(&runner, output_parent)?;
    let plan = build_plan(PlanInput {
        device_path: selected.path.clone(),
        device_size_bytes: selected.size_bytes,
        sector_size: selected.logical_sector_size,
        root_start_lba: root.start_lba,
        ext4_minimum_bytes: ext4.minimum_bytes,
        ext4_current_bytes: ext4.current_bytes,
        output_path: request.output.to_string_lossy().into_owned(),
        output_available_bytes,
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

    let partitions = mbr
        .partitions
        .iter()
        .map(|partition| PartitionMetadata {
            number: partition.number,
            type_code: partition.type_code,
            start_lba: partition.start_lba,
            end_lba: if partition.number == root.number {
                plan.shrink.root_end_lba
            } else {
                partition.end_lba
            },
        })
        .collect();
    Ok(Prepared {
        selected,
        plan,
        root_number: root.number,
        root_start_lba: root.start_lba,
        root_path,
        partitions,
        original_mbr: mbr,
        unknown_layout_warning,
        current_filesystem_bytes: ext4.current_bytes,
        output_available_bytes,
    })
}

/// Inspect a candidate without modifying it and classify its SBC layout.
pub fn analyze_layout(device: &DeviceIdentity) -> Result<LayoutClass> {
    let analysis = analyze(device)?;
    if let LayoutClass::Unsupported(reason) = &analysis.layout {
        bail!("unsupported layout: {reason}");
    }
    Ok(analysis.layout)
}

fn analyze(device: &DeviceIdentity) -> Result<Analysis> {
    let total_sectors = device
        .size_bytes
        .checked_div(device.logical_sector_size)
        .context("invalid device sector size")?;
    let mbr = read_mbr_from_disk(Path::new(&device.path), total_sectors)?;
    let runner = ProcessRunner;
    let mount_base = tempfile::Builder::new()
        .prefix("rust-imager-inspect-")
        .tempdir_in("/run")
        .context("unable to create secure inspection mount directory")?;
    let evidence = inspect_filesystems(&runner, &device.path, &mbr, mount_base.path())?;
    let layout = classify_layout(&mbr, &evidence);
    Ok(Analysis { mbr, layout })
}

fn execute(
    request: &ImageRequest,
    prepared: Prepared,
    on_event: &mut impl FnMut(EngineEvent),
) -> Result<()> {
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
        device_size_bytes: prepared.selected.size_bytes,
        logical_sector_size: prepared.selected.logical_sector_size,
        original_mbr: prepared.original_mbr,
    };
    let mutation_mount = tempfile::Builder::new()
        .prefix("rust-imager-root-")
        .tempdir_in("/run")
        .context("unable to create secure mutation mount directory")?;
    on_event(EngineEvent::Mutating);
    defer_termination(|| {
        execute_shrink(
            &LinuxShrinkBackend::new(&runner, mutation_mount.path().to_path_buf()),
            &shrink,
        )
    })??;
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
            on_event(EngineEvent::Extracting {
                bytes: progress.bytes_read,
                total: progress.total_bytes,
            });
        },
    )?;
    on_event(EngineEvent::Verifying);
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
        prepared.selected.path.clone(),
        extracted.raw_bytes,
        extracted.compressed_bytes,
        request.compression,
        verification,
        extracted.compressed_sha256,
    )
    .with_source_layout(
        prepared.selected.size_bytes,
        prepared.selected.logical_sector_size,
        prepared.partitions,
    );
    write_sidecar(&request.output, &metadata)?;
    on_event(EngineEvent::Complete {
        output: request.output.clone(),
        raw_bytes: metadata.raw_bytes,
        compressed_bytes: metadata.compressed_bytes,
        compressed_sha256: metadata.compressed_sha256.clone(),
        verification: metadata.verification.status,
        metadata_path: sidecar_path(&request.output),
        log_path: append_suffix(&request.output, ".log"),
    });
    Ok(())
}

fn append_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut value = path.as_os_str().to_os_string();
    value.push(suffix);
    PathBuf::from(value)
}

fn path_occupied(path: &Path) -> Result<bool> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error).with_context(|| format!("unable to inspect {}", path.display())),
    }
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

fn normalize_output_path(output: &Path) -> Result<PathBuf> {
    let parent = output
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let parent = parent
        .canonicalize()
        .with_context(|| format!("unable to resolve output directory {}", parent.display()))?;
    let name = output.file_name().context("output path must name a file")?;
    Ok(parent.join(name))
}

fn defer_termination<T>(operation: impl FnOnce() -> T) -> Result<T> {
    let mut signals = Signals::new([SIGINT, SIGTERM])?;
    let handle = signals.handle();
    let listener = thread::spawn(move || {
        for signal in signals.forever() {
            eprintln!(
                "ignoring signal {signal} while source mutation is active; wait for the current operation"
            );
        }
    });
    let result = operation();
    handle.close();
    listener
        .join()
        .map_err(|_| anyhow::anyhow!("signal listener thread failed"))?;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(output: PathBuf, compression: Compression) -> ImageRequest {
        ImageRequest {
            device: "/dev/sdz".into(),
            output,
            confirm_model: "fixture".into(),
            compression,
            verification: VerificationLevel::Decode,
        }
    }

    #[test]
    fn validates_compression_extension_and_level() {
        let directory = tempfile::tempdir().expect("tempdir");
        validate_request(&request(
            directory.path().join("image.zst"),
            Compression::Zstandard { level: 3 },
        ))
        .expect("valid zstd request");
        assert!(
            validate_request(&request(
                directory.path().join("image.xz"),
                Compression::Zstandard { level: 3 },
            ))
            .is_err()
        );
        assert!(
            validate_request(&request(
                directory.path().join("image.xz"),
                Compression::Xz { level: 10 },
            ))
            .is_err()
        );
    }

    #[test]
    fn rejects_any_existing_output_artifact() {
        let directory = tempfile::tempdir().expect("tempdir");
        let output = directory.path().join("image.zst");
        fs::write(sidecar_path(&output), b"existing").expect("fixture");
        assert!(validate_request(&request(output, Compression::Zstandard { level: 3 })).is_err());
    }

    #[test]
    fn reports_each_output_conflict_path() {
        let directory = tempfile::tempdir().expect("tempdir");
        let output = directory.path().join("image.zst");
        let conflicts = [
            output.clone(),
            partial_path(&output),
            sidecar_path(&output),
            sidecar_partial_path(&output),
            append_suffix(&output, ".log"),
        ];
        for conflict in conflicts {
            fs::write(&conflict, b"existing").expect("fixture");
            assert_eq!(
                output_conflict(&output).expect("conflict query"),
                Some(conflict.clone())
            );
            fs::remove_file(conflict).expect("remove fixture");
        }
        assert_eq!(output_conflict(&output).expect("clear query"), None);
    }

    #[test]
    fn normalizes_relative_and_symlinked_output_parents() {
        let directory = tempfile::tempdir().expect("tempdir");
        let link = directory.path().join("link");
        std::os::unix::fs::symlink(directory.path(), &link).expect("symlink");
        let normalized = normalize_output_path(&link.join("image.zst")).expect("normalized output");
        assert_eq!(
            normalized,
            directory
                .path()
                .canonicalize()
                .expect("canonical tempdir")
                .join("image.zst")
        );
        assert!(normalized.is_absolute());
    }

    #[test]
    fn lsblk_discovery_columns_support_util_linux_2_31() {
        for unsupported in ["PATH", "MOUNTPOINTS"] {
            assert!(
                !LSBLK_DISCOVERY_COLUMNS
                    .split(',')
                    .any(|column| column == unsupported)
            );
        }
        for required in [
            "NAME", "TYPE", "SIZE", "LOG-SEC", "TRAN", "MODEL", "SERIAL", "MAJ:MIN", "RM", "FSTYPE",
        ] {
            assert!(
                LSBLK_DISCOVERY_COLUMNS
                    .split(',')
                    .any(|column| column == required)
            );
        }
    }

    #[test]
    fn findmnt_discovery_args_support_util_linux_2_31() {
        assert!(!FINDMNT_DISCOVERY_ARGS.contains(&"--real"));
        assert_eq!(
            FINDMNT_DISCOVERY_ARGS,
            ["--json", "--output", "TARGET,SOURCE"]
        );
    }
}
