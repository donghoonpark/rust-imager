//! Pure wizard state and reducer.

use rust_imager_core::device::DeviceIdentity;
use rust_imager_core::plan::{Compression, VerificationLevel};

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
}

impl AppModel {
    /// Create initial wizard state.
    #[must_use]
    pub fn new(devices: Vec<DeviceIdentity>) -> Self {
        Self {
            screen: Screen::DeviceSelection,
            devices,
            selected: None,
            confirmation: String::new(),
            output: String::new(),
            compression: Compression::Zstandard { level: 3 },
            verification: VerificationLevel::Decode,
            error: None,
            unknown_layout_warning: false,
            progress_bytes: 0,
            progress_total: 0,
        }
    }

    /// Apply one action.
    pub fn reduce(&mut self, action: Action) {
        self.error = None;
        match action {
            Action::SelectDevice(index) if index < self.devices.len() => {
                self.selected = Some(index);
                self.confirmation.clear();
                self.screen = Screen::ConfirmDevice;
            }
            Action::SelectDevice(_) => self.error = Some("Invalid device selection".into()),
            Action::SetConfirmation(value) => self.confirmation = value,
            Action::SetOutput(value) => self.output = value,
            Action::SetCompression(value) => self.compression = value,
            Action::SetVerification(value) => self.verification = value,
            Action::Continue => self.continue_current(),
            Action::PreparationStarted => self.screen = Screen::Preparing,
            Action::MutationStarted => self.screen = Screen::Mutating,
            Action::ExtractionStarted => self.screen = Screen::Extracting,
            Action::VerificationStarted => self.screen = Screen::Verifying,
            Action::Progress { bytes, total } => {
                self.progress_bytes = bytes;
                self.progress_total = total;
            }
            Action::Finished => self.screen = Screen::Complete,
            Action::Failed(message) => self.error = Some(message),
            Action::SetUnknownLayoutWarning(value) => self.unknown_layout_warning = value,
        }
    }

    /// Selected device, if any.
    #[must_use]
    pub fn selected_device(&self) -> Option<&DeviceIdentity> {
        self.selected.and_then(|index| self.devices.get(index))
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
