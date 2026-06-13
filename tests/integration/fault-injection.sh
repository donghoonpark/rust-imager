#!/usr/bin/env bash
set -euo pipefail

if [[ $EUID -ne 0 ]]; then
    echo "fault-injection.sh must run as root" >&2
    exit 1
fi

work=$(mktemp -d)
backing="$work/backing"
name=rust-imager-error-$$
tiny="$work/tiny"
cleanup() {
    if mountpoint -q "$tiny"; then
        umount "$tiny"
    fi
    dmsetup remove "$name" 2>/dev/null || true
    if [[ -n ${loop:-} ]]; then
        losetup -d "$loop" 2>/dev/null || true
    fi
    rm -rf "$work"
}
trap cleanup EXIT
truncate -s 64M "$backing"
loop=$(losetup --find --show "$backing")
sectors=$(blockdev --getsz "$loop")
echo "0 $sectors error" | dmsetup create "$name"

test_binary=$(
    cargo test -p rust-imager-pipeline --test block_failure --no-run --message-format=json |
        jq -r 'select(.reason == "compiler-artifact" and .target.name == "block_failure") | .executable' |
        tail -n 1
)
[[ -x "$test_binary" ]]

env \
    RUST_IMAGER_FAILURE_SOURCE="/dev/mapper/$name" \
    RUST_IMAGER_FAILURE_OUTPUT="$work/read-error.img.zst" \
    RUST_IMAGER_FAILURE_BYTES=$((64 * 1024 * 1024)) \
    "$test_binary" --ignored --nocapture

mkdir "$tiny"
mount -t tmpfs -o size=1M tmpfs "$tiny"
dd if=/dev/urandom of="$work/random" bs=1M count=16 status=none
env \
    RUST_IMAGER_FAILURE_SOURCE="$work/random" \
    RUST_IMAGER_FAILURE_OUTPUT="$tiny/output.img.zst" \
    RUST_IMAGER_FAILURE_BYTES=$((16 * 1024 * 1024)) \
    "$test_binary" --ignored --nocapture

echo "pipeline read-error and ENOSPC failures verified"
