//! Interactive terminal wizard.

use std::io::{self, stdout};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::thread;
use std::time::Duration;

use anyhow::{Context, Result, anyhow, bail};
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Text};
use ratatui::widgets::{Block, Borders, Gauge, Paragraph, Wrap};
use rust_imager_core::device::DeviceIdentity;
use rust_imager_core::flash::{PostVerify, PreVerify};
use rust_imager_core::plan::{Compression, VerificationLevel};
use rust_imager_core::profile::LayoutClass;
use rust_imager_tui::model::{Action, AppModel, OperationResult, PlanPreview, Screen};
use rust_imager_tui::view::draw;
use time::OffsetDateTime;
use time::format_description::well_known::Iso8601;

use crate::app::{EngineEvent, ImageRequest};
use crate::flash::{FlashEvent, FlashRequest};

enum WorkerMessage {
    Event(EngineEvent),
    Finished(Result<(), String>),
}

enum FlashWorkerMessage {
    Event(FlashEvent),
    Finished(Result<(), String>),
}

/// Request selected by the unified TUI.
pub enum WizardRequest {
    /// Compact image creation.
    Image(ImageRequest),
    /// Image flashing.
    Flash(FlashRequest),
}

/// Run the keyboard-only setup wizard.
pub fn run(devices: Vec<DeviceIdentity>) -> Result<WizardRequest> {
    if devices.is_empty() {
        bail!("no safe external USB /dev/sdX devices found");
    }
    with_terminal(|terminal| {
        if choose_operation(terminal)? {
            event_loop(terminal, AppModel::new(devices)).map(WizardRequest::Image)
        } else {
            flash_event_loop(terminal, &devices).map(WizardRequest::Flash)
        }
    })
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

/// Execute a confirmed flash request with a terminal progress dashboard.
pub fn run_flash_operation(request: &FlashRequest) -> Result<()> {
    with_terminal(|terminal| {
        let (sender, receiver) = mpsc::channel();
        let worker_request = request.clone();
        let worker = thread::spawn(move || {
            let events = sender.clone();
            let result = crate::flash::run_with_progress(&worker_request, move |event| {
                let _ = events.send(FlashWorkerMessage::Event(event));
            })
            .map_err(|error| format!("{error:#}"));
            let _ = sender.send(FlashWorkerMessage::Finished(result));
        });
        let mut state = FlashDashboard::default();
        let result = loop {
            match receiver.recv_timeout(Duration::from_millis(100)) {
                Ok(FlashWorkerMessage::Event(event)) => state.apply(event),
                Ok(FlashWorkerMessage::Finished(result)) => break result,
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => {
                    break Err("flash worker disconnected unexpectedly".into());
                }
            }
            terminal.draw(|frame| draw_flash_dashboard(frame, request, &state))?;
        };
        worker
            .join()
            .map_err(|_| anyhow!("flash worker panicked"))?;
        if let Err(message) = &result {
            state.error = Some(message.clone());
            terminal.draw(|frame| draw_flash_dashboard(frame, request, &state))?;
        }
        wait_for_acknowledgement(terminal)?;
        result.map_err(anyhow::Error::msg)
    })
}

#[derive(Default)]
struct FlashDashboard {
    phase: String,
    bytes: u64,
    total: u64,
    hash: Option<String>,
    missing_sidecar: bool,
    complete: bool,
    error: Option<String>,
}

impl FlashDashboard {
    fn apply(&mut self, event: FlashEvent) {
        match event {
            FlashEvent::Inspecting => self.phase = "Inspecting input image".into(),
            FlashEvent::MissingSidecar => self.missing_sidecar = true,
            FlashEvent::PreparingTarget => self.phase = "Preparing target disk".into(),
            FlashEvent::Writing { bytes, total } => {
                self.phase = "Flashing image".into();
                self.bytes = bytes;
                self.total = total;
            }
            FlashEvent::VerifyingTarget => self.phase = "Verifying flashed target".into(),
            FlashEvent::Complete {
                bytes,
                raw_sha256,
                post_verified: _,
            } => {
                self.phase = "Flash complete".into();
                self.bytes = bytes;
                self.total = bytes;
                self.hash = Some(raw_sha256);
                self.complete = true;
            }
        }
    }
}

#[derive(Clone, Copy)]
enum FlashScreen {
    Image,
    PreVerify,
    Device,
    Confirm,
    PostVerify,
    Review,
}

struct FlashSetup {
    screen: FlashScreen,
    image: String,
    pre_verify: PreVerify,
    post_verify: PostVerify,
    cursor: usize,
    selected: Option<usize>,
    confirmation: String,
    sidecar_present: Option<bool>,
    error: Option<String>,
}

fn choose_operation(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> Result<bool> {
    let mut create = true;
    loop {
        terminal.draw(|frame| {
            let area = centered(frame.area(), 72, 16);
            let lines = vec![
                Line::styled(
                    "RUST IMAGER",
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
                Line::from(""),
                choice_line("Create Image", create),
                choice_line("Flash Image", !create),
                Line::from(""),
                Line::styled(
                    "Up/Down or j/k: choose  |  Enter: continue  |  Esc: cancel",
                    Style::default().fg(Color::DarkGray),
                ),
            ];
            frame.render_widget(
                Paragraph::new(lines)
                    .block(Block::default().title(" OPERATION ").borders(Borders::ALL))
                    .wrap(Wrap { trim: true }),
                area,
            );
        })?;
        if let Event::Key(key) = event::read()?
            && key.kind == KeyEventKind::Press
        {
            match key.code {
                KeyCode::Up | KeyCode::Down | KeyCode::Char('j' | 'k') => create = !create,
                KeyCode::Char('1') => return Ok(true),
                KeyCode::Char('2') => return Ok(false),
                KeyCode::Enter => return Ok(create),
                KeyCode::Esc => bail!("cancelled before mutation"),
                _ => {}
            }
        }
    }
}

#[allow(clippy::too_many_lines)]
fn flash_event_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    devices: &[DeviceIdentity],
) -> Result<FlashRequest> {
    let mut setup = FlashSetup {
        screen: FlashScreen::Image,
        image: String::new(),
        pre_verify: PreVerify::Basic,
        post_verify: PostVerify::None,
        cursor: 0,
        selected: None,
        confirmation: String::new(),
        sidecar_present: None,
        error: None,
    };
    loop {
        terminal.draw(|frame| draw_flash_setup(frame, devices, &setup))?;
        let Event::Key(key) = event::read()? else {
            continue;
        };
        if key.kind != KeyEventKind::Press {
            continue;
        }
        setup.error = None;
        if key.code == KeyCode::Esc {
            bail!("cancelled before mutation");
        }
        match setup.screen {
            FlashScreen::Image => match key.code {
                KeyCode::Char(value) => setup.image.push(value),
                KeyCode::Backspace => {
                    setup.image.pop();
                }
                KeyCode::Enter if !setup.image.trim().is_empty() => {
                    setup.screen = FlashScreen::PreVerify;
                }
                _ => {}
            },
            FlashScreen::PreVerify => match key.code {
                KeyCode::Char('1') => setup.pre_verify = PreVerify::None,
                KeyCode::Char('2') => setup.pre_verify = PreVerify::Basic,
                KeyCode::Char('3') => setup.pre_verify = PreVerify::Full,
                KeyCode::Enter => {
                    let probe = FlashRequest {
                        image: setup.image.clone().into(),
                        device: String::new(),
                        confirm_model: String::new(),
                        pre_verify: setup.pre_verify,
                        post_verify: setup.post_verify,
                    };
                    match crate::flash::inspect(&probe) {
                        Ok(inspected) => {
                            setup.sidecar_present = Some(inspected.sidecar_present);
                            setup.screen = FlashScreen::Device;
                        }
                        Err(error) => setup.error = Some(format!("{error:#}")),
                    }
                }
                _ => {}
            },
            FlashScreen::Device => match key.code {
                KeyCode::Up | KeyCode::Char('k') => {
                    setup.cursor = setup.cursor.checked_sub(1).unwrap_or(devices.len() - 1);
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    setup.cursor = (setup.cursor + 1) % devices.len();
                }
                KeyCode::Enter => {
                    setup.selected = Some(setup.cursor);
                    setup.screen = FlashScreen::Confirm;
                }
                _ => {}
            },
            FlashScreen::Confirm => match key.code {
                KeyCode::Char(value) => setup.confirmation.push(value),
                KeyCode::Backspace => {
                    setup.confirmation.pop();
                }
                KeyCode::Enter => {
                    let expected = setup
                        .selected
                        .and_then(|index| devices.get(index))
                        .map(|device| device.model.as_str());
                    if expected == Some(setup.confirmation.trim()) {
                        setup.screen = FlashScreen::PostVerify;
                    } else {
                        setup.error = Some("Type the target model exactly".into());
                    }
                }
                _ => {}
            },
            FlashScreen::PostVerify => match key.code {
                KeyCode::Char('1') => setup.post_verify = PostVerify::None,
                KeyCode::Char('2') => setup.post_verify = PostVerify::Full,
                KeyCode::Enter => setup.screen = FlashScreen::Review,
                _ => {}
            },
            FlashScreen::Review if key.code == KeyCode::Enter => {
                let device = setup
                    .selected
                    .and_then(|index| devices.get(index))
                    .context("no selected target")?;
                return Ok(FlashRequest {
                    image: setup.image.into(),
                    device: device.path.clone(),
                    confirm_model: setup.confirmation,
                    pre_verify: setup.pre_verify,
                    post_verify: setup.post_verify,
                });
            }
            FlashScreen::Review => {}
        }
    }
}

#[allow(clippy::too_many_lines)]
fn draw_flash_setup(
    frame: &mut ratatui::Frame<'_>,
    devices: &[DeviceIdentity],
    setup: &FlashSetup,
) {
    let area = centered(frame.area(), 100, 28);
    let selected = setup.selected.and_then(|index| devices.get(index));
    let body = match setup.screen {
        FlashScreen::Image => Text::from(vec![
            Line::from("IMAGE PATH"),
            Line::styled(&setup.image, Style::default().fg(Color::Cyan)),
            Line::from("Supported: .img, .img.zst, .img.xz"),
        ]),
        FlashScreen::PreVerify => Text::from(vec![
            Line::from("PRE-FLASH VERIFICATION"),
            choice_line("1  None", setup.pre_verify == PreVerify::None),
            choice_line(
                "2  Basic (decode + MBR)",
                setup.pre_verify == PreVerify::Basic,
            ),
            choice_line(
                "3  Full (sidecar hashes when present)",
                setup.pre_verify == PreVerify::Full,
            ),
        ]),
        FlashScreen::Device => Text::from(
            devices
                .iter()
                .enumerate()
                .map(|(index, device)| {
                    choice_line(
                        &format!("{}  {}  {}", device.path, device.model, device.size_bytes),
                        index == setup.cursor,
                    )
                })
                .collect::<Vec<_>>(),
        ),
        FlashScreen::Confirm => Text::from(vec![
            Line::styled(
                "WARNING: THE TARGET DISK WILL BE OVERWRITTEN",
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            ),
            Line::from(format!(
                "Target: {}  {}",
                selected.map_or("", |device| &device.path),
                selected.map_or("", |device| &device.model)
            )),
            Line::from("Type the exact model:"),
            Line::styled(&setup.confirmation, Style::default().fg(Color::Cyan)),
        ]),
        FlashScreen::PostVerify => Text::from(vec![
            Line::from("POST-FLASH VERIFICATION"),
            choice_line("1  None", setup.post_verify == PostVerify::None),
            choice_line(
                "2  Full target reread",
                setup.post_verify == PostVerify::Full,
            ),
        ]),
        FlashScreen::Review => Text::from(vec![
            Line::styled(
                "DESTRUCTIVE TARGET",
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            ),
            Line::from(format!(
                "{}  {}",
                selected.map_or("", |device| &device.path),
                selected.map_or("", |device| &device.model)
            )),
            Line::from(format!("Image: {}", setup.image)),
            Line::from(format!("Pre-verify: {:?}", setup.pre_verify)),
            Line::from(format!("Post-verify: {:?}", setup.post_verify)),
            Line::from(format!(
                "Sidecar: {}",
                if setup.sidecar_present == Some(true) {
                    "validated"
                } else {
                    "missing; allowed with warning"
                }
            )),
            Line::from(""),
            Line::styled(
                "Press Enter to overwrite the target",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
    };
    let mut lines = body.lines;
    if let Some(error) = &setup.error {
        lines.push(Line::from(""));
        lines.push(Line::styled(error, Style::default().fg(Color::Red)));
    }
    lines.push(Line::from(""));
    lines.push(Line::styled(
        "Enter: continue  |  Esc: cancel",
        Style::default().fg(Color::DarkGray),
    ));
    frame.render_widget(
        Paragraph::new(lines)
            .block(
                Block::default()
                    .title(" FLASH IMAGE ")
                    .borders(Borders::ALL),
            )
            .wrap(Wrap { trim: true }),
        area,
    );
}

fn draw_flash_dashboard(
    frame: &mut ratatui::Frame<'_>,
    request: &FlashRequest,
    state: &FlashDashboard,
) {
    let area = centered(frame.area(), 100, 24);
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(8),
            Constraint::Length(5),
            Constraint::Min(5),
        ])
        .split(area);
    frame.render_widget(
        Paragraph::new(vec![
            Line::styled(
                if state.complete {
                    "FLASH COMPLETE"
                } else {
                    "FLASHING IMAGE"
                },
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Line::from(format!("Image: {}", request.image.display())),
            Line::from(format!("Target: {}", request.device)),
            Line::from(format!("Phase: {}", state.phase)),
            Line::from(if state.missing_sidecar {
                "Warning: sidecar metadata was not present"
            } else {
                ""
            }),
        ])
        .block(
            Block::default()
                .title(" RUST IMAGER ")
                .borders(Borders::ALL),
        ),
        chunks[0],
    );
    let percent = if state.total == 0 {
        0
    } else {
        u16::try_from(state.bytes.saturating_mul(100) / state.total).unwrap_or(100)
    };
    frame.render_widget(
        Gauge::default()
            .block(Block::default().title(" PROGRESS ").borders(Borders::ALL))
            .percent(percent.min(100))
            .label(format!("{} / {} bytes", state.bytes, state.total)),
        chunks[1],
    );
    let detail = if let Some(error) = &state.error {
        Line::styled(error, Style::default().fg(Color::Red))
    } else if let Some(hash) = &state.hash {
        Line::from(format!("Raw SHA-256: {hash}\nEnter/Esc: close"))
    } else {
        Line::from("Do not disconnect the target device")
    };
    frame.render_widget(
        Paragraph::new(detail).block(Block::default().title(" STATUS ").borders(Borders::ALL)),
        chunks[2],
    );
}

fn centered(area: ratatui::layout::Rect, width: u16, height: u16) -> ratatui::layout::Rect {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(area.height.saturating_sub(height) / 2),
            Constraint::Length(height.min(area.height)),
            Constraint::Min(0),
        ])
        .split(area);
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(area.width.saturating_sub(width) / 2),
            Constraint::Length(width.min(area.width)),
            Constraint::Min(0),
        ])
        .split(vertical[1])[1]
}

fn choice_line(label: &str, selected: bool) -> Line<'static> {
    Line::styled(
        format!("{} {label}", if selected { ">" } else { " " }),
        if selected {
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        },
    )
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
