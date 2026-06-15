//! Guarded image flashing application engine.

use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;

use anyhow::{Context, Result, ensure};
use rust_imager_core::flash::{ImageFormat, PostVerify, PreVerify};
use rust_imager_linux::command::ProcessRunner;
use rust_imager_linux::discovery::revalidate_identity;
use rust_imager_linux::quiescence::{quiesce_disk, sysfs_holders};
use rust_imager_linux::tools::PartitionTools;
use rust_imager_pipeline::flash::{
    FlashRequest as PipelineRequest, InspectRequest, InspectedImage, flash_image, inspect_image,
};

/// Fully confirmed flash request.
#[derive(Debug, Clone)]
pub struct FlashRequest {
    /// Input image.
    pub image: PathBuf,
    /// Selected whole target disk.
    pub device: String,
    /// Exact target model confirmation.
    pub confirm_model: String,
    /// Pre-write verification.
    pub pre_verify: PreVerify,
    /// Optional target reread.
    pub post_verify: PostVerify,
}

/// Observable flashing state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FlashEvent {
    /// Input image inspection started.
    Inspecting,
    /// No adjacent sidecar was found.
    MissingSidecar,
    /// Target preparation started.
    PreparingTarget,
    /// Decoded bytes are being written.
    Writing {
        /// Bytes written.
        bytes: u64,
        /// Total decoded bytes.
        total: u64,
    },
    /// Optional target reread is running.
    VerifyingTarget,
    /// Flashing completed.
    Complete {
        /// Bytes written.
        bytes: u64,
        /// Decoded SHA-256.
        raw_sha256: String,
        /// Whether target reread passed.
        post_verified: bool,
    },
}

/// Inspect an image without touching a target.
pub fn inspect(request: &FlashRequest) -> Result<InspectedImage> {
    let format = ImageFormat::from_path(&request.image)?;
    inspect_image(&InspectRequest {
        image: request.image.clone(),
        format,
        level: request.pre_verify,
    })
    .map_err(Into::into)
}

/// Execute a guarded flash and print progress.
pub fn run(request: &FlashRequest) -> Result<()> {
    run_with_progress(request, |event| match event {
        FlashEvent::Inspecting => eprintln!("inspecting input image"),
        FlashEvent::MissingSidecar => {
            eprintln!("WARNING: no sidecar metadata; flashing validated image stream");
        }
        FlashEvent::PreparingTarget => eprintln!("preparing target disk"),
        FlashEvent::Writing { bytes, total } => eprintln!("flashing: {bytes} / {total} bytes"),
        FlashEvent::VerifyingTarget => eprintln!("verifying flashed target"),
        FlashEvent::Complete { .. } => {}
    })
}

/// Execute a guarded flash while reporting progress.
pub fn run_with_progress(
    request: &FlashRequest,
    mut on_event: impl FnMut(FlashEvent),
) -> Result<()> {
    on_event(FlashEvent::Inspecting);
    let inspected = inspect(request)?;
    if !inspected.sidecar_present {
        on_event(FlashEvent::MissingSidecar);
    }
    let candidates = crate::app::discover(None)?;
    let selected = candidates
        .iter()
        .find(|candidate| candidate.path == request.device)
        .cloned()
        .context("selected device is not currently a safe USB /dev/sdX candidate")?;
    ensure!(
        selected.model == request.confirm_model.trim(),
        "device model confirmation does not match"
    );
    ensure!(
        selected.logical_sector_size == 512,
        "only 512-byte logical-sector devices are supported"
    );
    ensure!(
        selected.size_bytes >= inspected.raw_bytes,
        "target is smaller than decoded image"
    );
    let current = crate::app::discover(None)?
        .into_iter()
        .find(|candidate| candidate.path == selected.path)
        .context("target disappeared before flashing")?;
    revalidate_identity(&selected, &current)?;

    on_event(FlashEvent::PreparingTarget);
    let runner = ProcessRunner;
    quiesce_disk(&runner, &selected.path, sysfs_holders)?;
    let log_path = PathBuf::from(format!("{}.flash.log", request.image.display()));
    let mut log = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
        .with_context(|| format!("unable to open flash log {}", log_path.display()))?;
    writeln!(
        log,
        "rust-imager {} image={} target={} raw_bytes={} sidecar={}",
        env!("CARGO_PKG_VERSION"),
        request.image.display(),
        selected.path,
        inspected.raw_bytes,
        inspected.sidecar_present
    )?;

    let result = flash_image(
        &PipelineRequest {
            image: request.image.clone(),
            format: ImageFormat::from_path(&request.image)?,
            target: selected.path.clone().into(),
            target_capacity: selected.size_bytes,
            expected_raw_bytes: inspected.raw_bytes,
            expected_raw_sha256: inspected.raw_sha256,
            post_verify: request.post_verify,
        },
        |bytes, total| on_event(FlashEvent::Writing { bytes, total }),
    );
    match result {
        Ok(result) => {
            if request.post_verify == PostVerify::Full {
                on_event(FlashEvent::VerifyingTarget);
            }
            PartitionTools::new(&runner).reread(&selected.path)?;
            writeln!(
                log,
                "result=success bytes={} sha256={} post_verified={}",
                result.bytes_written, result.raw_sha256, result.post_verified
            )?;
            log.sync_all()?;
            on_event(FlashEvent::Complete {
                bytes: result.bytes_written,
                raw_sha256: result.raw_sha256,
                post_verified: result.post_verified,
            });
            println!(
                "flashed {} bytes to {}",
                result.bytes_written, selected.path
            );
            Ok(())
        }
        Err(error) => {
            writeln!(log, "result=failure error={error}")?;
            log.sync_all()?;
            Err(error.into())
        }
    }
}
