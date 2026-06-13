//! Responsive Ratatui dashboard rendering.

use std::cmp::Ordering;

use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Block, BorderType, Borders, Gauge, List, ListItem, Padding, Paragraph, Wrap,
};

use crate::format::{bytes, duration, percentage, speed};
use crate::model::{AppModel, OperationPhase, Screen};
use crate::theme;

const MIN_WIDTH: u16 = 40;
const MIN_HEIGHT: u16 = 10;

#[derive(Clone, Copy)]
enum Density {
    Wide,
    Medium,
    Compact,
}

/// Draw the current model.
pub fn draw(frame: &mut Frame<'_>, model: &AppModel) {
    let area = frame.area();
    if area.width < MIN_WIDTH || area.height < MIN_HEIGHT {
        render_too_small(frame, area);
        return;
    }

    let density = if area.width >= 100 && area.height >= 28 {
        Density::Wide
    } else if area.width >= 72 && area.height >= 20 {
        Density::Medium
    } else {
        Density::Compact
    };

    match density {
        Density::Compact => render_compact(frame, area, model),
        Density::Wide | Density::Medium => render_dashboard(frame, area, model, density),
    }
}

fn render_dashboard(frame: &mut Frame<'_>, area: Rect, model: &AppModel, density: Density) {
    let header_height = match density {
        Density::Wide => 7,
        _ => 4,
    };
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(header_height),
            Constraint::Length(3),
            Constraint::Min(8),
            Constraint::Length(if model.error.is_some() { 5 } else { 3 }),
        ])
        .split(area);

    render_header(frame, rows[0], model, density);
    render_timeline(frame, rows[1], model);

    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints(match density {
            Density::Wide => [Constraint::Percentage(68), Constraint::Percentage(32)],
            _ => [Constraint::Percentage(62), Constraint::Percentage(38)],
        })
        .split(rows[2]);

    if is_operation(model.screen) {
        render_operation(frame, columns[0], model);
        render_events(frame, columns[1], model);
    } else {
        render_wizard(frame, columns[0], model);
        render_context(frame, columns[1], model);
    }
    render_status(frame, rows[3], model);
}

fn render_compact(frame: &mut Frame<'_>, area: Rect, model: &AppModel) {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Min(3),
            Constraint::Length(if model.error.is_some() { 4 } else { 2 }),
        ])
        .split(area);

    let title = if let Some(phase) = model.operation_phase() {
        format!("rust-imager  |  {}", phase_name(phase))
    } else {
        format!("rust-imager  |  {}", screen_name(model.screen))
    };
    frame.render_widget(
        Paragraph::new(title)
            .style(theme::accent())
            .block(panel("")),
        rows[0],
    );
    render_timeline(frame, rows[1], model);
    if is_operation(model.screen) {
        render_operation(frame, rows[2], model);
    } else {
        render_wizard(frame, rows[2], model);
    }
    render_status(frame, rows[3], model);
}

fn render_header(frame: &mut Frame<'_>, area: Rect, model: &AppModel, density: Density) {
    let phase = model
        .operation_phase()
        .map_or_else(|| screen_name(model.screen), phase_name);
    let device = model.selected_device().map_or_else(
        || {
            model
                .operation_source
                .as_deref()
                .unwrap_or("No device selected")
        },
        |value| value.model.as_str(),
    );
    let text = match density {
        Density::Wide => vec![
            Line::from(Span::styled(
                " RUST IMAGER ",
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(vec![
                Span::styled("Fast SBC image acquisition", theme::accent()),
                Span::raw("  |  "),
                Span::styled(phase, theme::active()),
            ]),
            Line::from(vec![
                Span::styled("SOURCE  ", theme::muted()),
                Span::raw(device),
                Span::styled("    OUTPUT  ", theme::muted()),
                Span::raw(value_or_dash(&model.output)),
            ]),
        ],
        _ => vec![
            Line::from(vec![
                Span::styled("RUST IMAGER", theme::accent()),
                Span::raw("  |  "),
                Span::styled(phase, theme::active()),
            ]),
            Line::from(Span::styled(device, theme::muted())),
        ],
    };
    frame.render_widget(Paragraph::new(text).block(panel(" DASHBOARD ")), area);
}

fn render_timeline(frame: &mut Frame<'_>, area: Rect, model: &AppModel) {
    let (steps, current) = if let Some(phase) = model.operation_phase() {
        (
            ["INSPECT", "SHRINK", "EXTRACT", "VERIFY", "COMPLETE"],
            operation_index(phase),
        )
    } else {
        (
            ["DEVICE", "CONFIRM", "OUTPUT", "VERIFY", "REVIEW"],
            wizard_index(model.screen),
        )
    };

    let mut spans = Vec::new();
    for (index, step) in steps.iter().enumerate() {
        if index > 0 {
            spans.push(Span::styled(" -- ", theme::muted()));
        }
        let (marker, style) = match index.cmp(&current) {
            Ordering::Less => ("[x]", theme::complete()),
            Ordering::Equal => ("[>]", theme::active()),
            Ordering::Greater => ("[ ]", theme::muted()),
        };
        spans.push(Span::styled(format!("{marker} {step}"), style));
    }
    frame.render_widget(
        Paragraph::new(Line::from(spans))
            .alignment(Alignment::Center)
            .block(panel(" WORKFLOW ")),
        area,
    );
}

fn render_wizard(frame: &mut Frame<'_>, area: Rect, model: &AppModel) {
    match model.screen {
        Screen::DeviceSelection => render_device_selection(frame, area, model),
        Screen::ConfirmDevice => render_device_confirmation(frame, area, model),
        Screen::Output => frame.render_widget(
            Paragraph::new(vec![
                label_value("OUTPUT", value_or_dash(&model.output)),
                label_value("COMPRESSION", &format!("{:?}", model.compression)),
                Line::default(),
                Line::from(Span::styled(
                    "Only the compacted, useful disk range will be read.",
                    theme::muted(),
                )),
            ])
            .block(panel(" OUTPUT IMAGE ")),
            area,
        ),
        Screen::Verification => frame.render_widget(
            Paragraph::new(vec![
                label_value("VERIFY", &format!("{:?}", model.verification)),
                Line::default(),
                Line::from("The completed stream and metadata will be checked before success."),
            ])
            .block(panel(" VERIFICATION ")),
            area,
        ),
        Screen::Review => frame.render_widget(
            Paragraph::new(vec![
                Line::from(Span::styled("READY TO IMAGE", theme::warning())),
                Line::default(),
                label_value(
                    "SOURCE",
                    model
                        .selected_device()
                        .map_or("?", |value| value.path.as_str()),
                ),
                label_value("OUTPUT", value_or_dash(&model.output)),
                label_value("COMPRESSION", &format!("{:?}", model.compression)),
                label_value("VERIFY", &format!("{:?}", model.verification)),
            ])
            .block(panel(" REVIEW IMMUTABLE PLAN ")),
            area,
        ),
        _ => {}
    }
}

fn render_device_selection(frame: &mut Frame<'_>, area: Rect, model: &AppModel) {
    let items = if model.devices.is_empty() {
        vec![ListItem::new(Line::from(Span::styled(
            "No supported external /dev/sdX device found",
            theme::warning(),
        )))]
    } else {
        model
            .devices
            .iter()
            .enumerate()
            .map(|(index, device)| {
                let marker = if model.device_cursor == index {
                    ">"
                } else {
                    " "
                };
                ListItem::new(vec![
                    Line::from(vec![
                        Span::styled(format!("{marker} {:02}  ", index + 1), theme::accent()),
                        Span::styled(&device.model, Style::default().add_modifier(Modifier::BOLD)),
                    ]),
                    Line::from(vec![
                        Span::styled("    ", theme::muted()),
                        Span::raw(format!(
                            "{}  |  {} ({} bytes)  |  {}",
                            device.path,
                            bytes(device.size_bytes),
                            device.size_bytes,
                            device.transport
                        )),
                    ]),
                ])
            })
            .collect()
    };
    frame.render_widget(List::new(items).block(panel(" SELECT DEVICE ")), area);
}

fn render_device_confirmation(frame: &mut Frame<'_>, area: Rect, model: &AppModel) {
    let device = model.selected_device();
    let mut lines = Vec::new();
    if model.unknown_layout_warning {
        lines.push(Line::from(Span::styled(
            "STRONG WARNING: layout does not match a known Raspberry Pi or ODROID profile.",
            theme::warning(),
        )));
        lines.push(Line::default());
    }
    lines.extend([
        Line::from(Span::styled(
            "This operation shrinks and modifies the source eMMC.",
            theme::failure(),
        )),
        Line::default(),
        label_value("DEVICE", device.map_or("?", |value| value.path.as_str())),
        label_value("MODEL", device.map_or("?", |value| value.model.as_str())),
        Line::default(),
        label_value("TYPE MODEL", &model.confirmation),
    ]);
    frame.render_widget(
        Paragraph::new(lines)
            .block(panel(" CONFIRM SOURCE "))
            .wrap(Wrap { trim: false }),
        area,
    );
}

fn render_context(frame: &mut Frame<'_>, area: Rect, model: &AppModel) {
    let lines = if let Some(device) = model.selected_device() {
        let capacity = format!("{} ({} bytes)", bytes(device.size_bytes), device.size_bytes);
        let sector = format!("{} B", device.logical_sector_size);
        vec![
            label_value("PATH", &device.path),
            label_value("MODEL", &device.model),
            label_owned("CAPACITY", capacity),
            label_owned("SECTOR", sector),
            label_value("TRANSPORT", &device.transport),
            label_value("SERIAL", &device.serial),
        ]
    } else {
        vec![
            Line::from(Span::styled("SOURCE POLICY", theme::accent())),
            Line::default(),
            Line::from("External USB block devices"),
            Line::from("/dev/sdX whole-disk exposure"),
            Line::from("Root and mounted devices rejected"),
            Line::default(),
            Line::from(Span::styled(
                "Select a device to inspect its identity.",
                theme::muted(),
            )),
        ]
    };
    frame.render_widget(
        Paragraph::new(lines)
            .block(panel(" DEVICE CONTEXT "))
            .wrap(Wrap { trim: false }),
        area,
    );
}

fn render_operation(frame: &mut Frame<'_>, area: Rect, model: &AppModel) {
    let phase = model.operation_phase().unwrap_or(OperationPhase::Inspect);
    if phase == OperationPhase::Extract {
        render_extraction(frame, area, model);
        return;
    }

    let (headline, detail, style) = match phase {
        OperationPhase::Inspect => (
            "INSPECTING SOURCE",
            "Reading partition and filesystem metadata",
            theme::active(),
        ),
        OperationPhase::Shrink => (
            "DO NOT REMOVE POWER",
            "Filesystem and partition metadata are changing",
            theme::warning(),
        ),
        OperationPhase::Verify => (
            "VERIFYING IMAGE",
            "Decoding the output stream and writing metadata",
            theme::active(),
        ),
        OperationPhase::Complete => (
            "IMAGE COMPLETE",
            "Extraction and verification completed successfully",
            theme::complete(),
        ),
        OperationPhase::Extract => unreachable!(),
    };
    frame.render_widget(
        Paragraph::new(vec![
            Line::default(),
            Line::from(Span::styled(headline, style)),
            Line::default(),
            Line::from(detail),
            Line::default(),
            label_value("ELAPSED", &duration(model.elapsed())),
            label_value("OUTPUT", value_or_dash(&model.output)),
        ])
        .alignment(Alignment::Center)
        .block(panel(" OPERATION ")),
        area,
    );
}

fn render_extraction(frame: &mut Frame<'_>, area: Rect, model: &AppModel) {
    let metrics = model.progress_metrics();
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Min(1),
        ])
        .split(area);
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(Span::styled("EXTRACTING IMAGE", theme::active())),
            Line::from(format!(
                "{} / {}",
                bytes(metrics.bytes),
                bytes(metrics.total)
            )),
        ])
        .alignment(Alignment::Center)
        .block(panel(" OPERATION ")),
        rows[0],
    );
    frame.render_widget(
        Gauge::default()
            .block(panel(""))
            .gauge_style(
                Style::default()
                    .fg(Color::Cyan)
                    .bg(Color::Black)
                    .add_modifier(Modifier::BOLD),
            )
            .ratio(progress_ratio(metrics.bytes, metrics.total))
            .label(percentage(metrics.bytes, metrics.total)),
        rows[1],
    );
    let eta = metrics.eta.map_or_else(|| "--:--".into(), duration);
    let rate = metrics
        .recent_bytes_per_second
        .max(metrics.average_bytes_per_second);
    frame.render_widget(
        Paragraph::new(vec![
            label_value("SPEED", &speed(rate)),
            label_value("ELAPSED", &duration(model.elapsed())),
            label_value("ETA", &eta),
            label_value("OUTPUT", value_or_dash(&model.output)),
        ])
        .block(panel(" LIVE METRICS ")),
        rows[2],
    );
}

fn render_events(frame: &mut Frame<'_>, area: Rect, model: &AppModel) {
    let items: Vec<_> = model
        .recent_events()
        .iter()
        .rev()
        .map(|event| {
            ListItem::new(Line::from(vec![
                Span::styled(format!("{}  ", duration(event.elapsed)), theme::muted()),
                Span::raw(&event.message),
            ]))
        })
        .collect();
    frame.render_widget(List::new(items).block(panel(" RECENT EVENTS ")), area);
}

fn render_status(frame: &mut Frame<'_>, area: Rect, model: &AppModel) {
    if let Some(error) = &model.error {
        frame.render_widget(
            Paragraph::new(vec![
                Line::from(Span::styled("FAILED", theme::failure())),
                Line::from(error.as_str()),
            ])
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .border_style(theme::failure())
                    .padding(Padding::horizontal(1)),
            ),
            area,
        );
        return;
    }

    let help = if is_operation(model.screen) {
        "LIVE  |  Do not disconnect the source  |  Ctrl+C: request stop"
    } else {
        "Up/Down: navigate  |  Enter: continue  |  Esc: cancel"
    };
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(" READY ", theme::complete()),
            Span::styled(help, theme::muted()),
        ]))
        .block(panel("")),
        area,
    );
}

fn render_too_small(frame: &mut Frame<'_>, area: Rect) {
    frame.render_widget(
        Paragraph::new("rust-imager\nTerminal too small (minimum 40x10)")
            .alignment(Alignment::Center)
            .block(panel(""))
            .wrap(Wrap { trim: true }),
        area,
    );
}

fn panel(title: &'static str) -> Block<'static> {
    Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::DarkGray))
        .padding(Padding::horizontal(1))
}

fn label_value<'a>(label: &'a str, value: &'a str) -> Line<'a> {
    Line::from(vec![
        Span::styled(format!("{label:<12}"), theme::muted()),
        Span::raw(value),
    ])
}

fn label_owned(label: &str, value: String) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!("{label:<12}"), theme::muted()),
        Span::raw(value),
    ])
}

fn value_or_dash(value: &str) -> &str {
    if value.trim().is_empty() { "-" } else { value }
}

fn is_operation(screen: Screen) -> bool {
    matches!(
        screen,
        Screen::Preparing
            | Screen::Mutating
            | Screen::Extracting
            | Screen::Verifying
            | Screen::Complete
    )
}

fn screen_name(screen: Screen) -> &'static str {
    match screen {
        Screen::DeviceSelection => "SELECT DEVICE",
        Screen::ConfirmDevice => "CONFIRM SOURCE",
        Screen::Output => "OUTPUT",
        Screen::Verification => "VERIFICATION",
        Screen::Review => "REVIEW",
        Screen::Preparing => "INSPECT",
        Screen::Mutating => "SHRINK",
        Screen::Extracting => "EXTRACT",
        Screen::Verifying => "VERIFY",
        Screen::Complete => "COMPLETE",
    }
}

fn phase_name(phase: OperationPhase) -> &'static str {
    match phase {
        OperationPhase::Inspect => "INSPECT",
        OperationPhase::Shrink => "SHRINK",
        OperationPhase::Extract => "EXTRACT",
        OperationPhase::Verify => "VERIFY",
        OperationPhase::Complete => "COMPLETE",
    }
}

fn wizard_index(screen: Screen) -> usize {
    match screen {
        Screen::ConfirmDevice => 1,
        Screen::Output => 2,
        Screen::Verification => 3,
        Screen::Review => 4,
        Screen::DeviceSelection
        | Screen::Preparing
        | Screen::Mutating
        | Screen::Extracting
        | Screen::Verifying
        | Screen::Complete => 0,
    }
}

fn operation_index(phase: OperationPhase) -> usize {
    match phase {
        OperationPhase::Inspect => 0,
        OperationPhase::Shrink => 1,
        OperationPhase::Extract => 2,
        OperationPhase::Verify => 3,
        OperationPhase::Complete => 4,
    }
}

fn progress_ratio(bytes: u64, total: u64) -> f64 {
    if total == 0 {
        return 0.0;
    }
    let millionths = (u128::from(bytes.min(total)) * 1_000_000) / u128::from(total);
    f64::from(u32::try_from(millionths).unwrap_or(1_000_000)) / 1_000_000.0
}
