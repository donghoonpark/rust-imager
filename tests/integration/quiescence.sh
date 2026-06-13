#!/usr/bin/env bash
set -euo pipefail

if [[ $EUID -ne 0 ]]; then
    echo "quiescence.sh must run as root" >&2
    exit 1
fi

work=$(mktemp -d)
image="$work/disk.img"
boot="$work/boot"
root="$work/root"
loop=
cleanup() {
    if mountpoint -q "$boot"; then
        umount "$boot"
    fi
    if mountpoint -q "$root"; then
        umount "$root"
    fi
    if [[ -n "$loop" ]]; then
        losetup -d "$loop" 2>/dev/null || true
    fi
    rm -rf "$work"
}
trap cleanup EXIT

truncate -s 256M "$image"
sfdisk "$image" <<'SFDISK'
label: dos
unit: sectors

start=2048, size=65536, type=c
start=67584, type=83
SFDISK
loop=$(losetup --find --show --partscan "$image")
mkfs.vfat -F 32 "${loop}p1"
mkfs.ext4 -F "${loop}p2"
mkdir "$boot" "$root"
mount "${loop}p1" "$boot"
mount "${loop}p2" "$root"

test_binary=$(
    cargo test -p rust-imager-linux --test quiescence_real --no-run --message-format=json |
        jq -r 'select(.reason == "compiler-artifact" and .target.name == "quiescence_real") | .executable' |
        tail -n 1
)
[[ -x "$test_binary" ]]
env RUST_IMAGER_QUIESCENCE_DISK="$loop" "$test_binary" --ignored --nocapture

if mountpoint -q "$boot" || mountpoint -q "$root"; then
    echo "quiescence left a partition mounted" >&2
    exit 1
fi
echo "whole-disk quiescence verified"
