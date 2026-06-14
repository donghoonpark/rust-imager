#!/usr/bin/env bash
set -euo pipefail

package=${1:?usage: build.sh PACKAGE.deb [IMAGE_TAG]}
image_tag=${2:-rust-imager-demo:local}
script_dir=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)

cp "$package" "$script_dir/rust-imager.deb"
trap 'rm -f "$script_dir/rust-imager.deb"' EXIT
docker build --tag "$image_tag" "$script_dir"
