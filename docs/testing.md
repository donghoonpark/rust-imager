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
sudo tests/integration/fault-injection.sh
```

The fixture generator creates an MBR/FAT/ext4 image through a loop device.
Fault injection uses device-mapper to prove that permanent read errors are
observable rather than silently replaced.

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

## CI

- `ci.yml`: format, Clippy, unit/integration-safe tests, dependency advisories
  and licenses.
- `heavy-ci.yml`: scheduled/manual privileged fixtures, device-mapper failure,
  real image downloads, and QEMU first-boot expansion.
