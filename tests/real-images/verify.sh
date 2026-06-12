#!/usr/bin/env bash
set -euo pipefail

manifest=${1:-tests/real-images/manifest.json}
cache=${RUST_IMAGER_IMAGE_CACHE:-"$HOME/.cache/rust-imager/images"}
mkdir -p "$cache"

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
    echo "validated real image: $id"
done
