# Ubuntu LTS Packaging and Release Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build reproducible `amd64` and `arm64` Debian packages and publish them only after installation tests pass on every Ubuntu LTS from 18.04 through 26.04.

**Architecture:** Shell scripts own package metadata, assembly, and smoke tests so local and CI execution share one path. Docker Buildx builds each package in an Ubuntu 18.04 userspace and tests the package in architecture-matched LTS containers; GitHub Actions coordinates artifacts and publishes tag releases.

**Tech Stack:** Bash, Cargo/Rust 1.94.1, `dpkg-deb`, Docker Buildx, QEMU user emulation, GitHub Actions, GitHub CLI

---

### Task 1: Debian package assembly and metadata tests

**Files:**
- Create: `packaging/debian/build-deb.sh`
- Create: `packaging/debian/test-build-deb.sh`
- Modify: `.gitignore`

- [ ] **Step 1: Write a failing package-builder test**

Create a shell test that supplies a fake `rust-imager` executable reporting
`rust-imager 0.1.0`, invokes the missing builder, and checks package filename,
control metadata, installed paths, and permissions with `dpkg-deb`.

- [ ] **Step 2: Run the test and verify it fails**

Run:

```bash
bash packaging/debian/test-build-deb.sh
```

Expected: failure because `packaging/debian/build-deb.sh` does not exist.

- [ ] **Step 3: Implement the package builder**

Implement strict argument validation, Debian version validation through
`dpkg --validate-version`, `amd64`/`arm64` allowlisting, binary version
comparison, temporary package-root cleanup, explicit control metadata, and
deterministic file modes. Produce
`rust-imager_<version>_<architecture>.deb` in the requested output directory.

- [ ] **Step 4: Run package tests**

Run:

```bash
bash packaging/debian/test-build-deb.sh
```

Expected: PASS with metadata, paths, permissions, invalid-version, invalid-arch,
and binary-version-mismatch cases covered.

- [ ] **Step 5: Commit**

```bash
git add .gitignore packaging/debian/build-deb.sh packaging/debian/test-build-deb.sh
git commit -m "build: add Debian package assembly"
```

### Task 2: Installed-package smoke testing

**Files:**
- Create: `packaging/debian/test-package.sh`
- Create: `packaging/debian/test-test-package.sh`

- [ ] **Step 1: Write a failing smoke-test contract test**

Create a test using fake `dpkg-query`, `rust-imager`, `ldd`, and runtime tools.
Assert success for matching metadata and failure for a missing shared library,
missing command, wrong architecture, and wrong binary version.

- [ ] **Step 2: Run the contract test and verify it fails**

Run:

```bash
bash packaging/debian/test-test-package.sh
```

Expected: failure because `packaging/debian/test-package.sh` does not exist.

- [ ] **Step 3: Implement the installed-package smoke test**

Accept expected version and Debian architecture, query installed package
metadata, check `/usr/sbin/rust-imager`, run `--version` and `--help`, reject
`ldd` output containing `not found`, and require:

```text
e2fsck resize2fs sfdisk mount umount partprobe lsblk findmnt blkid swapon setsid
```

- [ ] **Step 4: Run both shell test suites**

Run:

```bash
bash packaging/debian/test-build-deb.sh
bash packaging/debian/test-test-package.sh
```

Expected: both PASS.

- [ ] **Step 5: Commit**

```bash
git add packaging/debian/test-package.sh packaging/debian/test-test-package.sh
git commit -m "test: add installed package smoke checks"
```

### Task 3: Reproducible multi-architecture container builds

**Files:**
- Create: `packaging/docker/Dockerfile.build`
- Create: `packaging/docker/Dockerfile.test`
- Create: `packaging/build-package.sh`
- Create: `packaging/test-lts-matrix.sh`
- Create: `packaging/test-container-scripts.sh`

- [ ] **Step 1: Write static and behavior tests for container drivers**

Assert the build image is Ubuntu 18.04, the Rust version comes from
`rust-toolchain.toml`, both Debian architectures map to the correct Docker
platform, and the LTS list is exactly:

```text
18.04 20.04 22.04 24.04 26.04
```

Test argument rejection without requiring Docker.

- [ ] **Step 2: Run the driver tests and verify they fail**

Run:

```bash
bash packaging/test-container-scripts.sh
```

Expected: failure because the Dockerfiles and drivers do not exist.

- [ ] **Step 3: Implement the Buildx package driver**

Build a platform-specific Ubuntu 18.04 image with compiler dependencies,
Rust 1.94.1, and `dpkg-dev`; compile the workspace release binary; call
`build-deb.sh`; export the package through a Buildx local output.

- [ ] **Step 4: Implement the LTS matrix driver**

For one supplied package and architecture, run `Dockerfile.test` for all five
Ubuntu tags on the matching platform. Each image installs package dependencies,
installs the `.deb`, copies `test-package.sh`, and executes the smoke test.
Any unavailable image or emulation error stops the matrix.

- [ ] **Step 5: Run static tests and available local package checks**

Run:

```bash
bash packaging/test-container-scripts.sh
shellcheck packaging/*.sh packaging/debian/*.sh
```

Expected: PASS. Also build and test the native architecture package if Docker
is available; record any host-emulation limitation.

- [ ] **Step 6: Commit**

```bash
git add packaging
git commit -m "build: add Ubuntu LTS package matrix"
```

### Task 4: GitHub Actions release pipeline

**Files:**
- Create: `.github/workflows/release.yml`
- Create: `packaging/release-version.sh`
- Create: `packaging/test-release-workflow.sh`

- [ ] **Step 1: Write workflow contract tests**

Check triggers, read-only default permissions, tag/version validation, both
architectures, every LTS version, artifact retention, full-matrix dependency,
tag-only publishing, immutable action SHAs, and checksum generation.

- [ ] **Step 2: Run the workflow contract test and verify it fails**

Run:

```bash
bash packaging/test-release-workflow.sh
```

Expected: failure because the workflow and version helper do not exist.

- [ ] **Step 3: Implement version derivation**

Read `workspace.package.version` from Cargo metadata. For tag events, require
`v<workspace-version>`. For manual events, emit
`<workspace-version>~ci<run-number>`.

- [ ] **Step 4: Implement the release workflow**

Use pinned action commits for checkout, QEMU, Buildx, artifact upload/download,
and GitHub-script or `gh`. Build the two packages, test each package against all
five LTS tags, then publish both packages plus `SHA256SUMS` only for tag pushes.
Manual runs retain packages as Actions artifacts and skip release creation.

- [ ] **Step 5: Validate workflow and helper**

Run:

```bash
bash packaging/test-release-workflow.sh
actionlint .github/workflows/release.yml
```

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add .github/workflows/release.yml packaging/release-version.sh packaging/test-release-workflow.sh
git commit -m "ci: publish verified Ubuntu packages"
```

### Task 5: Installation and release documentation

**Files:**
- Modify: `README.md`
- Modify: `docs/testing.md`
- Modify: `CHANGELOG.md`

- [ ] **Step 1: Document package installation**

Add GitHub Release download guidance, SHA-256 verification, local `.deb`
installation, supported architecture/LTS matrix, package dependencies, and the
existing `sudo` requirement.

- [ ] **Step 2: Document package verification**

Add local package assembly, native matrix testing, cross-architecture Buildx
requirements, manual Actions dispatch behavior, and tag release behavior.

- [ ] **Step 3: Update changelog**

Record Debian packaging, `amd64`/`arm64`, Ubuntu 18.04-26.04 tests, and guarded
GitHub Release publication under version 0.1.0.

- [ ] **Step 4: Check documentation**

Run:

```bash
rg -n "18\\.04|20\\.04|22\\.04|24\\.04|26\\.04|amd64|arm64|SHA256SUMS" README.md docs/testing.md CHANGELOG.md
```

Expected: each release promise and installation artifact is documented.

- [ ] **Step 5: Commit**

```bash
git add README.md docs/testing.md CHANGELOG.md
git commit -m "docs: document Ubuntu package installation"
```

### Task 6: End-to-end verification

**Files:**
- Modify only if verification exposes a defect.

- [ ] **Step 1: Run repository tests**

```bash
cargo fmt --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
```

Expected: PASS.

- [ ] **Step 2: Run packaging tests**

```bash
bash packaging/debian/test-build-deb.sh
bash packaging/debian/test-test-package.sh
bash packaging/test-container-scripts.sh
bash packaging/test-release-workflow.sh
shellcheck packaging/*.sh packaging/debian/*.sh
actionlint .github/workflows/release.yml
```

Expected: PASS.

- [ ] **Step 3: Run the local Docker matrix**

Build both package architectures and execute all available LTS compatibility
tests. The complete authoritative 10-job matrix runs in GitHub Actions.

- [ ] **Step 4: Push and run manual release workflow**

Push `main`, dispatch `release.yml`, wait for both package builds and all ten
compatibility tests, and verify no GitHub Release was created by the manual run.

- [ ] **Step 5: Commit verification fixes separately**

If verification required code changes:

```bash
git add <fixed-files>
git commit -m "fix: harden Ubuntu package verification"
```
