//! Wizard reducer tests.

use ratatui::{Terminal, backend::TestBackend};
use rust_imager_core::device::DeviceIdentity;
use rust_imager_tui::model::{Action, AppModel, Screen};
use rust_imager_tui::view::draw;

fn device() -> DeviceIdentity {
    DeviceIdentity {
        path: "/dev/sda".into(),
        major_minor: "8:0".into(),
        serial: "ABC".into(),
        model: "eMMC Reader".into(),
        size_bytes: 64_000_000_000,
        logical_sector_size: 512,
        transport: "usb".into(),
        removable: true,
    }
}

#[test]
fn requires_exact_model_confirmation() {
    let mut model = AppModel::new(vec![device()]);
    model.reduce(Action::SelectDevice(0));
    assert_eq!(model.screen, Screen::ConfirmDevice);
    model.reduce(Action::SetConfirmation("wrong".into()));
    model.reduce(Action::Continue);
    assert_eq!(model.screen, Screen::ConfirmDevice);
    assert!(model.error.is_some());
    model.reduce(Action::SetConfirmation("eMMC Reader".into()));
    model.reduce(Action::Continue);
    assert_eq!(model.screen, Screen::Output);
}

#[test]
fn small_terminal_render_does_not_panic() {
    let backend = TestBackend::new(32, 8);
    let mut terminal = Terminal::new(backend).expect("terminal");
    let model = AppModel::new(vec![device()]);
    terminal
        .draw(|frame| draw(frame, &model))
        .expect("small render");
}

#[test]
fn retains_unknown_layout_warning_on_confirmation_screen() {
    let mut model = AppModel::new(vec![device()]);
    model.reduce(Action::SelectDevice(0));
    model.reduce(Action::SetUnknownLayoutWarning(true));
    assert_eq!(model.screen, Screen::ConfirmDevice);
    assert!(model.unknown_layout_warning);
}
