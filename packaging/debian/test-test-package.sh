#!/usr/bin/env bash
set -euo pipefail

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
smoke_test="$repo_root/packaging/debian/test-package.sh"
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

fakebin="$work/bin"
mkdir -p "$fakebin"

cat >"$fakebin/dpkg-query" <<'EOF'
#!/usr/bin/env bash
case "${FAKE_DPKG_FIELD:-}" in
    version) printf '%s\n' "${FAKE_DPKG_VERSION:-0.1.0}" ;;
    architecture) printf '%s\n' "${FAKE_DPKG_ARCH:-amd64}" ;;
    *)
        case "$*" in
            *Version*) printf '%s\n' "${FAKE_DPKG_VERSION:-0.1.0}" ;;
            *Architecture*) printf '%s\n' "${FAKE_DPKG_ARCH:-amd64}" ;;
            *) exit 1 ;;
        esac
        ;;
esac
EOF

cat >"$fakebin/ldd" <<'EOF'
#!/usr/bin/env bash
printf '%s\n' "${FAKE_LDD_OUTPUT:-libc.so.6 => /lib/libc.so.6}"
EOF

cat >"$fakebin/rust-imager" <<'EOF'
#!/usr/bin/env bash
case "${1:-}" in
    --version) echo "rust-imager ${FAKE_BINARY_VERSION:-0.1.0}" ;;
    --help) echo "usage: rust-imager" ;;
    *) exit 1 ;;
esac
EOF

chmod 0755 "$fakebin/dpkg-query" "$fakebin/ldd" "$fakebin/rust-imager"
for tool in e2fsck resize2fs sfdisk mount umount partprobe lsblk findmnt blkid swapon setsid; do
    cat >"$fakebin/$tool" <<'EOF'
#!/usr/bin/env bash
exit 0
EOF
    chmod 0755 "$fakebin/$tool"
done

run_smoke() {
    env \
        PATH="$fakebin:/usr/bin:/bin" \
        RUST_IMAGER_BINARY="$fakebin/rust-imager" \
        "$smoke_test" "$@"
}

run_smoke "0.1.0" "amd64"
expect_failure env FAKE_DPKG_VERSION=0.2.0 PATH="$fakebin:/usr/bin:/bin" \
    RUST_IMAGER_BINARY="$fakebin/rust-imager" "$smoke_test" "0.1.0" "amd64"
expect_failure env FAKE_DPKG_ARCH=arm64 PATH="$fakebin:/usr/bin:/bin" \
    RUST_IMAGER_BINARY="$fakebin/rust-imager" "$smoke_test" "0.1.0" "amd64"
expect_failure env FAKE_BINARY_VERSION=0.2.0 PATH="$fakebin:/usr/bin:/bin" \
    RUST_IMAGER_BINARY="$fakebin/rust-imager" "$smoke_test" "0.1.0" "amd64"
expect_failure env FAKE_LDD_OUTPUT='liblzma.so.5 => not found' PATH="$fakebin:/usr/bin:/bin" \
    RUST_IMAGER_BINARY="$fakebin/rust-imager" "$smoke_test" "0.1.0" "amd64"

rm "$fakebin/partprobe"
expect_failure run_smoke "0.1.0" "amd64"

echo "Installed package smoke-test contract passed"
