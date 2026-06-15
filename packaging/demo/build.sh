#!/usr/bin/env bash
set -euo pipefail

package=${1:?usage: build.sh PACKAGE.deb [IMAGE_TAG] [UBUNTU_VERSION] [amd64|arm64]}
image_tag=${2:-rust-imager-demo:local}
ubuntu_version=${3:-24.04}
architecture=${4:-}
script_dir=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)

if [[ -z "$architecture" ]]; then
    command -v dpkg-deb >/dev/null || {
        echo "architecture is required when dpkg-deb is unavailable" >&2
        exit 1
    }
    architecture=$(dpkg-deb -f "$package" Architecture)
fi

case "$architecture" in
    amd64) platform=linux/amd64 ;;
    arm64) platform=linux/arm64 ;;
    *)
        echo "unsupported Debian architecture: $architecture" >&2
        exit 1
        ;;
esac

cp "$package" "$script_dir/rust-imager.deb"
trap 'rm -f "$script_dir/rust-imager.deb"' EXIT
docker buildx build \
    --load \
    --platform "$platform" \
    --build-arg "UBUNTU_VERSION=$ubuntu_version" \
    --tag "$image_tag" \
    "$script_dir"
