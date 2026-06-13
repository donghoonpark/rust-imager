# Pre-Hardware Validation Hardening Design

- Status: approved
- Date: 2026-06-13
- Scope: safety and validation work before physical eMMC hardware is available

## 1. Goal

Increase confidence in `rust-imager` without claiming hardware behavior that
cannot be reproduced in CI. The work must close known destructive-operation
gaps, exercise the complete application engine against virtual block devices,
and make remaining physical acceptance requirements explicit.

## 2. Validation Boundary

Automated validation will cover:

- Linux block-device discovery and output-device exclusion
- mounted partitions, swap, and device-holder rejection
- 512-byte logical-sector enforcement
- ext4 shrink and MBR rewrite invariants
- command failure, timeout, and termination handling
- bounded extraction, output-full failures, and retained partial artifacts
- full engine outputs including image, log, metadata, and verification
- first-boot expansion through two real QEMU boots
- pinned Raspberry Pi OS and ODROID image shrink/extract verification
- repeatable throughput and memory-bound checks

The following remain physical acceptance items:

- USB bridge firmware behavior during resets, disconnects, and sustained I/O
- actual eMMC wear, bad-block handling, and power-loss behavior
- board ROM and vendor bootloader behavior after writing the generated image
- USB 2.0 throughput on representative readers and host controllers
- separately exposed eMMC boot0/boot1 areas

## 3. Safety Boundary Changes

### 3.1 Output-device exclusion

The output directory is resolved to an absolute existing parent before device
selection. `findmnt -T` identifies the backing mount source, and block topology
maps that source to its whole-disk ancestor. The same disk cannot be selected as
the source, including when the output was supplied as a relative path, through a
symlink, or beneath a bind mount.

String-prefix matching is not a safety decision.

### 3.2 Target quiescence

Immediately before mutation, the selected disk is inspected again. Mutation is
rejected when:

- any child partition is active swap
- any child has device-mapper or kernel holders
- a child is mounted and cannot be unmounted
- a mount remains after the unmount attempt
- the disk identity or partition table changed since planning

All mounted child partitions are unmounted, not only the root partition.

### 3.3 Sector-size policy

Version 0.1 accepts only devices with a 512-byte logical sector. MBR parsing and
the first-boot script are currently designed around conventional 512-byte DOS
sector semantics. 4Kn support requires a separate design and test matrix.

### 3.4 Destructive command execution

External commands use process groups. Read-only commands have finite timeouts.
Destructive commands have longer finite timeouts and report timeout or signal
termination distinctly.

While a destructive child command is running, the parent handles interactive
termination signals by leaving the child operation in control and reporting
that interruption is unsafe. This reduces accidental terminal cancellation but
does not claim protection from `SIGKILL`, power loss, kernel failure, or device
removal.

## 4. Post-Shrink Invariants

The shrink transaction is successful only after all of these checks pass:

- selected device identity is unchanged
- MBR remains valid, non-GPT, non-overlapping, and in bounds
- all non-root partition entries are byte-for-byte unchanged
- root partition number, type, boot flag, and start LBA are unchanged
- root end LBA exactly matches the immutable plan
- ext4 block size and block count fit within the rewritten partition
- ext4 reported size matches the requested target within `resize2fs` unit
  rounding
- read-only `e2fsck -fn` succeeds

The `Revalidate` transaction stage performs real checks rather than acting as a
placeholder.

## 5. Virtual Block-Device Test Matrix

### 5.1 Discovery and topology tests

Captured `lsblk`/`findmnt` fixtures cover:

- absolute and relative output paths
- symlinked output directories
- bind-mounted output paths
- root, boot, output, swap, holder, USB, non-USB, and changed identities
- unsupported logical sector sizes

### 5.2 Privileged loop tests

Loop-backed MBR/FAT/ext4 images exercise:

- the complete application engine
- all-partition unmount behavior
- source/output same-disk refusal
- final image, JSON sidecar, and durable log creation
- Zstandard and XZ output
- decode and source-reread verification

Tests call the same orchestration code as the CLI. System discovery and command
execution are dependency-injected only where the host cannot naturally expose
the required topology.

### 5.3 Failure injection

Device-mapper and constrained filesystems inject:

- permanent and mid-stream source read failures
- output `ENOSPC`
- `resize2fs` failure
- `sfdisk` failure after filesystem shrink
- partition reread failure after MBR update
- identity or geometry change before mutation

Each case verifies exit status, retained artifacts, log stage, and
`source_modified`.

### 5.4 First-boot QEMU

The QEMU fixture uses a DOS disk with a FAT boot-like partition and ext4 root,
not a single ext4 partition. The compressed image is decoded into a larger disk
before boot. Two boots verify partition growth, filesystem growth, payload
integrity, preservation of the first partition, and service cleanup.

An ARM64 `virt` job may be added when its runtime remains practical. It validates
generic ARM64 Linux/systemd behavior, not Raspberry Pi or ODROID firmware.

## 6. Real Image Validation

Pinned Raspberry Pi OS and ODROID images continue to be:

- checksum verified
- classified
- copied before mutation
- shrunk using production code
- extracted as Zstandard and XZ
- decoded and source-reread verified
- checked with `e2fsck -fn`

The test additionally verifies non-root MBR entries remain unchanged and the
installed first-boot assets exist in the extracted root filesystem.

These tests establish image-layout compatibility, not board bootability.

## 7. Performance Validation

The Criterion benchmark creates or removes a unique output on every iteration.
It must run without output-collision panics.

CI records:

- raw copy throughput
- Zstandard level 3 throughput
- compressed/raw throughput ratio
- peak resident memory for a fixed and a larger input

CI uses generous regression thresholds because shared runners are noisy.
Physical acceptance retains the target of at least 95% of raw sequential read
throughput over USB 2.0 when compression and destination storage are not the
bottleneck.

## 8. Completion Criteria

The hardening work is complete when:

- every safety finding has a regression test
- the full local suite, Clippy, format, release build, shell syntax, and
  benchmark smoke test pass
- privileged loop, failure injection, real-image, and QEMU jobs pass in GitHub
  Actions
- documentation distinguishes automated guarantees from hardware acceptance
- every major implementation checkpoint is committed separately
