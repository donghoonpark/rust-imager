# Ubuntu LTS Loop Compatibility Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the complete imaging transaction work with Ubuntu 18.04 util-linux 2.31 and run privileged loop-backed validation on Ubuntu 18.04, 20.04, 22.04, and 24.04 in CI.

**Architecture:** Use only legacy `lsblk` columns shared by all supported LTS releases, with `--paths` plus `NAME` for absolute paths and singular `MOUNTPOINT` for quiescence. Parameterize the existing Docker demo by Ubuntu version, make its wrappers execute the real distribution tools before injecting USB `/dev/sdz` identity, and reuse that demo from a native-architecture CI matrix.

**Tech Stack:** Rust, Serde, util-linux 2.31+, e2fsprogs, Bash, Docker Buildx, privileged loop devices, GitHub Actions.

---

### Task 1: Legacy quiescence topology

**Files:**
- Modify: `crates/rust-imager-linux/src/quiescence.rs`
- Modify: `crates/rust-imager-linux/tests/quiescence.rs`

- [ ] Add a failing test requiring `lsblk --json --paths --output NAME,MOUNTPOINT`.
- [ ] Add a util-linux 2.31 JSON fixture using `name` and singular `mountpoint`.
- [ ] Run `cargo test -p rust-imager-linux --test quiescence --locked` and confirm the new assertions fail.
- [ ] Parse `name` as the absolute path, derive the sysfs holder name from its basename, and collect singular mountpoints.
- [ ] Re-run the focused test and the Linux crate tests.
- [ ] Commit as `fix: support legacy lsblk quiescence`.

### Task 2: Distribution-real Docker demo

**Files:**
- Modify: `packaging/demo/Dockerfile`
- Modify: `packaging/demo/build.sh`
- Modify: `packaging/demo/entrypoint.sh`
- Create: `packaging/test-lts-loop-matrix.sh`
- Modify: `packaging/test-container-scripts.sh`

- [ ] Add failing shell contract assertions for a parameterized Ubuntu base and four-version loop matrix.
- [ ] Run `bash packaging/test-container-scripts.sh` and confirm failure.
- [ ] Add `ARG UBUNTU_VERSION` to the demo image and pass architecture/platform/version through the build helper.
- [ ] Update the `lsblk` wrapper so discovery and quiescence both execute the real command successfully before emitting alias-normalized JSON.
- [ ] Keep `findmnt`, `partprobe`, filesystem tools, and partition tools backed by the installed distribution binaries.
- [ ] Add a loop matrix driver for Ubuntu 18.04, 20.04, 22.04, and 24.04.
- [ ] Run all four arm64 containers locally through `full-run`.
- [ ] Commit as `test: run full imaging across Ubuntu LTS`.

### Task 3: CI loop matrix

**Files:**
- Modify: `.github/workflows/release.yml`
- Modify: `packaging/test-release-workflow.sh`
- Modify: `docs/testing.md`

- [ ] Add failing workflow contract assertions requiring privileged loop validation for 18.04 through 24.04.
- [ ] Run `bash packaging/test-release-workflow.sh` and confirm failure.
- [ ] Extend each native compatibility job to run the full loop transaction after package smoke testing.
- [ ] Retain amd64 and arm64 coverage and remove Ubuntu 26.04 from the supported release matrix.
- [ ] Document local and CI loop validation.
- [ ] Run workflow, shell, format, Clippy, and workspace tests.
- [ ] Commit as `ci: validate loop imaging on Ubuntu LTS`.

### Task 4: Patch release

**Files:**
- Modify: `Cargo.toml`
- Modify: `Cargo.lock`
- Modify: `crates/*/Cargo.toml`
- Modify: `CHANGELOG.md`

- [ ] Bump the patch version from `0.2.6` to `0.2.7`.
- [ ] Record util-linux 2.31 quiescence support and the full LTS loop matrix.
- [ ] Build the Ubuntu 18.04-based amd64 and arm64 packages.
- [ ] Re-run the four-version loop matrix with the release package.
- [ ] Commit as `chore: release v0.2.7`.
- [ ] Push `main` and annotated tag `v0.2.7`.
- [ ] Verify CI, package assets, checksums, and GitHub Release publication.
