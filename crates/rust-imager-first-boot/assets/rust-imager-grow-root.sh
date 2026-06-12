#!/bin/sh
set -eu

STATE_DIR=/var/lib/rust-imager-grow-root
STATE_FILE=$STATE_DIR/state
UNIT=/etc/systemd/system/rust-imager-grow-root.service
WANTS=/etc/systemd/system/multi-user.target.wants/rust-imager-grow-root.service

mkdir -p "$STATE_DIR"
phase=partition
if [ -r "$STATE_FILE" ]; then
    phase=$(cat "$STATE_FILE")
fi

root_source=$(findmnt -nro SOURCE /)
root_device=$(readlink -f "$root_source")
parent_name=$(lsblk -nro PKNAME "$root_device")
part_number=$(lsblk -nro PARTN "$root_device")

if [ -z "$parent_name" ] || [ -z "$part_number" ]; then
    echo "Unable to identify root disk and partition" >&2
    exit 1
fi

disk=/dev/$parent_name
last_partition=$(lsblk -nrpo NAME,TYPE "$disk" | awk '$2 == "part" { last=$1 } END { print last }')
if [ "$root_device" != "$last_partition" ]; then
    echo "Root partition is not the last partition on $disk" >&2
    exit 1
fi

case "$phase" in
    partition)
        start=$(sfdisk -d "$disk" | awk -v root="$root_device" '
            $1 == root {
                for (i = 1; i <= NF; i++) {
                    if ($i == "start=") {
                        value=$(i + 1)
                        gsub(/,/, "", value)
                        print value
                        exit
                    }
                }
            }')
        if [ -z "$start" ]; then
            echo "Unable to read root partition start" >&2
            exit 1
        fi
        printf 'start=%s, size=+\n' "$start" | sfdisk --no-reread --force -N "$part_number" "$disk"
        sync
        printf '%s\n' filesystem > "$STATE_FILE"
        partprobe "$disk" || true
        systemctl reboot
        ;;
    filesystem)
        resize2fs "$root_device"
        printf '%s\n' cleanup > "$STATE_FILE"
        "$0"
        ;;
    cleanup)
        rm -f "$WANTS" "$UNIT" "$STATE_FILE"
        rmdir "$STATE_DIR" 2>/dev/null || true
        systemctl daemon-reload
        ;;
    *)
        echo "Unknown rust-imager grow-root state: $phase" >&2
        exit 1
        ;;
esac
