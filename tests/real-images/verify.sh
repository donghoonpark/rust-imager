#!/usr/bin/env bash
set -euo pipefail

manifest=${1:-tests/real-images/manifest.json}
cache=${RUST_IMAGER_IMAGE_CACHE:-"$HOME/.cache/rust-imager/images"}
mkdir -p "$cache"
test_binary=$(
    cargo test -p rust-imager-linux --test real_image --no-run --message-format=json |
        jq -r 'select(.reason == "compiler-artifact" and .target.name == "real_image") | .executable' |
        tail -n 1
)
[[ -x "$test_binary" ]]

jq -c '.images[]' "$manifest" | while read -r entry; do
    id=$(jq -r '.id' <<<"$entry")
    url=$(jq -r '.url' <<<"$entry")
    archive_sha=$(jq -r '.archive_sha256' <<<"$entry")
    image_sha=$(jq -r '.image_sha256 // empty' <<<"$entry")
    archive="$cache/$id.img.xz"
    image="$cache/$id.img"

    if [[ ! -f "$archive" ]] || ! echo "$archive_sha  $archive" | sha256sum -c -; then
        rm -f "$archive"
        curl --fail --location --retry 4 --output "$archive" "$url"
        echo "$archive_sha  $archive" | sha256sum -c -
    fi
    xz --decompress --keep --force "$archive"
    if [[ -n "$image_sha" ]]; then
        echo "$image_sha  $image" | sha256sum -c -
    fi
    [[ $(stat --format=%s "$image") -gt 0 ]]
    table=$(sfdisk --json "$image")
    [[ $(jq -r '.partitiontable.label' <<<"$table") == dos ]]
    jq -e '.partitiontable.partitions | length > 0' <<<"$table" >/dev/null

    work=$(mktemp -d)
    test_image="$work/$id.img"
    output="$work/output"
    loop=
    cleanup() {
        if [[ -n "$loop" ]]; then
            sudo losetup -d "$loop" 2>/dev/null || true
        fi
        rm -rf "$work"
    }
    trap cleanup EXIT
    cp --reflink=auto --sparse=always "$image" "$test_image"
    loop=$(sudo losetup --find --show --partscan "$test_image")
    sudo env \
        RUST_IMAGER_REAL_DISK="$loop" \
        RUST_IMAGER_REAL_FAMILY="$(jq -r '.family' <<<"$entry")" \
        RUST_IMAGER_REAL_SIZE="$(stat --format=%s "$test_image")" \
        RUST_IMAGER_REAL_OUTPUT="$output" \
        "$test_binary" --ignored --nocapture
    sudo losetup -d "$loop"
    loop=
    rm -rf "$work"
    trap - EXIT
    echo "fully validated real image: $id"
done
