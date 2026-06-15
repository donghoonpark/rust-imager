#!/usr/bin/env bash
set -euo pipefail

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
readonly lts_versions=(18.04 20.04 22.04 24.04)

platform_for() {
    case "$1" in
        amd64) echo "linux/amd64" ;;
        arm64) echo "linux/arm64" ;;
        *)
            echo "unsupported Debian architecture: $1" >&2
            return 1
            ;;
    esac
}

case "${1:-}" in
    --print-lts)
        [[ $# -eq 1 ]] || exit 2
        echo "${lts_versions[*]}"
        exit
        ;;
    --print-platform)
        [[ $# -eq 2 ]] || exit 2
        platform_for "$2"
        exit
        ;;
esac

if [[ $# -lt 3 || $# -gt 4 ]]; then
    echo "usage: $0 <package.deb> <version> <amd64|arm64> [ubuntu-version]" >&2
    exit 2
fi

package=$1
version=$2
architecture=$3
selected_version=${4:-}
platform_for "$architecture" >/dev/null

[[ -f "$package" ]] || {
    echo "package does not exist: $package" >&2
    exit 1
}

versions=("${lts_versions[@]}")
if [[ -n "$selected_version" ]]; then
    [[ " ${lts_versions[*]} " == *" $selected_version "* ]] || {
        echo "unsupported Ubuntu LTS version: $selected_version" >&2
        exit 1
    }
    versions=("$selected_version")
fi

output=$(mktemp -d)
trap 'rm -rf "$output"' EXIT

for ubuntu_version in "${versions[@]}"; do
    image_tag="rust-imager-demo:${version}-${architecture}-ubuntu-${ubuntu_version}"
    echo "Loop-testing rust-imager $version ($architecture) on Ubuntu $ubuntu_version"
    "$repo_root/packaging/demo/build.sh" \
        "$package" "$image_tag" "$ubuntu_version" "$architecture"
    rm -rf "${output:?}"/*
    docker run --rm --privileged \
        --platform "$(platform_for "$architecture")" \
        --volume "$output:/output" \
        "$image_tag" full-run
done
