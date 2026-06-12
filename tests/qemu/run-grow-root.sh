#!/usr/bin/env bash
set -euo pipefail

if [[ $EUID -ne 0 ]]; then
    echo "run-grow-root.sh must run as root" >&2
    exit 1
fi
for tool in debootstrap grub-install qemu-system-x86_64 losetup sfdisk mkfs.ext4; do
    command -v "$tool" >/dev/null
done

work=$(mktemp -d)
image="$work/grow-root.img"
mount_dir="$work/root"
loop=
cleanup() {
    if mountpoint -q "$mount_dir"; then
        umount -R "$mount_dir"
    fi
    if [[ -n "$loop" ]]; then
        losetup -d "$loop" 2>/dev/null || true
    fi
    rm -rf "$work"
}
trap cleanup EXIT

truncate -s 1400M "$image"
sfdisk "$image" <<'SFDISK'
label: dos
unit: sectors

start=2048, type=83, bootable
SFDISK
loop=$(losetup --find --show --partscan "$image")
mkfs.ext4 -F -L rootfs "${loop}p1"
mkdir -p "$mount_dir"
mount "${loop}p1" "$mount_dir"
debootstrap --variant=minbase bookworm "$mount_dir" http://deb.debian.org/debian
mount --rbind /dev "$mount_dir/dev"
mount --make-rslave "$mount_dir/dev"
mount -t proc proc "$mount_dir/proc"
mount -t sysfs sys "$mount_dir/sys"
echo 'root:rust-imager' | chroot "$mount_dir" chpasswd
printf 'proc /proc proc defaults 0 0\nLABEL=rootfs / ext4 defaults 0 1\n' >"$mount_dir/etc/fstab"
chroot "$mount_dir" apt-get update
chroot "$mount_dir" apt-get install -y \
    fdisk grub2-common initramfs-tools linux-image-amd64 systemd-sysv
mkdir -p "$mount_dir/usr/lib/rust-imager" \
    "$mount_dir/etc/systemd/system/multi-user.target.wants"
install -m 0755 crates/rust-imager-first-boot/assets/rust-imager-grow-root.sh \
    "$mount_dir/usr/lib/rust-imager/grow-root.sh"
install -m 0644 crates/rust-imager-first-boot/assets/rust-imager-grow-root.service \
    "$mount_dir/etc/systemd/system/rust-imager-grow-root.service"
ln -s ../rust-imager-grow-root.service \
    "$mount_dir/etc/systemd/system/multi-user.target.wants/rust-imager-grow-root.service"
echo qemu-payload >"$mount_dir/etc/rust-imager-payload"
payload_hash=$(sha256sum "$mount_dir/etc/rust-imager-payload" | cut -d' ' -f1)
cat >"$mount_dir/etc/default/grub" <<'GRUB'
GRUB_TIMEOUT=0
GRUB_CMDLINE_LINUX="console=ttyS0"
GRUB_TERMINAL=serial
GRUB_SERIAL_COMMAND="serial --unit=0 --speed=115200"
GRUB
grub-install --target=i386-pc --boot-directory="$mount_dir/boot" "$loop"
chroot "$mount_dir" update-grub
sync
umount -R "$mount_dir"
losetup -d "$loop"
loop=

truncate -s 2200M "$image"
status=0
timeout 180 qemu-system-x86_64 -machine accel=tcg -m 1024 -nographic \
    -drive "file=$image,format=raw,if=virtio" -no-reboot || status=$?
[[ $status -eq 0 || $status -eq 124 ]]
status=0
timeout 180 qemu-system-x86_64 -machine accel=tcg -m 1024 -nographic \
    -drive "file=$image,format=raw,if=virtio" -no-reboot || status=$?
[[ $status -eq 0 || $status -eq 124 ]]

loop=$(losetup --find --show --partscan "$image")
e2fsck -fn "${loop}p1"
mount "${loop}p1" "$mount_dir"
test ! -e "$mount_dir/etc/systemd/system/rust-imager-grow-root.service"
echo "$payload_hash  $mount_dir/etc/rust-imager-payload" | sha256sum -c -
partition_sectors=$(sfdisk --json "$image" | jq '.partitiontable.partitions[0].size')
[[ $partition_sectors -gt 3500000 ]]
echo "QEMU first-boot expansion verified"
