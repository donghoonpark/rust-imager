# Testing

## Fast Checks

```bash
cargo fmt --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo build --workspace --release
```

The unit suite covers MBR fuzz inputs, overlap/bounds/GPT rejection, device
filtering, identity changes, SBC profiles, execution planning, exact external
command argv, shrink ordering, first-boot installation, bounded compression,
atomic outputs, all verification levels, metadata, and TUI reducers.

## Privileged Linux Fixtures

```bash
tests/fixtures/generate.sh
sudo tests/integration/quiescence.sh
sudo tests/integration/fault-injection.sh
sudo tests/integration/full-engine.sh
```

The fixture generator creates an MBR/FAT/ext4 image through a loop device.
The quiescence test mounts both FAT and ext4 partitions and proves production
code unmounts the complete disk. The full-engine test exposes a loop disk
through a CI-only `/dev/sdX` facade and runs the real CLI through filesystem
inspection, first-boot installation, shrink, MBR rewrite, compression, reread
verification, logging, and metadata generation. Fault injection uses
device-mapper and a constrained tmpfs to verify production extraction retains
partial artifacts after source read errors and output `ENOSPC`.

## Real Images

```bash
tests/real-images/verify.sh
```

The manifest pins one Raspberry Pi OS image and one Ubuntu-based ODROID N2
image by URL and SHA-256. Downloads are cached, but every use rechecks the
digest. URL failure or mismatch fails the job. Each image is copied to a loop
device, classified by the Rust implementation, modified with the real shrink
transaction, extracted as both Zstandard and XZ, decoded, source-reread
verified, and checked with read-only `e2fsck`.

## QEMU First Boot

On Ubuntu:

```bash
sudo apt install debootstrap e2fsprogs fdisk grub-pc-bin jq qemu-system-x86
sudo tests/qemu/run-grow-root.sh
```

The harness builds a minimal Debian system on an MBR/ext4 disk, installs the
same systemd assets embedded in the Rust crate, enlarges the backing disk, boots
twice in QEMU, and verifies partition growth, ext4 consistency, service cleanup,
and payload integrity.

QEMU validates generic Linux/systemd behavior. Raspberry Pi and ODROID boot ROM,
firmware, and USB eMMC bridge behavior still require real hardware acceptance.

The heavy workflow also downloads the checksum-pinned official Ubuntu 24.04
Noble root filesystem, installs it into a DOS/MBR FAT+ext4 disk, compresses and
restores that compact disk into a larger image, and boots the restored Ubuntu
system twice. It verifies root growth, first-partition preservation, payload
integrity, and one-shot cleanup using Ubuntu's real kernel, systemd, and storage
utilities.

## Physical Acceptance Remaining

Automation cannot establish:

- board ROM and vendor firmware boot from a restored image
- behavior of representative USB eMMC bridges during reset or disconnect
- eMMC bad-block, wear, and sudden-power-loss behavior
- sustained USB 2.0 throughput on physical host controllers
- eMMC boot0/boot1 areas not exposed in the user-data disk address space

## Ubuntu Packages

Package script contracts and static container/release checks run without
modifying block devices:

```bash
docker run --rm -v "$PWD:/workspace" -w /workspace ubuntu:24.04 \
  bash -lc 'packaging/debian/test-build-deb.sh &&
            packaging/debian/test-test-package.sh'
bash packaging/test-container-scripts.sh
bash packaging/test-release-workflow.sh
shellcheck packaging/*.sh packaging/debian/*.sh
actionlint .github/workflows/release.yml
```

Build an architecture package in an Ubuntu 18.04 userspace:

```bash
packaging/build-package.sh 0.1.0 amd64 dist
packaging/build-package.sh 0.1.0 arm64 dist
```

Docker Buildx and QEMU/binfmt support are required when the requested
architecture differs from the host. Install-test one package on every supported
Ubuntu LTS:

```bash
packaging/test-lts-matrix.sh \
  dist/rust-imager_0.1.0_amd64.deb 0.1.0 amd64
```

The matrix is Ubuntu 18.04, 20.04, 22.04, and 24.04. Passing a fourth argument
tests only that version. A missing image, failed package dependency, unresolved
shared library, or emulation failure stops the run.

Run the destructive transaction against a real loop-backed MBR disk on the
same matrix:

```bash
packaging/test-lts-loop-matrix.sh \
  dist/rust-imager_0.1.0_amd64.deb 0.1.0 amd64
```

Manual dispatch of `release.yml` builds `amd64` and `arm64` packages, executes
all eight architecture/LTS combinations, and retains test artifacts without
creating a release. A `v<workspace-version>` tag runs the same gates and then
publishes both packages and `SHA256SUMS` to GitHub Releases.

## Interactive Docker Demo

Build a privileged Ubuntu demo around an installable package:

```bash
packaging/demo/build.sh \
  dist/rust-imager_0.2.3_arm64.deb \
  rust-imager-demo:0.2.3 \
  18.04 \
  arm64
```

The container creates a real loop-backed DOS/MBR disk with FAT boot and ext4
root partitions. `/dev/sdz`, `/dev/sdz1`, and `/dev/sdz2` are real block device
nodes that share the loop device's major/minor numbers, so destructive resize,
partition, extraction, compression, and verification paths are exercised.

Run the complete non-interactive transaction:

```bash
docker run --rm --privileged \
  -v "$PWD/demo-output:/output" \
  rust-imager-demo:0.2.3 full-run
```

Launch the TUI in Ghostty:

```bash
open -na Ghostty.app --args \
  -e "$PWD/packaging/demo/launch-ghostty.sh" rust-imager-demo:0.2.3
```

Regenerate the README TUI recording from the Ubuntu 24.04 demo image:

```bash
brew install vhs
vhs docs/assets/rust-imager-demo.tape
```

The tape drives the real privileged loop-backed demo and writes
`docs/assets/rust-imager-demo.gif`. Build the image tag referenced by the tape
before recording when it is not already present locally.

## CI

- `ci.yml`: format, Clippy, unit/integration-safe tests, dependency advisories
  and licenses.
- `heavy-ci.yml`: scheduled/manual privileged fixtures, device-mapper failure,
  real image downloads, and QEMU first-boot expansion.
- `release.yml`: Ubuntu 18.04-based `amd64`/`arm64` packages, installation tests
  and full loop imaging on Ubuntu 18.04 through 24.04, plus guarded tag release
  publication.
