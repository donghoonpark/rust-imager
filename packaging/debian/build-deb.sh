#!/usr/bin/env bash
set -euo pipefail

usage() {
    echo "usage: $0 <binary> <version> <amd64|arm64> <output-directory>" >&2
    exit 2
}

[[ $# -eq 4 ]] || usage

binary=$1
version=$2
architecture=$3
output_dir=$4
repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)

[[ -f "$binary" && -x "$binary" ]] || {
    echo "binary is missing or not executable: $binary" >&2
    exit 1
}

command -v dpkg >/dev/null || {
    echo "dpkg is required" >&2
    exit 1
}
command -v dpkg-deb >/dev/null || {
    echo "dpkg-deb is required" >&2
    exit 1
}

dpkg --validate-version "$version" >/dev/null 2>&1 || {
    echo "invalid Debian version: $version" >&2
    exit 1
}

case "$architecture" in
    amd64 | arm64) ;;
    *)
        echo "unsupported Debian architecture: $architecture" >&2
        exit 1
        ;;
esac

reported_version=$("$binary" --version | awk 'NR == 1 { print $2 }')
upstream_version=${version%%~*}
[[ "$reported_version" == "$upstream_version" ]] || {
    echo "binary version $reported_version does not match package version $version" >&2
    exit 1
}

[[ -f "$repo_root/README.md" ]] || {
    echo "README.md is missing from repository root" >&2
    exit 1
}
[[ -f "$repo_root/CHANGELOG.md" ]] || {
    echo "CHANGELOG.md is missing from repository root" >&2
    exit 1
}
[[ -f "$repo_root/LICENSE" ]] || {
    echo "LICENSE is missing from repository root" >&2
    exit 1
}

package_root=$(mktemp -d)
trap 'rm -rf "$package_root"' EXIT

install -d -m 0755 \
    "$package_root/DEBIAN" \
    "$package_root/usr/sbin" \
    "$package_root/usr/share/doc/rust-imager"
install -m 0755 "$binary" "$package_root/usr/sbin/rust-imager"
install -m 0644 "$repo_root/README.md" "$package_root/usr/share/doc/rust-imager/README.md"
install -m 0644 "$repo_root/CHANGELOG.md" "$package_root/usr/share/doc/rust-imager/CHANGELOG.md"
install -m 0644 "$repo_root/LICENSE" "$package_root/usr/share/doc/rust-imager/copyright"

installed_size=$(du -sk "$package_root/usr" | awk '{ print $1 }')
cat >"$package_root/DEBIAN/control" <<EOF
Package: rust-imager
Version: $version
Section: utils
Priority: optional
Architecture: $architecture
Maintainer: donghoonpark <donghun94@gmail.com>
Homepage: https://github.com/donghoonpark/rust-imager
Installed-Size: $installed_size
Depends: e2fsprogs, fdisk, mount, parted, util-linux
Description: Fast and safe SBC eMMC imaging tool
 Shrinks supported MBR/FAT/ext4 SBC images before streaming them to a
 compressed image and installs a systemd first-boot root expansion service.
EOF
chmod 0644 "$package_root/DEBIAN/control"

mkdir -p "$output_dir"
package="$output_dir/rust-imager_${version}_${architecture}.deb"
rm -f "$package"
dpkg-deb --root-owner-group --build "$package_root" "$package"
echo "$package"
