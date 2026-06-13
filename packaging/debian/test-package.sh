#!/usr/bin/env bash
set -euo pipefail

usage() {
    echo "usage: $0 <expected-version> <amd64|arm64>" >&2
    exit 2
}

[[ $# -eq 2 ]] || usage

expected_version=$1
expected_architecture=$2
binary=${RUST_IMAGER_BINARY:-/usr/sbin/rust-imager}

case "$expected_architecture" in
    amd64 | arm64) ;;
    *)
        echo "unsupported Debian architecture: $expected_architecture" >&2
        exit 1
        ;;
esac

installed_version=$(dpkg-query -W -f='${Version}\n' rust-imager)
[[ "$installed_version" == "$expected_version" ]] || {
    echo "installed version $installed_version does not match $expected_version" >&2
    exit 1
}

installed_architecture=$(dpkg-query -W -f='${Architecture}\n' rust-imager)
[[ "$installed_architecture" == "$expected_architecture" ]] || {
    echo "installed architecture $installed_architecture does not match $expected_architecture" >&2
    exit 1
}

[[ -x "$binary" ]] || {
    echo "rust-imager binary is missing or not executable: $binary" >&2
    exit 1
}

binary_version=$("$binary" --version | awk 'NR == 1 { print $2 }')
upstream_version=${expected_version%%~*}
[[ "$binary_version" == "$upstream_version" ]] || {
    echo "binary version $binary_version does not match $upstream_version" >&2
    exit 1
}

"$binary" --help >/dev/null

linkage=$(ldd "$binary")
if grep -Fq "not found" <<<"$linkage"; then
    echo "unresolved shared library:" >&2
    echo "$linkage" >&2
    exit 1
fi

required_commands=(
    e2fsck
    resize2fs
    sfdisk
    mount
    umount
    partprobe
    lsblk
    findmnt
    blkid
    swapon
    setsid
)

for command_name in "${required_commands[@]}"; do
    command -v "$command_name" >/dev/null || {
        echo "required runtime command is missing: $command_name" >&2
        exit 1
    }
done

echo "rust-imager $expected_version ($expected_architecture) package smoke test passed"
