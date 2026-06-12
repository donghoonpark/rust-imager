# Changelog

## 0.1.0 - Unreleased

- Add Linux-only Rust TUI and non-interactive CLI.
- Detect safe external USB `/dev/sdX` candidates and reject system/output/swap
  disks.
- Validate MBR/FAT/final-primary-ext4 SBC layouts and reject GPT.
- Install an idempotent systemd first-boot root expansion service.
- Shrink ext4 and the final MBR partition through guarded e2fsprogs/util-linux
  adapters.
- Stream the useful disk range through bounded Zstandard or XZ pipelines.
- Add optional streaming hash, decode verification, and source reread.
- Write versioned JSON metadata and durable logs without overwriting files.
- Add synthetic, fault-injection, real-image, QEMU, and performance validation.
