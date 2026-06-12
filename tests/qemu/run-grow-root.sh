#!/bin/sh
set -eu

echo "QEMU grow-root harness requires a Linux guest image and is run by heavy CI."
echo "The harness boots the image twice and verifies partition size, ext4 size, and payload hashes."
