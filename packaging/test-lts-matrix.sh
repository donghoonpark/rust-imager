#!/usr/bin/env bash
set -euo pipefail

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
readonly lts_versions=(18.04 20.04 22.04 24.04 26.04)

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
platform=$(platform_for "$architecture")
selected_version=${4:-}

[[ -f "$package" ]] || {
    echo "package does not exist: $package" >&2
    exit 1
}
command -v docker >/dev/null || {
    echo "docker is required" >&2
    exit 1
}
docker buildx version >/dev/null

context=$(mktemp -d)
trap 'rm -rf "$context"' EXIT
cp "$package" "$context/rust-imager.deb"
cp "$repo_root/packaging/debian/test-package.sh" "$context/test-package.sh"

versions=("${lts_versions[@]}")
if [[ -n "$selected_version" ]]; then
    supported=false
    for ubuntu_version in "${lts_versions[@]}"; do
        if [[ "$ubuntu_version" == "$selected_version" ]]; then
            supported=true
            break
        fi
    done
    [[ "$supported" == true ]] || {
        echo "unsupported Ubuntu LTS version: $selected_version" >&2
        exit 1
    }
    versions=("$selected_version")
fi

for ubuntu_version in "${versions[@]}"; do
    echo "Testing rust-imager $version ($architecture) on Ubuntu $ubuntu_version"
    docker buildx build \
        --platform "$platform" \
        --file "$repo_root/packaging/docker/Dockerfile.test" \
        --build-arg "UBUNTU_VERSION=$ubuntu_version" \
        --build-arg "EXPECTED_VERSION=$version" \
        --build-arg "EXPECTED_ARCH=$architecture" \
        --progress plain \
        "$context"
done
