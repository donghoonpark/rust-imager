# rust-imager

`rust-imager` is a Linux-only Rust TUI for creating compact, compressed images
from external SBC eMMC modules exposed by a USB reader as `/dev/sdX`.

It reduces transfer time by shrinking the offline ext4 root filesystem and its
final MBR partition before reading from LBA 0 through the new partition end.
The resulting image preserves raw bootloader sectors, the FAT boot partition,
and the ext4 root partition. A systemd one-shot service expands the restored
root partition and filesystem on first boot.

## Warning

This program **modifies the source eMMC**. It runs `e2fsck`, installs first-boot
files, shrinks ext4, and rewrites the final MBR partition. The source remains
shrunken after imaging. Power loss, USB disconnects, bridge firmware bugs, or
storage failure during mutation can make the source unbootable.

Read [docs/safety.md](docs/safety.md) before using it.

## Supported Layout

- Linux host, run with `sudo`
- External USB eMMC reader exposed as a whole `/dev/sdX` disk
- DOS/MBR partition table
- Final primary partition is an offline ext4 Linux root
- Known Raspberry Pi/ODROID FAT boot layouts are recognized; structurally safe
  ext4-root layouts without a recognized FAT profile require a strong warning
  and exact model confirmation
- Output is a local filesystem path
- Zstandard (`.img.zst`) or XZ (`.img.xz`)

GPT, extended/logical partitions, live system eMMC, `/dev/mmcblkN`, RPMB,
separately exposed boot0/boot1 devices, network output, and non-ext4 root
filesystems are rejected.

## Build

```bash
rustup toolchain install 1.94.1 --component clippy,rustfmt
cargo build --release
sudo install -m 0755 target/release/rust-imager /usr/local/sbin/rust-imager
```

Runtime tools:

```bash
sudo apt install e2fsprogs fdisk mount util-linux
```

## Use

Launch the wizard:

```bash
sudo rust-imager
```

List eligible devices:

```bash
sudo rust-imager list
```

Automation uses the same guarded engine:

```bash
sudo rust-imager image \
  --device /dev/sda \
  --output /var/lib/rust-imager/board.img.zst \
  --confirm-model "eMMC Reader" \
  --compression zstd \
  --level 3 \
  --verify decode
```

Verification values are `none`, `hash`, `decode`, and `reread`.

## Output

Successful execution produces:

```text
board.img.zst
board.img.zst.json
board.img.zst.log
```

Failed extraction retains `board.img.zst.partial`. Existing final, partial,
sidecar, and log files are never overwritten.

## Development

```bash
cargo fmt --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo build --workspace --release
```

Privileged Linux, real-image, fault-injection, and QEMU validation are
documented in [docs/testing.md](docs/testing.md). Performance measurement is in
[docs/performance.md](docs/performance.md).

Automated validation includes pinned Raspberry Pi OS and ODROID images, a full
loop-backed CLI transaction, injected read/output failures, generic Debian
first-boot expansion, and a restored Ubuntu 24.04 FAT+ext4 image booted twice in
QEMU. Physical USB bridge behavior and vendor-board firmware boot remain
hardware acceptance requirements.

## License

MIT
