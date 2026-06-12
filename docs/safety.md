# Safety Model

## Destructive Boundary

Analysis and wizard configuration are cancellable. Once filesystem checking or
mutation starts, normal cancellation is not offered. Do not terminate the
process, remove the reader, or interrupt power during:

1. `e2fsck`
2. first-boot service installation
3. `resize2fs`
4. `sfdisk`
5. partition-table reread and revalidation

The program does not attempt automatic rollback after a failed destructive
stage. Reversing a partially completed filesystem/partition change without
knowing the exact completed operation can cause further damage.

## Device Selection

Only whole USB disks matching `/dev/sd[a-z]+` are considered. The running
system's root/boot disk, active swap disks, and the output filesystem's disk are
excluded. The selected path, major/minor number, serial, model, capacity,
logical sector size, and transport are captured and re-read before mutation.

The exact model string must be typed before proceeding. Unknown but structurally
compatible layouts display a strong warning. Profile detection is a heuristic;
it never bypasses mandatory MBR/FAT/final-primary-ext4 checks.

## Recovery

If failure occurs before `resize2fs`, unmount the eMMC and inspect it with
`e2fsck -fn`.

If ext4 shrink succeeded but the MBR update failed, do not grow or format the
filesystem. Recover the original start sector and set the partition end to at
least the shrunken ext4 size before running `e2fsck`.

If MBR shrink succeeded, the source is intentionally smaller. It can remain in
that state. A restored image expands on first boot through
`rust-imager-grow-root.service`.

Always retain the `.log` file and avoid write operations until the completed
stage is known.

## Reader Assumption

The USB reader must expose all boot-critical data in the `/dev/sdX` address
space. RPMB and separately exposed eMMC boot hardware partitions are outside
the initial release.
