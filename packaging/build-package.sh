#!/usr/bin/env bash
set -euo pipefail

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)

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

if [[ ${1:-} == "--print-platform" ]]; then
    [[ $# -eq 2 ]] || exit 2
    platform_for "$2"
    exit
fi

if [[ $# -ne 3 ]]; then
    echo "usage: $0 <version> <amd64|arm64> <output-directory>" >&2
    exit 2
fi

version=$1
architecture=$2
output_dir=$3
platform=$(platform_for "$architecture")

command -v docker >/dev/null || {
    echo "docker is required" >&2
    exit 1
}
docker buildx version >/dev/null

mkdir -p "$output_dir"
docker buildx build \
    --platform "$platform" \
    --file "$repo_root/packaging/docker/Dockerfile.build" \
    --build-arg "DEB_ARCH=$architecture" \
    --build-arg "PACKAGE_VERSION=$version" \
    --target package \
    --output "type=local,dest=$output_dir" \
    --progress plain \
    "$repo_root"

package="$output_dir/rust-imager_${version}_${architecture}.deb"
[[ -f "$package" ]] || {
    echo "Buildx did not produce expected package: $package" >&2
    exit 1
}
echo "$package"
