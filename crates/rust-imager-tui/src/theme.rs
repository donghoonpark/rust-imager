//! Shared dashboard styles.

use ratatui::style::{Color, Modifier, Style};

/// Primary product accent.
#[must_use]
pub fn accent() -> Style {
    Style::default()
        .fg(Color::Cyan)
        .add_modifier(Modifier::BOLD)
}

/// Active workflow state.
#[must_use]
pub fn active() -> Style {
    Style::default()
        .fg(Color::LightBlue)
        .add_modifier(Modifier::BOLD)
}

/// Completed workflow state.
#[must_use]
pub fn complete() -> Style {
    Style::default()
        .fg(Color::Green)
        .add_modifier(Modifier::BOLD)
}

/// Caution and destructive state.
#[must_use]
pub fn warning() -> Style {
    Style::default()
        .fg(Color::Yellow)
        .add_modifier(Modifier::BOLD)
}

/// Failure state.
#[must_use]
pub fn failure() -> Style {
    Style::default()
        .fg(Color::LightRed)
        .add_modifier(Modifier::BOLD)
}

/// Low-emphasis context.
#[must_use]
pub fn muted() -> Style {
    Style::default().fg(Color::DarkGray)
}
