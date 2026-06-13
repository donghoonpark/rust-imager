# Modern TUI Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the basic wizard rendering with a responsive modern dashboard that exposes safe workflow navigation, device context, honest phase status, extraction metrics, and persistent errors.

**Architecture:** Extend the pure TUI model with phases, timed progress samples, metrics, and recent events. Split Ratatui rendering into focused theme, formatting, wizard, and operation modules, then run the imaging engine on a worker thread so the main terminal loop can animate and redraw independently.

**Tech Stack:** Rust 1.94.1, Ratatui, Crossterm, standard-library channels and timing, Ratatui `TestBackend`

---

### Task 1: Operation phase and progress metrics

**Files:**
- Modify: `crates/rust-imager-tui/src/model.rs`
- Create: `crates/rust-imager-tui/src/format.rs`
- Modify: `crates/rust-imager-tui/src/lib.rs`
- Modify: `crates/rust-imager-tui/tests/model.rs`

- [ ] Add failing tests for five operation phases, phase completion, bounded
  recent events, progress clamping, average speed, recent speed, elapsed time,
  and ETA.
- [ ] Run `cargo test -p rust-imager-tui` and confirm the new tests fail because
  the phase and metric APIs do not exist.
- [ ] Add `OperationPhase`, timed progress samples, event records, `Tick`, and
  deterministic `reduce_at(action, instant)` support while retaining `reduce`.
- [ ] Add pure IEC byte, duration, percentage, speed, and ETA formatters.
- [ ] Run `cargo test -p rust-imager-tui` and Clippy until all model and
  formatter tests pass.
- [ ] Commit as `feat: add TUI operation metrics`.

### Task 2: Responsive dashboard renderer

**Files:**
- Create: `crates/rust-imager-tui/src/theme.rs`
- Create: `crates/rust-imager-tui/src/layout.rs`
- Create: `crates/rust-imager-tui/src/widgets.rs`
- Create: `crates/rust-imager-tui/src/wizard_view.rs`
- Create: `crates/rust-imager-tui/src/operation_view.rs`
- Modify: `crates/rust-imager-tui/src/view.rs`
- Modify: `crates/rust-imager-tui/src/lib.rs`
- Create: `crates/rust-imager-tui/tests/render.rs`

- [ ] Add failing TestBackend assertions for the ASCII brand, wizard stepper,
  highlighted device facts, strong warning, immutable review, operation
  timeline, progress metrics, event panel, contextual command bar, failure
  panel, and compact fallback.
- [ ] Run the render tests and confirm they fail against the current renderer.
- [ ] Implement cyan/blue/green/amber/red theme helpers and reusable card,
  badge, timeline, command-bar, and metric widgets.
- [ ] Implement wide, medium, and compact layout selection without coordinate
  arithmetic outside Ratatui layouts.
- [ ] Implement each wizard screen with explicit choices and human-readable
  values.
- [ ] Implement operation screens with indeterminate spinner phases and an
  extraction gauge containing percentage, bytes, speed, elapsed time, and ETA.
- [ ] Keep the selected device/output sidebar and unknown-layout warning visible
  where model data exists.
- [ ] Run all TUI tests at 120x36, 84x24, 56x16, 40x10, and 32x8 dimensions.
- [ ] Commit as `feat: render modern TUI dashboard`.

### Task 3: Interactive navigation polish

**Files:**
- Modify: `crates/rust-imager-tui/src/model.rs`
- Modify: `crates/rust-imager-cli/src/wizard.rs`
- Modify: `crates/rust-imager-tui/tests/model.rs`

- [ ] Add failing reducer tests for selected-list movement, compression choice,
  verification choice, back navigation before review, and contextual errors.
- [ ] Add arrow and `j`/`k` selection, Enter activation, visible current
  selections, Backspace editing, and Back/Escape semantics that never cross the
  mutation boundary.
- [ ] Preserve numeric shortcuts and exact model confirmation.
- [ ] Run CLI and TUI tests and manually render the setup wizard in the existing
  Ubuntu demo container.
- [ ] Commit as `feat: polish TUI wizard navigation`.

### Task 4: Asynchronous operation dashboard

**Files:**
- Modify: `crates/rust-imager-cli/src/wizard.rs`
- Modify: `crates/rust-imager-cli/src/app.rs` only if event labels require
  additional non-invasive context
- Create: `crates/rust-imager-cli/tests/operation_ui.rs`

- [ ] Add a failing test around a fake event producer proving redraw ticks occur
  between engine events and completion/error reaches the model.
- [ ] Extract an operation UI loop that receives engine events/results over an
  `mpsc` channel and redraws at a bounded interval.
- [ ] Run `run_image_with_progress` on a scoped worker thread while terminal
  input/rendering remains on the main thread.
- [ ] Record phase transition messages, progress samples, unknown-layout
  warning, completion, and full error text in the model.
- [ ] Keep failed and completed screens visible until Enter or Escape.
- [ ] Run CLI tests and the loop-backed integration test.
- [ ] Commit as `feat: animate TUI operation progress`.

### Task 5: Documentation and full verification

**Files:**
- Modify: `README.md`
- Modify: `CHANGELOG.md`
- Modify: `docs/testing.md`

- [ ] Document the modern dashboard, responsive terminal sizes, reported
  metrics, indeterminate phases, and keyboard controls.
- [ ] Run `cargo fmt --check`.
- [ ] Run `cargo clippy --workspace --all-targets --all-features -- -D warnings`.
- [ ] Run `cargo test --workspace --all-features`.
- [ ] Run `cargo build --workspace --release`.
- [ ] Run package shell tests and `actionlint` to ensure release packaging is
  unaffected.
- [ ] Launch the TUI in the Ubuntu Docker demo and visually inspect setup,
  warning, review, extraction, failure, compact, and completion states.
- [ ] Commit as `docs: document modern TUI workflow`.
