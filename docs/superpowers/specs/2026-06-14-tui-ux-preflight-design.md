# TUI UX and Full Demo Design

## Goal

Improve the interactive workflow with useful defaults, reversible setup,
early validation, honest preflight sizing, risk-focused review, and a complete
result summary. Make the Ghostty Docker demo execute the same destructive
shrink, extraction, and verification path as a real supported device.

## Setup Workflow

### Default Output

After device selection, the output field defaults to an absolute path in the
current directory:

`<sanitized-model>-YYYYMMDD-HHMMSS.img.zst`

The path remains editable. Compression changes update an automatically
generated extension but do not rewrite an explicitly edited conflicting
extension.

### Back Navigation

`Left`, `BackTab`, and `Shift+Tab` move to the previous setup screen.
Configuration values remain intact. Back navigation is unavailable after the
operation starts.

### Early Output Validation

Continuing from Output checks the final image, partial image, sidecar, partial
sidecar, and log paths. Any conflict remains on the Output screen with the
specific occupied path. Missing parent directories are allowed because the
engine creates them.

## Preflight and Review

After Verification, the CLI performs a read-only preflight using the same
device analysis, ext4 geometry, local-output policy, free-space query, and
planning logic used by execution. No source mutation occurs.

The Review screen displays:

- source capacity,
- current ext4 size,
- planned ext4 target,
- planned raw image range,
- available output bytes,
- selected compression and verification,
- the exact device and root partition that will be modified,
- a clear statement that the first irreversible action is filesystem checking
  followed by first-boot asset installation and shrinking.

The engine repeats the full preflight immediately before mutation; the Review
data is advisory and cannot weaken execution-time revalidation.

## Completion Summary

The engine completion event carries output path, raw bytes, compressed bytes,
compressed SHA-256, and verification status. The TUI preserves total elapsed
time and shows compression ratio plus the metadata and log paths. Failure
screens continue to show the persistent error and log path.

## Docker Demo

The demo uses a real loop-backed MBR disk with FAT boot and ext4 root
partitions. `/dev/sdz`, `/dev/sdz1`, and `/dev/sdz2` are block nodes with the
same major/minor values as the loop devices, not symlinks. Fake `lsblk` output
reports names that exist in sysfs so holder inspection works, while paths
remain `/dev/sdz*` for product discovery policy.

The demo image is verified by a non-interactive full transaction before it is
opened in Ghostty. The interactive session creates a fresh fixture and writes
successful output to the host-mounted `/output` directory.

## Verification

- Reducer tests cover defaults, back navigation, conflicts, preflight facts,
  and completion summary.
- CLI tests cover preflight conversion and completion events.
- Ratatui render tests assert Review and Complete details.
- A Docker smoke script performs full shrink, extraction, decode verification,
  metadata validation, and compressed stream testing.
- Workspace format, Clippy, tests, release build, Ubuntu package matrix, and
  GitHub Release publication gate v0.2.3.
