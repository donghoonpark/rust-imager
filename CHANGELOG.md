# Changelog

## 0.2.1 - 2026-06-14

- Replace the basic wizard with a responsive dashboard for wide, medium, and
  compact terminals.
- Add setup and operation timelines, prominent destructive-phase warnings, and
  persistent completion and failure panels.
- Show live extraction percentage, throughput, elapsed time, ETA, and recent
  engine events.
- Add arrow and `j`/`k` device navigation with screen-specific keyboard help.
- Run imaging on a worker thread so elapsed time and status continue updating
  during inspection, shrinking, extraction, and verification.
- Add deterministic Ratatui rendering tests across multiple terminal sizes.

## 0.1.0 - 2026-06-13

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
- Add installable `amd64` and `arm64` Debian packages built against Ubuntu
  18.04 and install-tested on every Ubuntu LTS through 26.04.
- Publish tagged releases only after all ten package compatibility jobs pass,
  with SHA-256 checksums and manual non-publishing validation runs.
