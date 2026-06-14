//! Responsive dashboard rendering tests.

use ratatui::{Terminal, backend::TestBackend};
use rust_imager_core::device::DeviceIdentity;
use rust_imager_tui::model::{Action, AppModel, OperationResult, PlanPreview, Screen};
use rust_imager_tui::view::draw;
use std::time::{Duration, Instant};

fn device() -> DeviceIdentity {
    DeviceIdentity {
        path: "/dev/sda".into(),
        major_minor: "8:0".into(),
        serial: "ABC".into(),
        model: "Virtual eMMC Reader".into(),
        size_bytes: 64_000_000_000,
        logical_sector_size: 512,
        transport: "usb".into(),
        removable: true,
    }
}

fn render(model: &AppModel, width: u16, height: u16) -> String {
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).expect("terminal");
    terminal.draw(|frame| draw(frame, model)).expect("draw");
    terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(ratatui::buffer::Cell::symbol)
        .collect()
}

#[test]
fn wide_wizard_has_brand_stepper_and_device_context() {
    let model = AppModel::new(vec![device()]);
    let content = render(&model, 120, 36);
    assert!(content.contains("RUST IMAGER"));
    assert!(content.contains("DEVICE"));
    assert!(content.contains("CONFIRM"));
    assert!(content.contains("Virtual eMMC Reader"));
    assert!(content.contains("64"));
    assert!(content.contains("SELECT DEVICE"));
}

#[test]
fn operation_dashboard_has_timeline_metrics_and_events() {
    let start = Instant::now();
    let mut model = AppModel::new(Vec::new());
    model.output = "/images/board.img.zst".into();
    model.reduce_at(Action::PreparationStarted, start);
    model.reduce_at(Action::MutationStarted, start + Duration::from_secs(1));
    model.reduce_at(Action::ExtractionStarted, start + Duration::from_secs(2));
    model.reduce_at(
        Action::Progress {
            bytes: 5_000_000,
            total: 10_000_000,
        },
        start + Duration::from_secs(7),
    );
    let content = render(&model, 120, 36);
    assert!(content.contains("OPERATION"));
    assert!(content.contains("INSPECT"));
    assert!(content.contains("EXTRACT"));
    assert!(content.contains("50.0%"));
    assert!(content.contains("ETA"));
    assert!(content.contains("RECENT EVENTS"));
}

#[test]
fn compact_layout_keeps_phase_and_progress() {
    let start = Instant::now();
    let mut model = AppModel::new(Vec::new());
    model.reduce_at(Action::ExtractionStarted, start);
    model.reduce_at(
        Action::Progress { bytes: 1, total: 2 },
        start + Duration::from_secs(1),
    );
    let content = render(&model, 56, 16);
    assert!(content.contains("rust-imager"));
    assert!(content.contains("EXTRACT"));
    assert!(content.contains("50.0%"));
}

#[test]
fn failure_is_prominent_and_persistent() {
    let mut model = AppModel::new(Vec::new());
    model.reduce(Action::PreparationStarted);
    model.reduce(Action::Failed("USB device disconnected".into()));
    let content = render(&model, 84, 24);
    assert!(content.contains("FAILED"));
    assert!(content.contains("USB device disconnected"));
}

#[test]
fn review_prioritizes_plan_sizes_and_destructive_target() {
    let mut model = AppModel::new(vec![device()]);
    model.reduce(Action::SelectDevice(0));
    model.screen = Screen::Review;
    model.output = "/images/board.img.zst".into();
    model.plan_preview = Some(PlanPreview {
        source_size_bytes: 64_000_000_000,
        current_filesystem_bytes: 8_000_000_000,
        target_filesystem_bytes: 3_000_000_000,
        image_bytes: 3_500_000_000,
        output_available_bytes: 100_000_000_000,
        root_partition: "/dev/sda2".into(),
    });

    let content = render(&model, 120, 36);
    assert!(content.contains("DESTRUCTIVE TARGET"));
    assert!(content.contains("/dev/sda2"));
    assert!(content.contains("CURRENT EXT4"));
    assert!(content.contains("PLANNED EXT4"));
    assert!(content.contains("RAW IMAGE RANGE"));
    assert!(content.contains("FIRST IRREVERSIBLE"));
}

#[test]
fn completion_shows_artifacts_sizes_hash_and_ratio() {
    let mut model = AppModel::new(Vec::new());
    model.reduce(Action::PreparationStarted);
    model.reduce(Action::SetOperationResult(OperationResult {
        output: "/images/board.img.zst".into(),
        raw_bytes: 10_000_000,
        compressed_bytes: 2_500_000,
        compressed_sha256: "abc123".into(),
        verification: "passed".into(),
        metadata_path: "/images/board.img.zst.json".into(),
        log_path: "/images/board.img.zst.log".into(),
    }));
    model.reduce(Action::Finished);

    let content = render(&model, 120, 36);
    assert!(content.contains("IMAGE COMPLETE"));
    assert!(content.contains("COMPRESSED"));
    assert!(content.contains("25.0%"));
    assert!(content.contains("abc123"));
    assert!(content.contains("board.img.zst.json"));
    assert!(content.contains("board.img.zst.log"));
}
