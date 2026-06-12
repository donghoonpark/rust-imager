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
use rust_imager_tui::model::{Action, AppModel, Screen};
use rust_imager_tui::view::draw;

use crate::app::ImageRequest;

/// Run the keyboard-only setup wizard.
pub fn run(devices: Vec<DeviceIdentity>) -> Result<ImageRequest> {
    if devices.is_empty() {
        bail!("no safe external USB /dev/sdX devices found");
    }
    enable_raw_mode()?;
    let mut output = stdout();
    execute!(output, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(output);
    let mut terminal = Terminal::new(backend)?;
    let result = event_loop(&mut terminal, AppModel::new(devices));
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    result
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
                    model.reduce(Action::SelectDevice(index));
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
