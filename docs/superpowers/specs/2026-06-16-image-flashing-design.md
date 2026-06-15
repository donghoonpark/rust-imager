# Image Flashing Design

## Goal

Add a guarded restore path that writes raw, Zstandard, or XZ disk images to an
external USB `/dev/sdX` device. The feature must preserve the existing device
safety policy, stream without a full raw temporary file, optionally validate
the input before writing, optionally reread the target after writing, and work
on Ubuntu 18.04, 20.04, 22.04, and 24.04.

## User Interfaces

The CLI gains:

```text
rust-imager flash \
  --image board.img.zst \
  --device /dev/sda \
  --confirm-model "eMMC Reader" \
  --pre-verify basic \
  --post-verify none
```

Supported inputs are `.img`, `.img.zst`, and `.img.xz`. `--pre-verify` accepts
`none`, `basic`, or `full` and defaults to `basic`. `--post-verify` accepts
`none` or `full` and defaults to `none`.

The TUI starts with an operation chooser:

1. Create Image
2. Flash Image

Create Image keeps the existing wizard. Flash Image asks for the input image,
pre-verification level, target device, exact target model, optional
post-verification, and a final destructive review.

## Input And Sidecar Policy

Compression is inferred strictly from the filename:

- `.img`: raw
- `.img.zst`: Zstandard
- `.img.xz`: XZ

The adjacent sidecar path follows the current convention, such as
`board.img.zst.json`. A missing sidecar never blocks flashing. Its absence is
shown as a strong warning in the TUI and CLI.

When a sidecar exists it must parse as the supported `ImageMetadata` schema.
Any mismatch discovered at the selected pre-verification level blocks
flashing. A malformed or unsupported sidecar is not silently ignored.

## Pre-Verification

`none` performs only checks required to avoid an invalid write:

- regular input file exists and can be opened;
- compression is recognized;
- raw byte count is known from a trusted sidecar or a complete decode/count
  pass;
- target capacity is at least the raw byte count.

Because target capacity cannot be checked safely without knowing decoded
length, compressed images without sidecars still require one complete
decode/count pass. This is a safety prerequisite rather than integrity
verification.

`basic` additionally:

- completely decodes the stream;
- rejects read or decoder errors;
- parses sector zero as DOS/MBR;
- rejects GPT/protective MBR, extended/logical layouts, overlaps, and
  partitions outside the decoded image range;
- checks sidecar raw size and partition geometry when present.

`full` includes `basic` and additionally:

- calculates the compressed file SHA-256 and compares it with the sidecar;
- calculates the decoded raw SHA-256 and compares it when the sidecar contains
  one;
- treats a sidecar without a raw hash as valid but reports that only compressed
  integrity was available.

## Target Safety

Targets use the existing discovery policy:

- whole external USB `/dev/sdX` disk only;
- 512-byte logical sectors;
- not mounted, swap-backed, held by device mapper, or used by the host;
- exact model string confirmation;
- identity revalidated immediately before the first write.

All target child mounts are unmounted through the existing quiescence adapter.
The tool never accepts `/dev/mmcblkN`, NVMe, loop devices from normal discovery,
or a partition path.

## Streaming Write

The pipeline opens a decoder and writes fixed-size buffers directly to the
target. It maintains a raw SHA-256 while writing, reports byte progress, and
uses bounded memory. It rejects a decoded stream that exceeds the previously
validated raw byte count or target capacity.

On success the writer:

1. flushes buffered data;
2. calls `fsync` on the target;
3. requests a partition-table reread using commands supported by Ubuntu 18.04;
4. emits completion facts including bytes written and raw SHA-256.

No automatic zero-fill is performed after the image range. A larger target is
expected; the installed first-boot service expands the final partition and
filesystem when applicable.

## Post-Verification

`none` ends after durable write and partition reread.

`full` rereads exactly the written byte range from the target and compares its
SHA-256 with the hash calculated while decoding and writing. A mismatch fails
the operation.

## Failure Behavior

Failures before the first write leave the target unchanged. Failures after the
first write are reported as destructive partial failures and the target must
be considered unusable until reflashed. Logs identify the stage, target,
bytes written, sidecar status, and whether mutation began.

SIGINT and SIGTERM are deferred during each individual write/fsync operation,
matching the existing destructive transaction policy.

## Validation

Unit and integration tests cover raw, Zstandard, and XZ input; malformed
streams; missing, valid, and mismatched sidecars; undersized targets; write
failures; post-write hash mismatch; CLI parsing; TUI reducers and rendering.

The Docker release matrix performs a real loop-backed flash transaction on
Ubuntu 18.04, 20.04, 22.04, and 24.04 for amd64 and arm64 packages. It creates
an image, flashes it to a second larger loop disk, rereads the target, and
compares the written range. Heavy CI additionally boots a flashed Ubuntu image
through the existing QEMU first-boot expansion path.

## Release

The completed feature is released as `0.3.0`.
