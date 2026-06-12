//! Ratatui rendering for the wizard.

use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Gauge, List, ListItem, Paragraph, Wrap};

use crate::model::{AppModel, Screen};

/// Draw the current model.
pub fn draw(frame: &mut Frame<'_>, model: &AppModel) {
    let area = frame.area();
    if area.width < 40 || area.height < 10 {
        frame.render_widget(
            Paragraph::new("rust-imager\nTerminal too small (minimum 40x10)")
                .block(Block::default().borders(Borders::ALL))
                .wrap(Wrap { trim: true }),
            area,
        );
        return;
    }
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(4),
            Constraint::Length(3),
        ])
        .split(area);
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(
                "rust-imager",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(format!("  {:?}", model.screen)),
        ]))
        .block(Block::default().borders(Borders::ALL)),
        chunks[0],
    );

    render_body(frame, chunks[1], model);
    let footer = model.error.as_deref().unwrap_or(
        "Enter: continue  Esc: cancel before mutation  Source changes are permanent after start",
    );
    frame.render_widget(
        Paragraph::new(footer).block(Block::default().borders(Borders::ALL)),
        chunks[2],
    );
}

fn render_body(frame: &mut Frame<'_>, area: ratatui::layout::Rect, model: &AppModel) {
    match model.screen {
        Screen::DeviceSelection => {
            let items: Vec<_> = model
                .devices
                .iter()
                .enumerate()
                .map(|(index, device)| {
                    ListItem::new(format!(
                        "{}. {}  {}  {} bytes",
                        index + 1,
                        device.path,
                        device.model,
                        device.size_bytes
                    ))
                })
                .collect();
            frame.render_widget(
                List::new(items).block(
                    Block::default()
                        .title("External USB /dev/sdX devices")
                        .borders(Borders::ALL),
                ),
                area,
            );
        }
        Screen::ConfirmDevice => {
            let device = model.selected_device();
            frame.render_widget(
                Paragraph::new(format!(
                    "WARNING: the source eMMC will be modified and left shrunken.\n\
                     Device: {}\nModel: {}\n\nType model: {}",
                    device.map_or("?", |value| value.path.as_str()),
                    device.map_or("?", |value| value.model.as_str()),
                    model.confirmation
                ))
                .block(Block::default().borders(Borders::ALL))
                .wrap(Wrap { trim: false }),
                area,
            );
        }
        Screen::Output | Screen::Verification | Screen::Review => {
            frame.render_widget(
                Paragraph::new(format!(
                    "Output: {}\nCompression: {:?}\nVerification: {:?}",
                    model.output, model.compression, model.verification
                ))
                .block(Block::default().borders(Borders::ALL)),
                area,
            );
        }
        Screen::Preparing => frame.render_widget(
            Paragraph::new("Inspecting the device, filesystems, output, and immutable plan")
                .block(Block::default().borders(Borders::ALL)),
            area,
        ),
        Screen::Mutating => frame.render_widget(
            warning("DO NOT REMOVE POWER: filesystem and partition metadata are changing"),
            area,
        ),
        Screen::Extracting => {
            let ratio = progress_ratio(model.progress_bytes, model.progress_total);
            frame.render_widget(
                Gauge::default()
                    .block(Block::default().borders(Borders::ALL).title("Extracting"))
                    .ratio(ratio),
                area,
            );
        }
        Screen::Verifying => frame.render_widget(
            Paragraph::new("Verifying the compressed image and writing metadata")
                .block(Block::default().borders(Borders::ALL)),
            area,
        ),
        Screen::Complete => frame.render_widget(
            Paragraph::new("Image extraction and verification completed")
                .block(Block::default().borders(Borders::ALL)),
            area,
        ),
    }
}

fn warning(text: &str) -> Paragraph<'_> {
    Paragraph::new(text)
        .style(Style::default().fg(Color::Red).add_modifier(Modifier::BOLD))
        .block(Block::default().borders(Borders::ALL))
        .wrap(Wrap { trim: true })
}

fn progress_ratio(bytes: u64, total: u64) -> f64 {
    if total == 0 {
        return 0.0;
    }
    let thousandths = (u128::from(bytes.min(total)) * 1000) / u128::from(total);
    let bounded = u32::try_from(thousandths).unwrap_or(1000);
    f64::from(bounded) / 1000.0
}
