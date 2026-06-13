#!/usr/bin/env bash
set -euo pipefail

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
workflow="$repo_root/.github/workflows/release.yml"
version_helper="$repo_root/packaging/release-version.sh"

fail() {
    echo "FAIL: $*" >&2
    exit 1
}

expect_failure() {
    if "$@" >/dev/null 2>&1; then
        fail "command unexpectedly succeeded: $*"
    fi
}

[[ -f "$workflow" ]] || fail "release workflow missing"
[[ -x "$version_helper" ]] || fail "release version helper missing"

workspace_version=$(cargo metadata --no-deps --format-version 1 |
    sed -n 's/.*"name":"rust-imager","version":"\([^"]*\)".*/\1/p')
[[ -n "$workspace_version" ]] || fail "could not read workspace version"

[[ $("$version_helper" workflow_dispatch "" 42) == "${workspace_version}~ci42" ]] ||
    fail "manual version is wrong"
[[ $("$version_helper" push "refs/tags/v${workspace_version}" 42) == "$workspace_version" ]] ||
    fail "tag version is wrong"
expect_failure "$version_helper" push "refs/tags/v9.9.9" 42
expect_failure "$version_helper" push "refs/heads/main" 42
expect_failure "$version_helper" workflow_dispatch "" nope

grep -Fq "workflow_dispatch:" "$workflow" || fail "manual trigger missing"
grep -Fq "'v*'" "$workflow" || fail "tag trigger missing"
grep -Fq "contents: read" "$workflow" || fail "read-only default permissions missing"
grep -Fq "contents: write" "$workflow" || fail "publish permission missing"
grep -Fq "amd64" "$workflow" || fail "amd64 build missing"
grep -Fq "arm64" "$workflow" || fail "arm64 build missing"
grep -Fq "ubuntu-24.04-arm" "$workflow" || fail "native arm64 runner missing"
grep -Fq "runs-on: \${{ matrix.runner }}" "$workflow" ||
    fail "architecture jobs do not select native runners"
for version in 18.04 20.04 22.04 24.04 26.04; do
    grep -Fq "$version" "$workflow" || fail "Ubuntu $version compatibility job missing"
done
grep -Fq "needs: [metadata, build, compatibility]" "$workflow" ||
    fail "publish does not require the full matrix"
grep -Fq "github.event_name == 'push'" "$workflow" || fail "publishing is not tag-only"
grep -Fq "SHA256SUMS" "$workflow" || fail "checksums are not generated"
grep -Fq "retention-days:" "$workflow" || fail "manual artifacts have no retention"

if grep -Eq 'uses: [^ ]+@v[0-9]' "$workflow"; then
    fail "GitHub Actions must be pinned to commit SHAs"
fi

echo "Release workflow contract tests passed"
