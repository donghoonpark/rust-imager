# TUI UX and Full Demo Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add the six high-priority TUI improvements, create a full loop-backed Docker demo, and release v0.2.3.

**Architecture:** Extend the pure TUI model with navigation and display facts, expose a read-only CLI preflight that reuses engine planning, enrich completion events, and keep destructive revalidation unchanged. Replace demo symlinks with real block nodes that preserve `/dev/sdz*` paths while using loop-device major/minor identities.

**Tech Stack:** Rust, Ratatui, Crossterm, Linux loop devices, Bash, Docker, GitHub Actions

---

### Task 1: Reversible Setup and Output Defaults

**Files:**
- Modify: `crates/rust-imager-tui/src/model.rs`
- Modify: `crates/rust-imager-tui/tests/model.rs`
- Modify: `crates/rust-imager-cli/src/wizard.rs`

- [ ] Add failing tests for default output assignment and backward screen transitions.
- [ ] Add model actions for previous-screen navigation and output provenance.
- [ ] Generate the timestamped default after device confirmation.
- [ ] Map `Left` and `BackTab` to previous-screen navigation.
- [ ] Run TUI and CLI tests and commit.

### Task 2: Output Conflict Preflight

**Files:**
- Modify: `crates/rust-imager-cli/src/app.rs`
- Modify: `crates/rust-imager-cli/src/wizard.rs`
- Modify: `crates/rust-imager-tui/src/model.rs`
- Modify: `crates/rust-imager-tui/tests/model.rs`

- [ ] Add failing tests for occupied final, partial, sidecar, and log paths.
- [ ] Expose a non-mutating output conflict query from the engine.
- [ ] Keep the wizard on Output and display the exact conflict.
- [ ] Run focused tests and commit.

### Task 3: Read-Only Plan Preview and Risk Review

**Files:**
- Modify: `crates/rust-imager-cli/src/app.rs`
- Modify: `crates/rust-imager-cli/src/wizard.rs`
- Modify: `crates/rust-imager-tui/src/model.rs`
- Modify: `crates/rust-imager-tui/src/view.rs`
- Modify: `crates/rust-imager-tui/tests/model.rs`
- Modify: `crates/rust-imager-tui/tests/render.rs`

- [ ] Add failing model/render tests for capacity, shrink target, image range,
      free space, root partition, and destructive warning.
- [ ] Add a public read-only preview DTO and function that reuse analysis and
      `build_plan`.
- [ ] Populate preview facts before entering Review.
- [ ] Render a risk-first Review screen.
- [ ] Run focused tests and commit.

### Task 4: Completion Summary

**Files:**
- Modify: `crates/rust-imager-cli/src/app.rs`
- Modify: `crates/rust-imager-cli/src/wizard.rs`
- Modify: `crates/rust-imager-tui/src/model.rs`
- Modify: `crates/rust-imager-tui/src/view.rs`
- Modify: tests in CLI and TUI crates

- [ ] Add failing tests for result facts and compression ratio rendering.
- [ ] Enrich `EngineEvent::Complete` with immutable result facts.
- [ ] Store and render output, raw/compressed sizes, hash, verification,
      metadata path, log path, and elapsed duration.
- [ ] Run focused tests and commit.

### Task 5: Full Docker Demo

**Files:**
- Create: `packaging/demo/Dockerfile`
- Create: `packaging/demo/entrypoint.sh`
- Create: `packaging/demo/full-run.sh`
- Create: `packaging/demo/launch-ghostty.sh`
- Modify: `docs/testing.md`

- [ ] Reproduce the symlink/sysfs failure in an automated shell check.
- [ ] Create `/dev/sdz*` block nodes from loop major/minor values.
- [ ] Make fake topology report real sysfs names and product-facing paths.
- [ ] Run a complete non-interactive shrink/extract/verify transaction in the
      demo image.
- [ ] Launch a fresh interactive fixture in Ghostty and commit.

### Task 6: Release v0.2.3

**Files:**
- Modify: workspace and crate manifests, `Cargo.lock`, `CHANGELOG.md`

- [ ] Bump all workspace/internal dependency versions to `0.2.3`.
- [ ] Run format, Clippy, workspace tests, release contract tests, release
      build, and Docker full-run validation.
- [ ] Commit, push `main`, tag `v0.2.3`, and monitor all GitHub Actions jobs.
- [ ] Confirm release assets and relaunch Ghostty using the released arm64
      package.
