# Automatic Output Extension Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Complete extensionless TUI output paths using the selected compression format and publish v0.2.2.

**Architecture:** Add output normalization to the pure TUI reducer while retaining strict validation in the CLI engine. Verify with reducer tests, then bump all workspace package versions and run the established release pipeline.

**Tech Stack:** Rust, Ratatui model tests, Cargo workspace, GitHub Actions, Docker, Ghostty

---

### Task 1: Output Path Normalization

**Files:**
- Modify: `crates/rust-imager-tui/tests/model.rs`
- Modify: `crates/rust-imager-tui/src/model.rs`

- [ ] Add failing reducer tests for extensionless, `.img`, matching, XZ, and conflicting paths.
- [ ] Run `cargo test -p rust-imager-tui --test model output` and confirm failure.
- [ ] Add a focused helper that appends `.img.zst`, `.img.xz`, `.zst`, or `.xz` only when no conflicting extension exists.
- [ ] Run TUI tests and Clippy.
- [ ] Commit as `feat: complete TUI output extensions`.

### Task 2: Release v0.2.2

**Files:**
- Modify: `Cargo.toml`
- Modify: `Cargo.lock`
- Modify: `crates/*/Cargo.toml`
- Modify: `CHANGELOG.md`

- [ ] Set workspace and internal dependency versions to `0.2.2`.
- [ ] Document output extension completion in the changelog.
- [ ] Run workspace format, Clippy, tests, release contract tests, and release build.
- [ ] Commit as `chore: release v0.2.2`.
- [ ] Push `main` and annotated tag `v0.2.2`.
- [ ] Confirm all package compatibility jobs and GitHub Release publication.

### Task 3: Refresh Docker Demo

**Files:**
- Rebuild: `/tmp/rust-imager-demo`

- [ ] Download the v0.2.2 arm64 release artifact.
- [ ] Rebuild the Ubuntu 24.04 demo image with the released package.
- [ ] Verify the TUI accepts `/output/demo` and shows `/output/demo.img.zst` on the next screen.
- [ ] Stop the old demo session and launch the v0.2.2 image in a new Ghostty window.
