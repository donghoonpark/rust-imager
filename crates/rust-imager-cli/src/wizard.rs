//! Interactive terminal wizard.

use std::io::{self, stdout};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::thread;
use std::time::Duration;

use anyhow::{Result, anyhow, bail};
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use rust_imager_core::device::DeviceIdentity;
use rust_imager_core::plan::{Compression, VerificationLevel};
use rust_imager_core::profile::LayoutClass;
use rust_imager_tui::model::{Action, AppModel, OperationResult, PlanPreview, Screen};
use rust_imager_tui::view::draw;
use time::OffsetDateTime;
use time::format_description::well_known::Iso8601;

use crate::app::{EngineEvent, ImageRequest};

enum WorkerMessage {
    Event(EngineEvent),
    Finished(Result<(), String>),
}

/// Run the keyboard-only setup wizard.
pub fn run(devices: Vec<DeviceIdentity>) -> Result<ImageRequest> {
    if devices.is_empty() {
        bail!("no safe external USB /dev/sdX devices found");
    }
    with_terminal(|terminal| event_loop(terminal, AppModel::new(devices)))
}

/// Execute a confirmed request while retaining progress inside the TUI.
pub fn run_operation(request: &ImageRequest) -> Result<()> {
    with_terminal(|terminal| {
        let mut model = AppModel::new(Vec::new());
        model.operation_source = Some(request.device.clone());
        model.output = request.output.to_string_lossy().into_owned();
        model.compression = request.compression;
        model.verification = request.verification;
        model.reduce(Action::PreparationStarted);
        terminal.draw(|frame| draw(frame, &model))?;

        let (sender, receiver) = mpsc::channel();
        let worker_request = request.clone();
        let worker = thread::spawn(move || {
            let events = sender.clone();
            let result = crate::app::run_image_with_progress(&worker_request, move |event| {
                let _ = events.send(WorkerMessage::Event(event));
            })
            .map_err(|error| format!("{error:#}"));
            let _ = sender.send(WorkerMessage::Finished(result));
        });

        let operation_result = loop {
            match receiver.recv_timeout(Duration::from_millis(100)) {
                Ok(WorkerMessage::Event(event)) => apply_engine_event(&mut model, event),
                Ok(WorkerMessage::Finished(result)) => break result,
                Err(RecvTimeoutError::Timeout) => model.reduce(Action::Tick),
                Err(RecvTimeoutError::Disconnected) => {
                    break Err("imaging worker disconnected unexpectedly".into());
                }
            }
            terminal.draw(|frame| draw(frame, &model))?;
        };
        worker
            .join()
            .map_err(|_| anyhow!("imaging worker panicked"))?;

        if let Err(message) = &operation_result {
            model.reduce(Action::Failed(message.clone()));
            terminal.draw(|frame| draw(frame, &model))?;
        }
        wait_for_acknowledgement(terminal)?;
        operation_result.map_err(anyhow::Error::msg)
    })
}

fn apply_engine_event(model: &mut AppModel, event: EngineEvent) {
    match event {
        EngineEvent::Preparing => model.reduce(Action::PreparationStarted),
        EngineEvent::WarningUnknownLayout => {
            model.reduce(Action::SetUnknownLayoutWarning(true));
        }
        EngineEvent::Mutating => model.reduce(Action::MutationStarted),
        EngineEvent::Extracting { bytes, total } => {
            model.reduce(Action::ExtractionStarted);
            model.reduce(Action::Progress { bytes, total });
        }
        EngineEvent::Verifying => model.reduce(Action::VerificationStarted),
        EngineEvent::Complete {
            output,
            raw_bytes,
            compressed_bytes,
            compressed_sha256,
            verification,
            metadata_path,
            log_path,
        } => {
            model.reduce(Action::SetOperationResult(OperationResult {
                output: output.to_string_lossy().into_owned(),
                raw_bytes,
                compressed_bytes,
                compressed_sha256,
                verification: match verification {
                    rust_imager_core::metadata::VerificationStatus::NotPerformed => {
                        "not performed".into()
                    }
                    rust_imager_core::metadata::VerificationStatus::Passed => "passed".into(),
                },
                metadata_path: metadata_path.to_string_lossy().into_owned(),
                log_path: log_path.to_string_lossy().into_owned(),
            }));
            model.reduce(Action::Finished);
        }
    }
}

fn with_terminal<T>(
    operation: impl FnOnce(&mut Terminal<CrosstermBackend<io::Stdout>>) -> Result<T>,
) -> Result<T> {
    enable_raw_mode()?;
    let mut output = stdout();
    execute!(output, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(output);
    let mut terminal = Terminal::new(backend)?;
    let result = operation(&mut terminal);
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    result
}

fn wait_for_acknowledgement(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> Result<()> {
    loop {
        if let Event::Key(key) = event::read()?
            && key.kind == KeyEventKind::Press
            && matches!(key.code, KeyCode::Enter | KeyCode::Esc)
        {
            terminal.show_cursor()?;
            return Ok(());
        }
    }
}

fn event_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    mut model: AppModel,
) -> Result<ImageRequest> {
    loop {
        terminal.draw(|frame| draw(frame, &model))?;
        if !event::poll(Duration::from_millis(250))? {
            continue;
        }
        let Event::Key(key) = event::read()? else {
            continue;
        };
        if key.kind != KeyEventKind::Press {
            continue;
        }
        if key.code == KeyCode::Esc {
            bail!("cancelled before mutation");
        }
        if matches!(key.code, KeyCode::Left | KeyCode::BackTab) {
            model.reduce(Action::Previous);
            continue;
        }
        match model.screen {
            Screen::DeviceSelection => match key.code {
                KeyCode::Up | KeyCode::Char('k') => {
                    model.reduce(Action::MoveDeviceCursor(-1));
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    model.reduce(Action::MoveDeviceCursor(1));
                }
                KeyCode::Enter => {
                    let index = model.device_cursor;
                    select_device(&mut model, index);
                }
                KeyCode::Char(value @ '1'..='9') => {
                    let index = usize::try_from(value.to_digit(10).unwrap_or(0))
                        .unwrap_or(0)
                        .saturating_sub(1);
                    select_device(&mut model, index);
                }
                _ => {}
            },
            Screen::ConfirmDevice => match key.code {
                KeyCode::Char(value) => model.confirmation.push(value),
                KeyCode::Backspace => {
                    model.confirmation.pop();
                }
                KeyCode::Enter => model.reduce(Action::Continue),
                _ => {}
            },
            Screen::Output => handle_output_key(&mut model, key.code)?,
            Screen::Verification => handle_verification_key(&mut model, key.code),
            Screen::Review if key.code == KeyCode::Enter => {
                return request_from_model(&model);
            }
            _ => {}
        }
    }
}

fn handle_output_key(model: &mut AppModel, key: KeyCode) -> Result<()> {
    match key {
        KeyCode::F(2) => {
            model.reduce(Action::SetCompression(Compression::Zstandard { level: 3 }));
        }
        KeyCode::F(3) => {
            model.reduce(Action::SetCompression(Compression::Xz { level: 3 }));
        }
        KeyCode::Char(value) => model.output.push(value),
        KeyCode::Backspace => {
            model.output.pop();
        }
        KeyCode::Enter => {
            model.reduce(Action::Continue);
            if model.screen == Screen::Verification
                && let Some(conflict) = crate::app::output_conflict(Path::new(&model.output))?
            {
                model.reduce(Action::Previous);
                model.reduce(Action::Failed(format!(
                    "Output artifact already exists: {}",
                    conflict.display()
                )));
            }
        }
        _ => {}
    }
    Ok(())
}

fn handle_verification_key(model: &mut AppModel, key: KeyCode) {
    match key {
        KeyCode::Char('1') => model.reduce(Action::SetVerification(VerificationLevel::None)),
        KeyCode::Char('2') => {
            model.reduce(Action::SetVerification(VerificationLevel::StreamingHash));
        }
        KeyCode::Char('3') => model.reduce(Action::SetVerification(VerificationLevel::Decode)),
        KeyCode::Char('4') => {
            model.reduce(Action::SetVerification(VerificationLevel::SourceReread));
        }
        KeyCode::Enter => match request_from_model(model)
            .and_then(|request| crate::app::preview_image(&request))
        {
            Ok(preview) => {
                model.reduce(Action::SetPlanPreview(PlanPreview {
                    source_size_bytes: preview.source_size_bytes,
                    current_filesystem_bytes: preview.current_filesystem_bytes,
                    target_filesystem_bytes: preview.target_filesystem_bytes,
                    image_bytes: preview.image_bytes,
                    output_available_bytes: preview.output_available_bytes,
                    root_partition: preview.root_partition,
                }));
                model.reduce(Action::Continue);
            }
            Err(error) => model.reduce(Action::Failed(format!("{error:#}"))),
        },
        _ => {}
    }
}

fn request_from_model(model: &AppModel) -> Result<ImageRequest> {
    let device = model
        .selected_device()
        .ok_or_else(|| anyhow!("no selected device"))?;
    Ok(ImageRequest {
        device: device.path.clone(),
        output: model.output.clone().into(),
        confirm_model: model.confirmation.clone(),
        compression: model.compression,
        verification: model.verification,
    })
}

fn select_device(model: &mut AppModel, index: usize) {
    let Some(device) = model.devices.get(index).cloned() else {
        model.reduce(Action::Failed("Invalid device selection".into()));
        return;
    };
    match crate::app::analyze_layout(&device) {
        Ok(layout) => {
            model.reduce(Action::SelectDevice(index));
            if model.output.is_empty()
                && let Ok(output) = default_output_path(&device, &std::env::current_dir())
            {
                model.reduce(Action::SetDefaultOutput(
                    output.to_string_lossy().into_owned(),
                ));
            }
            model.reduce(Action::SetUnknownLayoutWarning(
                layout == LayoutClass::WarningUnknown,
            ));
        }
        Err(error) => model.reduce(Action::Failed(format!("{error:#}"))),
    }
}

fn default_output_path(
    device: &DeviceIdentity,
    current_dir: &Result<PathBuf, std::io::Error>,
) -> Result<PathBuf> {
    let directory = current_dir
        .as_ref()
        .map_err(|error| anyhow!("unable to resolve current directory: {error}"))?;
    default_output_path_at(device, directory, OffsetDateTime::now_utc())
}

fn default_output_path_at(
    device: &DeviceIdentity,
    directory: &Path,
    now: OffsetDateTime,
) -> Result<PathBuf> {
    let model = device
        .model
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .split('-')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    let timestamp = now
        .format(&Iso8601::DEFAULT)
        .map_err(|error| anyhow!("unable to format output timestamp: {error}"))?;
    let compact = timestamp
        .chars()
        .filter(char::is_ascii_digit)
        .take(14)
        .collect::<String>();
    Ok(directory.join(format!("{model}-{compact}.img.zst")))
}

#[cfg(test)]
mod tests {
    use super::default_output_path_at;
    use rust_imager_core::device::DeviceIdentity;
    use std::path::Path;
    use time::macros::datetime;

    #[test]
    fn builds_sanitized_timestamped_default_output() {
        let device = DeviceIdentity {
            path: "/dev/sda".into(),
            major_minor: "8:0".into(),
            serial: "ABC".into(),
            model: "Virtual eMMC Reader".into(),
            size_bytes: 64_000_000_000,
            logical_sector_size: 512,
            transport: "usb".into(),
            removable: true,
        };
        let output = default_output_path_at(
            &device,
            Path::new("/images"),
            datetime!(2026-06-14 12:34:56 UTC),
        )
        .expect("default output");
        assert_eq!(
            output,
            Path::new("/images/virtual-emmc-reader-20260614123456.img.zst")
        );
    }
}
