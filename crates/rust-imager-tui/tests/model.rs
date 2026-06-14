//! Wizard reducer tests.

use ratatui::{Terminal, backend::TestBackend};
use rust_imager_core::device::DeviceIdentity;
use rust_imager_tui::model::{Action, AppModel, OperationPhase, Screen};
use rust_imager_tui::view::draw;
use std::time::{Duration, Instant};

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

#[test]
fn tracks_operation_phases_and_recent_events() {
    let start = Instant::now();
    let mut model = AppModel::new(Vec::new());
    model.reduce_at(Action::PreparationStarted, start);
    assert_eq!(model.operation_phase(), Some(OperationPhase::Inspect));

    model.reduce_at(Action::MutationStarted, start + Duration::from_secs(2));
    model.reduce_at(Action::ExtractionStarted, start + Duration::from_secs(4));
    model.reduce_at(Action::VerificationStarted, start + Duration::from_secs(6));
    model.reduce_at(Action::Finished, start + Duration::from_secs(8));

    assert_eq!(model.operation_phase(), Some(OperationPhase::Complete));
    assert_eq!(model.elapsed(), Duration::from_secs(8));
    assert!(model.recent_events().len() <= 8);
    assert!(
        model
            .recent_events()
            .iter()
            .any(|event| event.message.contains("complete"))
    );
}

#[test]
fn computes_progress_speed_and_eta_from_timed_samples() {
    let start = Instant::now();
    let mut model = AppModel::new(Vec::new());
    model.reduce_at(Action::ExtractionStarted, start);
    model.reduce_at(
        Action::Progress {
            bytes: 1_000,
            total: 10_000,
        },
        start + Duration::from_secs(1),
    );
    model.reduce_at(
        Action::Progress {
            bytes: 5_000,
            total: 10_000,
        },
        start + Duration::from_secs(5),
    );

    let metrics = model.progress_metrics();
    assert_eq!(metrics.bytes, 5_000);
    assert_eq!(metrics.total, 10_000);
    assert_eq!(metrics.average_bytes_per_second, 1_000);
    assert_eq!(metrics.recent_bytes_per_second, 1_000);
    assert_eq!(metrics.eta, Some(Duration::from_secs(5)));
}

#[test]
fn clamps_progress_and_bounds_event_history() {
    let start = Instant::now();
    let mut model = AppModel::new(Vec::new());
    model.reduce_at(Action::ExtractionStarted, start);
    for index in 0..20 {
        model.reduce_at(
            Action::Progress {
                bytes: 100 + index,
                total: 100,
            },
            start + Duration::from_secs(index + 1),
        );
    }

    assert_eq!(model.progress_metrics().bytes, 100);
    assert!(model.recent_events().len() <= 8);
}

#[test]
fn moves_device_cursor_without_confirming_selection() {
    let mut second = device();
    second.path = "/dev/sdb".into();
    let mut model = AppModel::new(vec![device(), second]);

    assert_eq!(model.device_cursor, 0);
    model.reduce(Action::MoveDeviceCursor(1));
    assert_eq!(model.device_cursor, 1);
    assert_eq!(model.selected, None);

    model.reduce(Action::MoveDeviceCursor(1));
    assert_eq!(model.device_cursor, 0);
    model.reduce(Action::MoveDeviceCursor(-1));
    assert_eq!(model.device_cursor, 1);
}

#[test]
fn ticks_elapsed_time_without_dismissing_failure() {
    let start = Instant::now();
    let mut model = AppModel::new(Vec::new());
    model.reduce_at(Action::PreparationStarted, start);
    model.reduce_at(
        Action::Failed("USB device disconnected".into()),
        start + Duration::from_secs(1),
    );
    model.reduce_at(Action::Tick, start + Duration::from_secs(5));

    assert_eq!(model.elapsed(), Duration::from_secs(5));
    assert_eq!(model.error.as_deref(), Some("USB device disconnected"));
}

fn output_model(path: &str, compression: rust_imager_core::plan::Compression) -> AppModel {
    let mut model = AppModel::new(Vec::new());
    model.screen = Screen::Output;
    model.output = path.into();
    model.compression = compression;
    model
}

#[test]
fn completes_extensionless_zstd_output_with_image_suffix() {
    let mut model = output_model(
        "/output/board",
        rust_imager_core::plan::Compression::Zstandard { level: 3 },
    );
    model.reduce(Action::Continue);

    assert_eq!(model.output, "/output/board.img.zst");
    assert_eq!(model.screen, Screen::Verification);
}

#[test]
fn completes_img_output_with_selected_compression_suffix() {
    let mut model = output_model(
        "/output/board.img",
        rust_imager_core::plan::Compression::Zstandard { level: 3 },
    );
    model.reduce(Action::Continue);

    assert_eq!(model.output, "/output/board.img.zst");
}

#[test]
fn preserves_matching_output_extension() {
    let mut model = output_model(
        "/output/board.img.zst",
        rust_imager_core::plan::Compression::Zstandard { level: 3 },
    );
    model.reduce(Action::Continue);

    assert_eq!(model.output, "/output/board.img.zst");
}

#[test]
fn completes_extensionless_xz_output() {
    let mut model = output_model(
        "/output/board",
        rust_imager_core::plan::Compression::Xz { level: 3 },
    );
    model.reduce(Action::Continue);

    assert_eq!(model.output, "/output/board.img.xz");
}

#[test]
fn preserves_conflicting_output_extension_for_engine_validation() {
    let mut model = output_model(
        "/output/board.raw",
        rust_imager_core::plan::Compression::Zstandard { level: 3 },
    );
    model.reduce(Action::Continue);

    assert_eq!(model.output, "/output/board.raw");
}
