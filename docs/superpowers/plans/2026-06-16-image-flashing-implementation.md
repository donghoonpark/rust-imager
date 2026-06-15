# Image Flashing Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add safe streaming image flashing with optional pre-write and post-write verification, expose it through CLI and TUI, validate it on all supported Ubuntu LTS releases, and publish v0.3.0.

**Architecture:** Add pure flash policy types to core, input inspection and streaming write modules to the pipeline crate, and target quiescence/reread orchestration to the CLI application layer. Extend the TUI model with an operation mode and flash-specific setup screens while retaining the existing shared progress dashboard.

**Tech Stack:** Rust, Clap, Ratatui, SHA-256, zstd, xz2, Linux block devices, Docker Buildx, GitHub Actions

---

### Task 1: Flash Domain And Input Inspection

**Files:**
- Create: `crates/rust-imager-core/src/flash.rs`
- Modify: `crates/rust-imager-core/src/lib.rs`
- Create: `crates/rust-imager-pipeline/src/flash.rs`
- Modify: `crates/rust-imager-pipeline/src/lib.rs`
- Create: `crates/rust-imager-pipeline/tests/flash.rs`

- [ ] Write failing tests for extension detection, raw/decompressed length, MBR validation, sidecar absence, sidecar mismatch, and full hash verification.
- [ ] Run `cargo test -p rust-imager-pipeline --test flash` and confirm the tests fail because the flash API is absent.
- [ ] Implement `ImageFormat`, `PreVerify`, `PostVerify`, `inspect_image`, and a reusable decoder.
- [ ] Run the focused tests and all core/pipeline tests.
- [ ] Commit as `feat: inspect flash images`.

### Task 2: Bounded Streaming Writer

**Files:**
- Modify: `crates/rust-imager-pipeline/src/flash.rs`
- Modify: `crates/rust-imager-pipeline/tests/flash.rs`

- [ ] Write failing tests for exact writes, progress, target-too-small rejection, retained partial writes, and post-write reread mismatch.
- [ ] Run the focused test and confirm the expected failures.
- [ ] Implement a one MiB bounded copy loop with raw hashing, flush, `sync_all`, and optional exact-prefix reread.
- [ ] Run focused and pipeline tests plus Clippy.
- [ ] Commit as `feat: stream images to block targets`.

### Task 3: CLI Flash Engine

**Files:**
- Create: `crates/rust-imager-cli/src/flash.rs`
- Modify: `crates/rust-imager-cli/src/main.rs`
- Modify: `crates/rust-imager-cli/src/app.rs`
- Modify: `crates/rust-imager-cli/tests/cli.rs`
- Create: `tests/integration/full-flash.sh`

- [ ] Write failing CLI tests for `flash`, defaults, all verification values, and required exact model confirmation.
- [ ] Write a failing privileged integration test that flashes a compressed fixture to a second loop-backed `/dev/sdy` alias and compares the written prefix.
- [ ] Implement safe candidate selection, identity revalidation, quiescence, capacity checks, logging, streaming write, optional reread, and partition reread.
- [ ] Run CLI tests and the privileged loop test.
- [ ] Commit as `feat: add guarded flash command`.

### Task 4: Unified TUI Flow

**Files:**
- Modify: `crates/rust-imager-tui/src/model.rs`
- Modify: `crates/rust-imager-tui/src/view.rs`
- Modify: `crates/rust-imager-tui/tests/model.rs`
- Modify: `crates/rust-imager-tui/tests/render.rs`
- Modify: `crates/rust-imager-cli/src/wizard.rs`

- [ ] Write failing reducer and rendering tests for operation choice, image input, pre-verification, target selection, missing-sidecar warning, post-verification, destructive review, progress, failure, and completion.
- [ ] Run focused TUI tests and confirm the new states are absent.
- [ ] Add operation mode and flash screens, then connect keyboard handling and the worker-thread flash engine.
- [ ] Run TUI/CLI tests and visually render compact and full layouts.
- [ ] Commit as `feat: add flash workflow to TUI`.

### Task 5: Ubuntu LTS Flash Matrix

**Files:**
- Modify: `packaging/demo/entrypoint.sh`
- Modify: `packaging/demo/full-run.sh`
- Modify: `packaging/test-lts-loop-matrix.sh`
- Modify: `packaging/test-container-scripts.sh`
- Modify: `.github/workflows/release.yml`
- Modify: `docs/testing.md`

- [ ] Add failing shell contracts requiring a second target disk and a flash transaction.
- [ ] Extend the demo to create `/dev/sdy` as a larger second loop disk and refresh its partition aliases.
- [ ] Run create-image, flash with full pre/post verification, and byte-compare assertions in each LTS container.
- [ ] Run the arm64 matrix locally and validate the amd64/arm64 GitHub matrix definition.
- [ ] Commit as `ci: validate flashing across Ubuntu LTS`.

### Task 6: Documentation And v0.3.0 Release

**Files:**
- Modify: `README.md`
- Modify: `docs/safety.md`
- Modify: `CHANGELOG.md`
- Modify: workspace and internal `Cargo.toml` files
- Modify: `Cargo.lock`

- [ ] Document flash commands, verification semantics, missing-sidecar behavior, and partial-write recovery.
- [ ] Bump all workspace packages to 0.3.0 and update the changelog.
- [ ] Run format, Clippy, all tests, shell contracts, package build, and the four-LTS loop matrix.
- [ ] Commit as `chore: release v0.3.0`.
- [ ] Push main, tag `v0.3.0`, monitor all release jobs, and verify published packages and checksums.
