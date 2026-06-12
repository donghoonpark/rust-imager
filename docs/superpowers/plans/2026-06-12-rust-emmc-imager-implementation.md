# Rust eMMC Imager Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a Linux-only Rust TUI that safely shrinks supported external MBR/FAT/ext4 eMMC devices, streams the useful range to Zstandard or XZ, verifies the result, and installs a first-boot systemd root expansion service.

**Architecture:** A Rust workspace separates pure analysis and planning from privileged Linux adapters. Immutable plans are produced before mutation; all external commands run through typed argv-based runners; extraction uses a bounded reader/hash/compressor/writer pipeline; the TUI consumes engine events without owning destructive logic.

**Tech Stack:** Rust 2024 edition, Ratatui/Crossterm, Serde, Tokio, tracing, thiserror, sha2, zstd, xz2, nix/libc, assert_cmd, tempfile, proptest, GitHub Actions, Linux loop/NBD/device-mapper, QEMU.

---

## File Map

- `Cargo.toml`: workspace metadata and shared dependency versions.
- `crates/rust-imager-core/`: pure domain types, MBR parser, profiles, planner, verification metadata.
- `crates/rust-imager-linux/`: Linux device discovery, process runner, mount/filesystem/partition operations.
- `crates/rust-imager-pipeline/`: bounded extraction, compression, hashing and verification.
- `crates/rust-imager-first-boot/`: generated systemd unit/script assets and installer.
- `crates/rust-imager-tui/`: wizard state and Ratatui rendering.
- `crates/rust-imager-cli/`: executable composition, logging and exit behavior.
- `tests/fixtures/`: small deterministic partition/filesystem fixtures and generators.
- `tests/integration/`: privileged Linux integration scripts.
- `tests/qemu/`: first-boot expansion VM harness.
- `.github/workflows/`: fast checks and heavy Linux/QEMU validation.

### Task 1: Workspace and Quality Baseline

**Files:**
- Create: `Cargo.toml`
- Create: `rust-toolchain.toml`
- Create: `deny.toml`
- Create: `.cargo/config.toml`
- Create: `crates/*/Cargo.toml`
- Create: `crates/*/src/lib.rs`
- Create: `crates/rust-imager-cli/src/main.rs`
- Create: `.github/workflows/ci.yml`

- [ ] Write a smoke test that invokes the CLI with `--version` and initially fails because no binary exists.
- [ ] Create the seven-crate workspace, package metadata, release profile, lint configuration and minimal binary.
- [ ] Run `cargo fmt --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`, and `cargo test --workspace`.
- [ ] Commit as `build: scaffold rust imager workspace`.

### Task 2: Domain Types and MBR Parser

**Files:**
- Create: `crates/rust-imager-core/src/device.rs`
- Create: `crates/rust-imager-core/src/mbr.rs`
- Create: `crates/rust-imager-core/src/error.rs`
- Create: `crates/rust-imager-core/tests/mbr.rs`

- [ ] Add failing tests for valid primary partitions, bad signature, protective `0xEE`, GPT header at LBA 1, overlap, overflow, logical/extended partitions, and partition bounds.
- [ ] Implement sector-safe MBR parsing with checked arithmetic and typed rejection reasons.
- [ ] Add property tests proving arbitrary 512-byte sectors never panic.
- [ ] Run core tests and workspace checks.
- [ ] Commit as `feat: parse and validate supported mbr layouts`.

### Task 3: Device Discovery and System-Disk Exclusion

**Files:**
- Create: `crates/rust-imager-linux/src/command.rs`
- Create: `crates/rust-imager-linux/src/discovery.rs`
- Create: `crates/rust-imager-linux/tests/discovery.rs`

- [ ] Add failing tests using captured `lsblk --json` and `findmnt --json` fixtures for USB `/dev/sdX`, partitions, root disk, boot disk, swap, non-USB and changed identity.
- [ ] Implement typed command execution without a shell and Serde-based machine output parsing.
- [ ] Implement candidate filtering and identity revalidation by path, major/minor, serial and size.
- [ ] Run Linux crate tests and workspace checks.
- [ ] Commit as `feat: discover safe external usb block devices`.

### Task 4: FAT/ext4 Inspection and SBC Profiles

**Files:**
- Create: `crates/rust-imager-core/src/filesystem.rs`
- Create: `crates/rust-imager-core/src/profile.rs`
- Create: `crates/rust-imager-linux/src/inspect.rs`
- Create: `crates/rust-imager-core/tests/profiles.rs`

- [ ] Add failing tests for Raspberry Pi, ODROID, unknown-compatible, unsupported filesystem and non-last ext4 layouts.
- [ ] Implement filesystem evidence types from `blkid` and read-only mount probes.
- [ ] Implement profile scoring that never overrides mandatory MBR/FAT/last-primary-ext4 checks.
- [ ] Run tests and workspace checks.
- [ ] Commit as `feat: classify supported sbc disk layouts`.

### Task 5: Immutable Execution Planner

**Files:**
- Create: `crates/rust-imager-core/src/plan.rs`
- Create: `crates/rust-imager-core/tests/plan.rs`

- [ ] Add failing tests for checked sector math, 256 MiB/5% safety margin, alignment, local output rejection, insufficient space, compression and verification policies.
- [ ] Implement immutable `ExecutionPlan`, `ShrinkPlan`, `OutputPlan`, `Compression`, and `VerificationLevel`.
- [ ] Serialize a redacted plan for logs and TUI confirmation.
- [ ] Run tests and workspace checks.
- [ ] Commit as `feat: build immutable imaging execution plans`.

### Task 6: Typed Linux System-Tool Layer

**Files:**
- Create: `crates/rust-imager-linux/src/tools/mod.rs`
- Create: `crates/rust-imager-linux/src/tools/e2fs.rs`
- Create: `crates/rust-imager-linux/src/tools/sfdisk.rs`
- Create: `crates/rust-imager-linux/src/tools/mount.rs`
- Create: `crates/rust-imager-linux/tests/tools.rs`

- [ ] Add failing fake-runner tests asserting exact argv, environment, locale, exit-code handling and captured diagnostics.
- [ ] Implement adapters for `e2fsck`, `resize2fs`, `sfdisk`, `mount`, `umount`, `partprobe`, `blkid`, `lsblk`, and `findmnt`.
- [ ] Ensure every command uses direct argv, `LC_ALL=C`, timeout/cancellation policy and structured results.
- [ ] Run tests and workspace checks.
- [ ] Commit as `feat: add typed linux storage tool adapters`.

### Task 7: First-Boot systemd Expansion

**Files:**
- Create: `crates/rust-imager-first-boot/assets/rust-imager-grow-root.service`
- Create: `crates/rust-imager-first-boot/assets/rust-imager-grow-root.sh`
- Create: `crates/rust-imager-first-boot/src/install.rs`
- Create: `crates/rust-imager-first-boot/tests/install.rs`
- Create: `tests/qemu/run-grow-root.sh`

- [ ] Add failing tests for asset installation, permissions, enablement symlink and idempotent state transitions.
- [ ] Implement dynamic root source/parent disk detection with `findmnt`, `lsblk` and `blkid`; preserve the MBR start sector; grow only the last primary partition; reboot only when required; run `resize2fs`; clean up after success.
- [ ] Add shell syntax checks and a QEMU harness that boots a larger disk twice and asserts partition/filesystem growth plus payload hashes.
- [ ] Run Rust tests and shell checks.
- [ ] Commit as `feat: install first boot root expansion service`.

### Task 8: Shrink Transaction

**Files:**
- Create: `crates/rust-imager-linux/src/shrink.rs`
- Create: `crates/rust-imager-linux/tests/shrink.rs`
- Create: `tests/integration/shrink-loop.sh`

- [ ] Add failing state-machine tests for preflight, unmount, fsck, first-boot install, second fsck, resize, post-fsck, MBR update and revalidation.
- [ ] Implement non-cancellable mutation phases and fail-closed behavior without automatic rollback.
- [ ] Add privileged loop-image integration coverage for successful shrink and each injected command failure.
- [ ] Run unit tests locally and privileged tests on Linux.
- [ ] Commit as `feat: shrink ext4 and mbr with guarded transaction`.

### Task 9: Zstandard Extraction Pipeline

**Files:**
- Create: `crates/rust-imager-pipeline/src/buffer.rs`
- Create: `crates/rust-imager-pipeline/src/extract.rs`
- Create: `crates/rust-imager-pipeline/src/progress.rs`
- Create: `crates/rust-imager-pipeline/tests/zstd.rs`

- [ ] Add failing tests for exact-range reads, bounded memory, progress events, raw/compressed hashes, `.partial`, fsync/rename and read/write failures.
- [ ] Implement a bounded sequential reader-to-Zstandard pipeline with backpressure and optional raw SHA-256.
- [ ] Benchmark against an uncompressed copy of the same range and record relative throughput.
- [ ] Run tests, checks and benchmark smoke test.
- [ ] Commit as `feat: stream block ranges to zstandard images`.

### Task 10: XZ Compression

**Files:**
- Create: `crates/rust-imager-pipeline/src/compression.rs`
- Create: `crates/rust-imager-pipeline/tests/xz.rs`

- [ ] Add failing tests for XZ presets, round-trip data and encoder failures.
- [ ] Generalize the compression sink and implement XZ without changing reader semantics.
- [ ] Run pipeline and workspace tests.
- [ ] Commit as `feat: add streaming xz compression`.

### Task 11: Verification and Sidecar Metadata

**Files:**
- Create: `crates/rust-imager-core/src/metadata.rs`
- Create: `crates/rust-imager-pipeline/src/verify.rs`
- Create: `crates/rust-imager-pipeline/tests/verify.rs`

- [ ] Add failing tests for all four verification levels, corruption, source mismatch, optional raw hash and atomic JSON sidecar creation.
- [ ] Implement compressed-file SHA-256 for every successful image, decode verification and optional source reread comparison.
- [ ] Implement stable versioned JSON metadata.
- [ ] Run tests and workspace checks.
- [ ] Commit as `feat: verify images and write metadata sidecars`.

### Task 12: Wizard TUI and Application Engine

**Files:**
- Create: `crates/rust-imager-tui/src/model.rs`
- Create: `crates/rust-imager-tui/src/view.rs`
- Create: `crates/rust-imager-tui/src/input.rs`
- Create: `crates/rust-imager-cli/src/app.rs`
- Create: `crates/rust-imager-cli/tests/cli.rs`

- [ ] Add failing reducer tests for environment, device, analysis, model confirmation, output, verification, final warning, mutation, extraction and completion screens.
- [ ] Implement keyboard-only wizard state, small-terminal fallback and typed model-string confirmation.
- [ ] Compose discovery, planning, shrinking, extraction and verification behind an event-driven engine.
- [ ] Restore the terminal on errors/panics and emit actionable exit codes.
- [ ] Run snapshot/reducer tests and workspace checks.
- [ ] Commit as `feat: add guarded terminal imaging wizard`.

### Task 13: Fixtures, Fault Injection and Real-Image CI

**Files:**
- Create: `tests/fixtures/generate.sh`
- Create: `tests/fixtures/manifest.json`
- Create: `tests/integration/fault-injection.sh`
- Create: `tests/real-images/manifest.json`
- Create: `tests/real-images/verify.sh`
- Create: `.github/workflows/heavy-ci.yml`

- [ ] Add deterministic synthetic MBR/FAT/ext4 fixture generation and corruption cases.
- [ ] Add `dm-flakey`/`dm-dust` or equivalent tests for read failure, slow device, output exhaustion and corruption.
- [ ] Pin one Raspberry Pi OS and one ODROID Ubuntu official image by URL and SHA-256, cache them, and fail closed on mismatch.
- [ ] Configure heavy CI for manual, scheduled and main-branch runs on an isolated privileged Linux runner.
- [ ] Run workflow lint and all locally available fixture checks.
- [ ] Commit as `test: add block fault and real sbc image validation`.

### Task 14: Performance and Hardening

**Files:**
- Create: `crates/rust-imager-pipeline/benches/throughput.rs`
- Create: `docs/performance.md`
- Modify: pipeline and Linux adapters based on measurements.

- [ ] Add reproducible raw-vs-compressed relative throughput benchmarks and maximum-memory assertions.
- [ ] Measure buffer sizes, thread counts, sequential advice and optional direct I/O on Linux USB 2.0-class throttled storage.
- [ ] Keep optimizations only when tests remain green and compatibility is preserved.
- [ ] Document results, hardware and commands.
- [ ] Commit as `perf: tune sequential imaging pipeline`.

### Task 15: Release Documentation and Acceptance

**Files:**
- Create: `README.md`
- Create: `docs/safety.md`
- Create: `docs/testing.md`
- Create: `CHANGELOG.md`
- Create: `LICENSE`

- [ ] Document installation, exact supported layout, destructive behavior, recovery guidance, output format and first-boot expansion.
- [ ] Run `cargo fmt --check`, Clippy with warnings denied, all unit/integration tests, QEMU tests, dependency/license audit and release build.
- [ ] Perform a requirements matrix review against the design specification.
- [ ] Commit as `docs: prepare initial rust imager release`.

## Verification Gates

Every task must satisfy its focused tests before commit. Before completion, the branch must satisfy:

```bash
cargo fmt --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo build --workspace --release
shellcheck tests/**/*.sh crates/rust-imager-first-boot/assets/*.sh
```

Linux-only gates additionally run fixture, loop-device, fault-injection and QEMU scripts. A passing macOS build alone is never considered acceptance for this Linux-only product.
