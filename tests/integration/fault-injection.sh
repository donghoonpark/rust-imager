#!/usr/bin/env bash
set -euo pipefail

if [[ $EUID -ne 0 ]]; then
    echo "fault-injection.sh must run as root" >&2
    exit 1
fi

backing=$(mktemp)
name=rust-imager-flakey-$$
cleanup() {
    dmsetup remove "$name" 2>/dev/null || true
    rm -f "$backing"
}
trap cleanup EXIT
truncate -s 64M "$backing"
loop=$(losetup --find --show "$backing")
trap 'losetup -d "$loop" 2>/dev/null || true; cleanup' EXIT
sectors=$(blockdev --getsz "$loop")
echo "0 $sectors flakey $loop 0 1 1" | dmsetup create "$name"
sleep 2
if dd if="/dev/mapper/$name" of=/dev/null bs=1M status=none; then
    echo "expected injected read failure" >&2
    exit 1
fi
echo "dm-flakey read failure observed"
