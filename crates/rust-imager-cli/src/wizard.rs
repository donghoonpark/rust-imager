//! Interactive terminal wizard.

use std::io::{self, stdout};
use std::time::Duration;

use anyhow::{Result, bail};
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
use rust_imager_tui::model::{Action, AppModel, Screen};
use rust_imager_tui::view::draw;

use crate::app::{EngineEvent, ImageRequest};

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
        model.output = request.output.to_string_lossy().into_owned();
        model.compression = request.compression;
        model.verification = request.verification;
        model.reduce(Action::PreparationStarted);
        terminal.draw(|frame| draw(frame, &model))?;

        let mut draw_error = None;
        let result = crate::app::run_image_with_progress(request, |event| {
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
                EngineEvent::Complete => model.reduce(Action::Finished),
            }
            if let Err(error) = terminal.draw(|frame| draw(frame, &model)) {
                draw_error = Some(error);
            }
        });
        if let Some(error) = draw_error {
            return Err(error.into());
        }
        if let Err(error) = result {
            model.reduce(Action::Failed(format!("{error:#}")));
            terminal.draw(|frame| draw(frame, &model))?;
            return Err(error);
        }
        wait_for_acknowledgement(terminal)
    })
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
        match model.screen {
            Screen::DeviceSelection => {
                if let KeyCode::Char(value @ '1'..='9') = key.code {
                    let index = usize::try_from(value.to_digit(10).unwrap_or(0))
                        .unwrap_or(0)
                        .saturating_sub(1);
                    let Some(device) = model.devices.get(index).cloned() else {
                        model.reduce(Action::Failed("Invalid device selection".into()));
                        continue;
                    };
                    match crate::app::analyze_layout(&device) {
                        Ok(layout) => {
                            model.reduce(Action::SelectDevice(index));
                            model.reduce(Action::SetUnknownLayoutWarning(
                                layout == LayoutClass::WarningUnknown,
                            ));
                        }
                        Err(error) => model.reduce(Action::Failed(format!("{error:#}"))),
                    }
                }
            }
            Screen::ConfirmDevice => match key.code {
                KeyCode::Char(value) => model.confirmation.push(value),
                KeyCode::Backspace => {
                    model.confirmation.pop();
                }
                KeyCode::Enter => model.reduce(Action::Continue),
                _ => {}
            },
            Screen::Output => match key.code {
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
                KeyCode::Enter => model.reduce(Action::Continue),
                _ => {}
            },
            Screen::Verification => match key.code {
                KeyCode::Char('1') => {
                    model.reduce(Action::SetVerification(VerificationLevel::None));
                }
                KeyCode::Char('2') => {
                    model.reduce(Action::SetVerification(VerificationLevel::StreamingHash));
                }
                KeyCode::Char('3') => {
                    model.reduce(Action::SetVerification(VerificationLevel::Decode));
                }
                KeyCode::Char('4') => {
                    model.reduce(Action::SetVerification(VerificationLevel::SourceReread));
                }
                KeyCode::Enter => model.reduce(Action::Continue),
                _ => {}
            },
            Screen::Review if key.code == KeyCode::Enter => {
                let device = model
                    .selected_device()
                    .ok_or_else(|| anyhow::anyhow!("no selected device"))?;
                return Ok(ImageRequest {
                    device: device.path.clone(),
                    output: model.output.clone().into(),
                    confirm_model: model.confirmation.clone(),
                    compression: model.compression,
                    verification: model.verification,
                });
            }
            _ => {}
        }
    }
}
