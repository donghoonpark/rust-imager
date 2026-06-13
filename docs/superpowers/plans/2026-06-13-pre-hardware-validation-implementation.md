# Pre-Hardware Validation Hardening Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Close destructive-operation safety gaps and maximize automated confidence before physical eMMC hardware is available.

**Architecture:** Introduce explicit block-topology and quiescence policies in the Linux crate, move post-shrink validation into the real transaction, and expose an injectable application engine for privileged loop tests. Extend device-mapper and QEMU harnesses to validate failures and restored-image behavior while preserving the existing crate boundaries.

**Tech Stack:** Rust 1.94.1, Linux sysfs/util-linux/e2fsprogs, loop devices, device-mapper, QEMU, systemd, Criterion, GitHub Actions.

---

### Task 1: Resolve Output Storage to a Whole Disk

**Files:**
- Modify: `crates/rust-imager-linux/src/discovery.rs`
- Modify: `crates/rust-imager-linux/tests/discovery.rs`
- Modify: `crates/rust-imager-cli/src/app.rs`

- [ ] Add failing discovery tests proving relative paths, symlinked parents, and
  bind-mount sources exclude the correct whole disk.
- [ ] Run `cargo test -p rust-imager-linux --test discovery` and confirm the new
  tests fail because string-prefix matching cannot resolve the backing disk.
- [ ] Replace output-prefix filtering with an API that accepts the canonical
  output parent and backing block source, then maps that source through the
  `lsblk` child-to-disk index.
- [ ] Make `app::discover` canonicalize the existing output parent and query
  `findmnt -T <parent> -n -o SOURCE` before candidate selection.
- [ ] Run the discovery tests and CLI unit tests.
- [ ] Commit with `fix: resolve output paths to backing disks`.

### Task 2: Enforce Supported Sector Geometry

**Files:**
- Modify: `crates/rust-imager-linux/src/discovery.rs`
- Modify: `crates/rust-imager-linux/tests/discovery.rs`
- Modify: `crates/rust-imager-cli/src/app.rs`
- Modify: `docs/safety.md`

- [ ] Add a failing test with a USB `/dev/sdX` device reporting a 4096-byte
  logical sector and assert it is not eligible.
- [ ] Run the focused test and confirm it fails.
- [ ] Reject any candidate whose logical sector size is not exactly 512 bytes.
- [ ] Add a defensive check in `prepare` so manually supplied or changed
  identities cannot bypass discovery policy.
- [ ] Document the 512-byte-sector limitation.
- [ ] Run discovery and planner tests.
- [ ] Commit with `fix: restrict imaging to 512 byte sectors`.

### Task 3: Quiesce the Entire Target Disk

**Files:**
- Modify: `crates/rust-imager-linux/src/tools.rs`
- Modify: `crates/rust-imager-linux/src/shrink.rs`
- Modify: `crates/rust-imager-linux/tests/tools.rs`
- Modify: `crates/rust-imager-linux/tests/shrink.rs`
- Create: `tests/integration/quiescence.sh`
- Modify: `.github/workflows/heavy-ci.yml`

- [ ] Add failing unit tests for enumerating every mounted child target and
  unmounting deepest mountpoints first.
- [ ] Add failing tests that reject active swap and non-empty sysfs holder sets.
- [ ] Run focused tests and confirm the missing whole-disk behavior.
- [ ] Add a pre-mutation quiescence operation that reads machine-readable
  `lsblk`, `/proc/swaps`, and `/sys/class/block/*/holders`.
- [ ] Unmount every mounted child, then rerun the checks and fail if any mount,
  swap, or holder remains.
- [ ] Add a privileged loop test with FAT and ext4 partitions mounted at the
  same time and verify both are unmounted.
- [ ] Run unit and privileged tests.
- [ ] Commit with `fix: quiesce all target disk users`.

### Task 4: Implement Real Post-Shrink Revalidation

**Files:**
- Modify: `crates/rust-imager-linux/src/shrink.rs`
- Modify: `crates/rust-imager-linux/src/inspect.rs`
- Modify: `crates/rust-imager-linux/tests/shrink.rs`
- Modify: `crates/rust-imager-linux/tests/real_image.rs`
- Modify: `crates/rust-imager-cli/src/app.rs`

- [ ] Add failing tests that alter a non-root MBR entry, root start, root type,
  ext4 size, and identity during the `Revalidate` stage.
- [ ] Run focused tests and confirm `Revalidate => Ok(())` lets them pass
  incorrectly.
- [ ] Extend `ShrinkRequest` with the captured identity, original MBR, logical
  sector size, and expected ext4 target.
- [ ] Implement revalidation of device identity, full MBR invariants, ext4 block
  count versus partition capacity, target rounding, and `e2fsck -fn`.
- [ ] Remove the partial duplicate check from the CLI and use the transaction
  result as the source of truth.
- [ ] Extend real-image assertions to verify all non-root entries are unchanged.
- [ ] Run Linux crate and real-image tests.
- [ ] Commit with `fix: verify complete post shrink invariants`.

### Task 5: Bound External Commands and Report Termination

**Files:**
- Modify: `crates/rust-imager-linux/Cargo.toml`
- Modify: `crates/rust-imager-linux/src/command.rs`
- Create: `crates/rust-imager-linux/tests/command.rs`
- Modify: `crates/rust-imager-linux/src/tools.rs`
- Modify: `crates/rust-imager-cli/src/app.rs`
- Modify: `docs/safety.md`

- [ ] Add failing tests for a sleeping command timeout and a command terminated
  by signal.
- [ ] Run the command tests and confirm timeout support is absent.
- [ ] Add command classes with read-only and destructive timeout defaults,
  process-group creation, timed wait, TERM grace period, and KILL fallback.
- [ ] Represent timeout and signal termination as distinct `CommandError`
  variants.
- [ ] Mark storage mutation commands destructive and inspection commands
  read-only.
- [ ] Install parent signal handling around the mutation phase that warns and
  defers ordinary interruption while a destructive command is active.
- [ ] Run command, tool, and application tests.
- [ ] Commit with `fix: bound and classify storage commands`.

### Task 6: Exercise the Complete Application Engine

**Files:**
- Modify: `crates/rust-imager-cli/src/app.rs`
- Create: `crates/rust-imager-cli/tests/engine_loop.rs`
- Create: `tests/integration/full-engine.sh`
- Modify: `.github/workflows/heavy-ci.yml`

- [ ] Define an `EngineSystem` boundary for topology queries, command execution,
  and source identity while keeping `run_image` as the production wrapper.
- [ ] Add a failing privileged test that runs the complete engine on a loop-backed
  FAT/ext4 disk and expects image, metadata, and log artifacts.
- [ ] Add same-source/output-disk, relative output, mounted FAT, and identity
  change cases.
- [ ] Implement only the injection needed for the test while preserving the CLI
  behavior.
- [ ] Verify Zstandard decode and source reread through the full engine.
- [ ] Run CLI tests and the privileged integration script.
- [ ] Commit with `test: cover the complete imaging engine`.

### Task 7: Expand Failure Injection

**Files:**
- Modify: `tests/integration/fault-injection.sh`
- Create: `crates/rust-imager-cli/tests/failure_log.rs`
- Modify: `crates/rust-imager-cli/src/app.rs`
- Modify: `.github/workflows/heavy-ci.yml`

- [ ] Add failing tests for output `ENOSPC`, mid-stream read error, `sfdisk`
  failure, and partition-reread failure.
- [ ] Assert image/partial behavior and the final log's stage and
  `source_modified` value for every case.
- [ ] Route injected command and I/O failures through the full engine test
  boundary.
- [ ] Ensure failure logs are synced even when final metadata creation fails.
- [ ] Run focused tests and privileged failure injection.
- [ ] Commit with `test: inject imaging transaction failures`.

### Task 8: Restore and Boot a Multi-Partition Image in QEMU

**Files:**
- Modify: `tests/qemu/run-grow-root.sh`
- Modify: `.github/workflows/heavy-ci.yml`
- Modify: `docs/testing.md`

- [ ] Change the QEMU fixture to contain a preserved FAT first partition and an
  ext4 root second partition.
- [ ] Compress the compact source image with the production pipeline or CLI,
  decode it into a larger target image, and boot that restored target twice.
- [ ] Verify first-partition start/end and payload hash are unchanged, root
  partition and filesystem grow, and first-boot assets remove themselves.
- [ ] Add an optional ARM64 `virt` job only if a local smoke run completes within
  the existing 90-minute job budget.
- [ ] Run shell syntax and QEMU validation.
- [ ] Commit with `test: boot restored multi partition images`.

### Task 9: Repair Performance and Memory Validation

**Files:**
- Modify: `crates/rust-imager-pipeline/benches/throughput.rs`
- Create: `crates/rust-imager-pipeline/tests/memory_bound.rs`
- Modify: `docs/performance.md`
- Modify: `.github/workflows/ci.yml`

- [ ] Add a benchmark smoke test that currently fails on the second iteration
  because the output already exists.
- [ ] Remove each benchmark output before an iteration or allocate unique output
  paths outside the timed setup.
- [ ] Add a test-visible pipeline metric proving queued input capacity is
  `buffer_bytes * queue_depth` and independent of source size.
- [ ] Add a non-gating benchmark smoke command to CI.
- [ ] Run the benchmark with a short sample and verify it completes.
- [ ] Commit with `test: make performance validation repeatable`.

### Task 10: Final Documentation and Verification

**Files:**
- Modify: `README.md`
- Modify: `docs/testing.md`
- Modify: `docs/performance.md`
- Modify: `docs/safety.md`

- [ ] Document automated guarantees and the remaining physical acceptance list.
- [ ] Run `cargo fmt --check`.
- [ ] Run `cargo clippy --workspace --all-targets --all-features -- -D warnings`.
- [ ] Run `cargo test --workspace --all-features`.
- [ ] Run `cargo build --workspace --release`.
- [ ] Run `bash -n tests/**/*.sh crates/rust-imager-first-boot/assets/*.sh`.
- [ ] Run the benchmark smoke test.
- [ ] Run `git diff --check`.
- [ ] Push the branch and require both normal and heavy GitHub Actions workflows
  to finish successfully.
- [ ] Commit with `docs: define pre hardware acceptance boundary`.
