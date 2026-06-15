#!/usr/bin/env bash
set -euo pipefail

output=/output/demo-full-run.img.zst
rm -f "$output" "$output.json" "$output.log" "$output.partial"

rust-imager image \
    --device /dev/sdz \
    --output "$output" \
    --confirm-model "Virtual eMMC Reader" \
    --compression zstd \
    --level 1 \
    --verify decode

test -s "$output"
test -s "$output.json"
test -s "$output.log"
zstd -q -t "$output"
jq -e '
    .verification.status == "passed"
    and .compressed_bytes > 0
    and .raw_bytes > .compressed_bytes
    and (.partitions | length) == 2
' "$output.json" >/dev/null
grep -q 'result=success' "$output.log"
grep -q 'source_modified=true' "$output.log"

rust-imager flash \
    --image "$output" \
    --device /dev/sdy \
    --confirm-model "Virtual Flash Target" \
    --pre-verify full \
    --post-verify full

raw_bytes=$(jq -r '.raw_bytes' "$output.json")
zstd -q -d -c "$output" | cmp -n "$raw_bytes" - /dev/sdy

printf 'Full imaging demo passed: %s\n' "$output"
