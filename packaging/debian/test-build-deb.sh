#!/usr/bin/env bash
set -euo pipefail

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
builder="$repo_root/packaging/debian/build-deb.sh"
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

fail() {
    echo "FAIL: $*" >&2
    exit 1
}

expect_failure() {
    if "$@" >/dev/null 2>&1; then
        fail "command unexpectedly succeeded: $*"
    fi
}

make_binary() {
    local path=$1
    local version=$2
    cat >"$path" <<EOF
#!/usr/bin/env bash
if [[ "\${1:-}" == "--version" ]]; then
    echo "rust-imager $version"
    exit 0
fi
exit 0
EOF
    chmod 0755 "$path"
}

binary="$work/rust-imager"
output="$work/dist"
make_binary "$binary" "0.1.0"

"$builder" "$binary" "0.1.0" "amd64" "$output"

package="$output/rust-imager_0.1.0_amd64.deb"
[[ -f "$package" ]] || fail "package was not created"
[[ $(dpkg-deb -f "$package" Package) == "rust-imager" ]] || fail "wrong package name"
[[ $(dpkg-deb -f "$package" Version) == "0.1.0" ]] || fail "wrong package version"
[[ $(dpkg-deb -f "$package" Architecture) == "amd64" ]] || fail "wrong architecture"
[[ $(dpkg-deb -f "$package" Depends) == "e2fsprogs, fdisk, mount, parted, util-linux" ]] ||
    fail "wrong dependencies"

dpkg-deb -x "$package" "$work/root"
[[ -x "$work/root/usr/sbin/rust-imager" ]] || fail "binary is not executable"
[[ -f "$work/root/usr/share/doc/rust-imager/README.md" ]] || fail "README missing"
[[ -f "$work/root/usr/share/doc/rust-imager/CHANGELOG.md" ]] || fail "changelog missing"
[[ -f "$work/root/usr/share/doc/rust-imager/copyright" ]] || fail "copyright missing"
[[ $(stat -c '%a' "$work/root/usr/sbin/rust-imager") == "755" ]] || fail "wrong binary mode"

expect_failure "$builder" "$binary" "not-a-version" "amd64" "$output"
expect_failure "$builder" "$binary" "0.1.0" "i386" "$output"
expect_failure "$builder" "$binary" "0.2.0" "amd64" "$output"
expect_failure "$builder" "$work/missing" "0.1.0" "amd64" "$output"

echo "Debian package builder tests passed"
