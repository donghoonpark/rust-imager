#!/usr/bin/env bash
set -euo pipefail

if [[ $EUID -ne 0 ]]; then
    echo "run-ubuntu-grow-root.sh must run as root" >&2
    exit 1
fi
for tool in curl grub-install losetup mkfs.ext4 mkfs.vfat qemu-system-x86_64 sfdisk zstd; do
    command -v "$tool" >/dev/null
done

release=https://cloud-images.ubuntu.com/releases/noble/release-20250805
archive_name=ubuntu-24.04-server-cloudimg-amd64-root.tar.xz
archive_sha=007a6c2bf9e83bb769bf813c2bf6375f4dbde368538b3747165eec158b430e82
cache=${RUST_IMAGER_IMAGE_CACHE:-/var/cache/rust-imager}
archive="$cache/$archive_name"
mkdir -p "$cache"
if [[ ! -f "$archive" ]] || ! echo "$archive_sha  $archive" | sha256sum -c -; then
    rm -f "$archive"
    curl --fail --location --retry 4 --output "$archive" "$release/$archive_name"
    echo "$archive_sha  $archive" | sha256sum -c -
fi

work=$(mktemp -d)
source_image="$work/ubuntu-compact.img"
compressed="$work/ubuntu-compact.img.zst"
restored="$work/ubuntu-restored.img"
mount_dir="$work/root"
boot_mount="$work/boot"
loop=
cleanup() {
    if mountpoint -q "$boot_mount"; then
        umount "$boot_mount"
    fi
    if mountpoint -q "$mount_dir"; then
        umount -R "$mount_dir"
    fi
    if [[ -n "$loop" ]]; then
        losetup -d "$loop" 2>/dev/null || true
    fi
    rm -rf "$work"
}
trap cleanup EXIT

truncate -s 3G "$source_image"
sfdisk "$source_image" <<'SFDISK'
label: dos
unit: sectors

start=2048, size=131072, type=c, bootable
start=133120, type=83
SFDISK
loop=$(losetup --find --show --partscan "$source_image")
mkfs.vfat -F 32 -n boot "${loop}p1"
mkfs.ext4 -F -L rootfs "${loop}p2"
mkdir -p "$mount_dir" "$boot_mount"
mount "${loop}p2" "$mount_dir"
mount "${loop}p1" "$boot_mount"
echo ubuntu-boot-marker >"$boot_mount/rust-imager-marker"
boot_hash=$(sha256sum "$boot_mount/rust-imager-marker" | cut -d' ' -f1)
umount "$boot_mount"
tar -xJpf "$archive" -C "$mount_dir"
mount --rbind /dev "$mount_dir/dev"
mount --make-rslave "$mount_dir/dev"
mount -t proc proc "$mount_dir/proc"
mount -t sysfs sys "$mount_dir/sys"
rm -f "$mount_dir/etc/resolv.conf"
cp /etc/resolv.conf "$mount_dir/etc/resolv.conf"
printf 'proc /proc proc defaults 0 0\nLABEL=rootfs / ext4 defaults 0 1\n' >"$mount_dir/etc/fstab"
chroot "$mount_dir" apt-get update
chroot "$mount_dir" apt-get install -y \
    e2fsprogs fdisk grub2-common initramfs-tools linux-image-generic parted systemd-sysv
mkdir -p "$mount_dir/usr/lib/rust-imager" \
    "$mount_dir/etc/systemd/system/multi-user.target.wants"
install -m 0755 crates/rust-imager-first-boot/assets/rust-imager-grow-root.sh \
    "$mount_dir/usr/lib/rust-imager/grow-root.sh"
install -m 0644 crates/rust-imager-first-boot/assets/rust-imager-grow-root.service \
    "$mount_dir/etc/systemd/system/rust-imager-grow-root.service"
ln -s ../rust-imager-grow-root.service \
    "$mount_dir/etc/systemd/system/multi-user.target.wants/rust-imager-grow-root.service"
cat >"$mount_dir/etc/systemd/system/rust-imager-qemu-poweroff.service" <<'UNIT'
[Unit]
Description=Power off after Ubuntu rust-imager validation
After=rust-imager-grow-root.service
ConditionPathExists=!/usr/lib/rust-imager/grow-root.sh

[Service]
Type=oneshot
ExecStart=/usr/bin/systemctl poweroff

[Install]
WantedBy=multi-user.target
UNIT
ln -s ../rust-imager-qemu-poweroff.service \
    "$mount_dir/etc/systemd/system/multi-user.target.wants/rust-imager-qemu-poweroff.service"
echo ubuntu-qemu-payload >"$mount_dir/etc/rust-imager-payload"
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

zstd -q -3 "$source_image" -o "$compressed"
zstd -q -d "$compressed" -o "$restored"
truncate -s 4G "$restored"

for boot in 1 2; do
    status=0
    timeout 240 qemu-system-x86_64 -machine accel=tcg -m 1536 -nographic \
        -drive "file=$restored,format=raw,if=virtio" -no-reboot || status=$?
    [[ $status -eq 0 ]]
    echo "Ubuntu QEMU boot $boot completed"
done

loop=$(losetup --find --show --partscan "$restored")
e2fsck -fn "${loop}p2"
mount "${loop}p2" "$mount_dir"
mount "${loop}p1" "$boot_mount"
test ! -e "$mount_dir/etc/systemd/system/rust-imager-grow-root.service"
echo "$payload_hash  $mount_dir/etc/rust-imager-payload" | sha256sum -c -
echo "$boot_hash  $boot_mount/rust-imager-marker" | sha256sum -c -
partition_sectors=$(sfdisk --json "$restored" | jq '.partitiontable.partitions[1].size')
[[ $partition_sectors -gt 7000000 ]]
echo "Ubuntu 24.04 restored-image expansion verified"
