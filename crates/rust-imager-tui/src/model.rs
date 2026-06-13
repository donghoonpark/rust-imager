//! Pure wizard state and reducer.

use std::collections::VecDeque;
use std::time::{Duration, Instant};

use rust_imager_core::device::DeviceIdentity;
use rust_imager_core::plan::{Compression, VerificationLevel};

const MAX_RECENT_EVENTS: usize = 8;
const MAX_PROGRESS_SAMPLES: usize = 8;

/// High-level operation phase.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperationPhase {
    /// Device inspection and immutable planning.
    Inspect,
    /// Destructive filesystem and partition shrink.
    Shrink,
    /// Reading and compressing the useful disk range.
    Extract,
    /// Image verification and metadata creation.
    Verify,
    /// Successful completion.
    Complete,
}

/// A recent operation event.
#[derive(Debug, Clone)]
pub struct OperationEvent {
    /// Time since the operation began.
    pub elapsed: Duration,
    /// User-facing event message.
    pub message: String,
}

/// Derived extraction metrics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProgressMetrics {
    /// Processed bytes, clamped to total.
    pub bytes: u64,
    /// Planned bytes.
    pub total: u64,
    /// Average rate since extraction began.
    pub average_bytes_per_second: u64,
    /// Rate between the oldest and newest retained samples.
    pub recent_bytes_per_second: u64,
    /// Estimated remaining duration.
    pub eta: Option<Duration>,
}

#[derive(Debug, Clone, Copy)]
struct ProgressSample {
    at: Instant,
    bytes: u64,
}

/// Wizard screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    /// Candidate selection.
    DeviceSelection,
    /// Destructive device model confirmation.
    ConfirmDevice,
    /// Output and compression configuration.
    Output,
    /// Verification configuration.
    Verification,
    /// Final immutable plan review.
    Review,
    /// Device and filesystem analysis is running.
    Preparing,
    /// Destructive shrink is running.
    Mutating,
    /// Extraction is running.
    Extracting,
    /// Completed image verification is running.
    Verifying,
    /// Operation completed.
    Complete,
}

/// User or engine action applied to the model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    /// Move the device cursor, wrapping at list boundaries.
    MoveDeviceCursor(i8),
    /// Select a candidate by index.
    SelectDevice(usize),
    /// Replace typed confirmation.
    SetConfirmation(String),
    /// Replace output path.
    SetOutput(String),
    /// Select compression.
    SetCompression(Compression),
    /// Select verification.
    SetVerification(VerificationLevel),
    /// Advance after validating the current screen.
    Continue,
    /// Mutation started.
    MutationStarted,
    /// Preflight analysis started.
    PreparationStarted,
    /// Extraction started.
    ExtractionStarted,
    /// Verification started.
    VerificationStarted,
    /// Update raw byte progress.
    Progress {
        /// Bytes consumed.
        bytes: u64,
        /// Total bytes.
        total: u64,
    },
    /// Operation completed successfully.
    Finished,
    /// Show an actionable error.
    Failed(String),
    /// Record whether the selected layout needs a strong warning.
    SetUnknownLayoutWarning(bool),
    /// Advance wall-clock metrics without changing state.
    Tick,
}

/// Complete serializable-independent UI state.
#[derive(Debug, Clone)]
pub struct AppModel {
    /// Current screen.
    pub screen: Screen,
    /// Safe candidate devices.
    pub devices: Vec<DeviceIdentity>,
    /// Selected candidate index.
    pub selected: Option<usize>,
    /// Highlighted candidate index.
    pub device_cursor: usize,
    /// Source path retained while the operation dashboard is running.
    pub operation_source: Option<String>,
    /// Typed model confirmation.
    pub confirmation: String,
    /// Final output path.
    pub output: String,
    /// Compression policy.
    pub compression: Compression,
    /// Verification policy.
    pub verification: VerificationLevel,
    /// Latest error message.
    pub error: Option<String>,
    /// Whether the selected layout is structurally safe but unrecognized.
    pub unknown_layout_warning: bool,
    /// Bytes processed.
    pub progress_bytes: u64,
    /// Planned bytes.
    pub progress_total: u64,
    operation_phase: Option<OperationPhase>,
    operation_started_at: Option<Instant>,
    extraction_started_at: Option<Instant>,
    updated_at: Option<Instant>,
    progress_samples: VecDeque<ProgressSample>,
    recent_events: VecDeque<OperationEvent>,
}

impl AppModel {
    /// Create initial wizard state.
    #[must_use]
    pub fn new(devices: Vec<DeviceIdentity>) -> Self {
        Self {
            screen: Screen::DeviceSelection,
            devices,
            selected: None,
            device_cursor: 0,
            operation_source: None,
            confirmation: String::new(),
            output: String::new(),
            compression: Compression::Zstandard { level: 3 },
            verification: VerificationLevel::Decode,
            error: None,
            unknown_layout_warning: false,
            progress_bytes: 0,
            progress_total: 0,
            operation_phase: None,
            operation_started_at: None,
            extraction_started_at: None,
            updated_at: None,
            progress_samples: VecDeque::new(),
            recent_events: VecDeque::new(),
        }
    }

    /// Apply one action.
    pub fn reduce(&mut self, action: Action) {
        self.reduce_at(action, Instant::now());
    }

    /// Apply one action at a supplied instant.
    pub fn reduce_at(&mut self, action: Action, now: Instant) {
        if !matches!(action, Action::Tick) {
            self.error = None;
        }
        self.updated_at = Some(now);
        match action {
            Action::MoveDeviceCursor(delta) => self.move_device_cursor(delta),
            Action::SelectDevice(index) if index < self.devices.len() => {
                self.selected = Some(index);
                self.device_cursor = index;
                self.confirmation.clear();
                self.screen = Screen::ConfirmDevice;
            }
            Action::SelectDevice(_) => self.error = Some("Invalid device selection".into()),
            Action::SetConfirmation(value) => self.confirmation = value,
            Action::SetOutput(value) => self.output = value,
            Action::SetCompression(value) => self.compression = value,
            Action::SetVerification(value) => self.verification = value,
            Action::Continue => self.continue_current(),
            Action::PreparationStarted => {
                self.start_operation(now);
                self.screen = Screen::Preparing;
                self.set_phase(OperationPhase::Inspect, now, "Inspecting source device");
            }
            Action::MutationStarted => {
                self.screen = Screen::Mutating;
                self.set_phase(
                    OperationPhase::Shrink,
                    now,
                    "Shrinking filesystem and partition",
                );
            }
            Action::ExtractionStarted => {
                self.screen = Screen::Extracting;
                if self.operation_phase != Some(OperationPhase::Extract) {
                    self.extraction_started_at = Some(now);
                    self.progress_samples.clear();
                    self.set_phase(OperationPhase::Extract, now, "Extracting compressed image");
                }
            }
            Action::VerificationStarted => {
                self.screen = Screen::Verifying;
                self.set_phase(OperationPhase::Verify, now, "Verifying image and metadata");
            }
            Action::Progress { bytes, total } => {
                self.progress_bytes = bytes.min(total);
                self.progress_total = total;
                self.push_progress_sample(now, self.progress_bytes);
            }
            Action::Finished => {
                self.screen = Screen::Complete;
                self.set_phase(OperationPhase::Complete, now, "Imaging complete");
            }
            Action::Failed(message) => {
                self.push_event(now, format!("Failed: {message}"));
                self.error = Some(message);
            }
            Action::SetUnknownLayoutWarning(value) => {
                self.unknown_layout_warning = value;
                if value && self.operation_started_at.is_some() {
                    self.push_event(now, "Warning: unrecognized SBC layout".into());
                }
            }
            Action::Tick => {}
        }
    }

    /// Selected device, if any.
    #[must_use]
    pub fn selected_device(&self) -> Option<&DeviceIdentity> {
        self.selected.and_then(|index| self.devices.get(index))
    }

    /// Current operation phase.
    #[must_use]
    pub fn operation_phase(&self) -> Option<OperationPhase> {
        self.operation_phase
    }

    /// Time elapsed since operation start.
    #[must_use]
    pub fn elapsed(&self) -> Duration {
        match (self.operation_started_at, self.updated_at) {
            (Some(start), Some(updated)) => updated.saturating_duration_since(start),
            _ => Duration::ZERO,
        }
    }

    /// Most recent operation events.
    #[must_use]
    pub fn recent_events(&self) -> &VecDeque<OperationEvent> {
        &self.recent_events
    }

    /// Current extraction metrics.
    #[must_use]
    pub fn progress_metrics(&self) -> ProgressMetrics {
        let average = rate_between(
            self.extraction_started_at,
            self.updated_at,
            0,
            self.progress_bytes,
        );
        let recent = match (self.progress_samples.front(), self.progress_samples.back()) {
            (Some(first), Some(last)) => {
                rate_between(Some(first.at), Some(last.at), first.bytes, last.bytes)
            }
            _ => 0,
        };
        let rate = recent.max(average);
        let remaining = self.progress_total.saturating_sub(self.progress_bytes);
        let eta =
            (rate > 0 && remaining > 0).then(|| Duration::from_secs(remaining.div_ceil(rate)));
        ProgressMetrics {
            bytes: self.progress_bytes,
            total: self.progress_total,
            average_bytes_per_second: average,
            recent_bytes_per_second: recent,
            eta,
        }
    }

    fn start_operation(&mut self, now: Instant) {
        if self.operation_started_at.is_none() {
            self.operation_started_at = Some(now);
        }
    }

    fn move_device_cursor(&mut self, delta: i8) {
        let len = self.devices.len();
        if len == 0 || delta == 0 {
            return;
        }
        self.device_cursor = if delta > 0 {
            (self.device_cursor + 1) % len
        } else if self.device_cursor == 0 {
            len - 1
        } else {
            self.device_cursor - 1
        };
    }

    fn set_phase(&mut self, phase: OperationPhase, now: Instant, message: &str) {
        self.start_operation(now);
        self.operation_phase = Some(phase);
        self.push_event(now, message.into());
    }

    fn push_progress_sample(&mut self, now: Instant, bytes: u64) {
        self.progress_samples
            .push_back(ProgressSample { at: now, bytes });
        while self.progress_samples.len() > MAX_PROGRESS_SAMPLES {
            self.progress_samples.pop_front();
        }
    }

    fn push_event(&mut self, now: Instant, message: String) {
        let elapsed = self
            .operation_started_at
            .map_or(Duration::ZERO, |start| now.saturating_duration_since(start));
        self.recent_events
            .push_back(OperationEvent { elapsed, message });
        while self.recent_events.len() > MAX_RECENT_EVENTS {
            self.recent_events.pop_front();
        }
    }

    fn continue_current(&mut self) {
        match self.screen {
            Screen::ConfirmDevice => {
                let expected = self.selected_device().map(|device| device.model.clone());
                if expected.as_deref() == Some(self.confirmation.trim()) {
                    self.screen = Screen::Output;
                } else {
                    self.error = Some("Type the device model exactly to continue".into());
                }
            }
            Screen::Output if self.output.trim().is_empty() => {
                self.error = Some("Choose a local output path".into());
            }
            Screen::Output => self.screen = Screen::Verification,
            Screen::Verification => self.screen = Screen::Review,
            _ => {}
        }
    }
}

fn rate_between(
    start: Option<Instant>,
    end: Option<Instant>,
    start_bytes: u64,
    end_bytes: u64,
) -> u64 {
    let (Some(start), Some(end)) = (start, end) else {
        return 0;
    };
    let elapsed = end.saturating_duration_since(start);
    if elapsed.is_zero() {
        return 0;
    }
    let bytes = end_bytes.saturating_sub(start_bytes);
    let nanos = elapsed.as_nanos();
    let rate = u128::from(bytes).saturating_mul(1_000_000_000) / nanos;
    u64::try_from(rate).unwrap_or(u64::MAX)
}
