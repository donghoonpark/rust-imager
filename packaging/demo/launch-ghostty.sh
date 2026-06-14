#!/usr/bin/env bash
set -euo pipefail

image_tag=${1:-rust-imager-demo:local}
output_dir=${RUST_IMAGER_DEMO_OUTPUT:-"$HOME/Documents/rust-imager-demo-output"}
mkdir -p "$output_dir"

docker rm -f rust-imager-demo-session >/dev/null 2>&1 || true
exec docker run --rm -it \
    --name rust-imager-demo-session \
    --privileged \
    --volume "$output_dir:/output" \
    "$image_tag"
