# Changelog

## 0.2.5 - 2026-06-14

- Use `lsblk --paths` with the legacy `NAME` column because util-linux 2.31
  does not provide the later `PATH` column.
- Preserve JSON device hierarchy on both util-linux 2.31 and current releases
  by requesting `NAME`, while continuing to use `findmnt` for mount filtering.

## 0.2.4 - 2026-06-14

- Restore Ubuntu 18.04 compatibility by limiting device discovery to `lsblk`
  columns supported by util-linux 2.31 while continuing to use `findmnt` for
  mounted-device filtering.

## 0.2.3 - 2026-06-14

- Generate timestamped output paths from the selected device and keep automatic
  extensions synchronized with the chosen compression format.
- Add reversible setup navigation, early output-collision checks, and a
  read-only capacity plan before destructive work begins.
- Prioritize destructive target and first irreversible action in review.
- Show final size ratio, SHA-256, verification status, metadata, log, and
  elapsed time on completion.
- Add a privileged loop-backed Docker demo that exercises filesystem shrink,
  MBR resize, extraction, compression, verification, and artifact generation
  through real `/dev/sdz*` block nodes.

## 0.2.2 - 2026-06-14

- Complete extensionless TUI output paths as `.img.zst` or `.img.xz` from the
  selected compression format.
- Complete paths ending in `.img` with only the compression extension.
- Preserve explicit or conflicting extensions for strict engine validation
  instead of silently rewriting user input.

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
