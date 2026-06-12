#!/usr/bin/env bash
set -euo pipefail

output=${1:-target/fixtures/sbc-mbr.img}
size_mib=${RUST_IMAGER_FIXTURE_MIB:-1024}
mkdir -p "$(dirname "$output")"
truncate -s "${size_mib}M" "$output"
sfdisk "$output" <<'SFDISK'
label: dos
unit: sectors

start=2048, size=131072, type=c, bootable
start=133120, type=83
SFDISK

loop=$(sudo losetup --find --show --partscan "$output")
cleanup() {
    mountpoint -q /tmp/rust-imager-fixture-boot && sudo umount /tmp/rust-imager-fixture-boot
    mountpoint -q /tmp/rust-imager-fixture-root && sudo umount /tmp/rust-imager-fixture-root
    sudo losetup --detach "$loop" 2>/dev/null || true
}
trap cleanup EXIT
sudo mkfs.vfat -F 32 -n boot "${loop}p1"
sudo mkfs.ext4 -F -L rootfs "${loop}p2"
sudo mkdir -p /tmp/rust-imager-fixture-boot /tmp/rust-imager-fixture-root
sudo mount "${loop}p1" /tmp/rust-imager-fixture-boot
sudo mount "${loop}p2" /tmp/rust-imager-fixture-root
echo fixture | sudo tee /tmp/rust-imager-fixture-boot/config.txt >/dev/null
echo firmware | sudo tee /tmp/rust-imager-fixture-boot/start4.elf >/dev/null
sudo mkdir -p /tmp/rust-imager-fixture-root/{etc,usr,var}
echo payload | sudo tee /tmp/rust-imager-fixture-root/etc/rust-imager-fixture >/dev/null
sync
