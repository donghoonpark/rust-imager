#!/usr/bin/env bash
set -euo pipefail

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
build_driver="$repo_root/packaging/build-package.sh"
matrix_driver="$repo_root/packaging/test-lts-matrix.sh"
loop_matrix_driver="$repo_root/packaging/test-lts-loop-matrix.sh"
build_dockerfile="$repo_root/packaging/docker/Dockerfile.build"
test_dockerfile="$repo_root/packaging/docker/Dockerfile.test"
demo_dockerfile="$repo_root/packaging/demo/Dockerfile"
demo_entrypoint="$repo_root/packaging/demo/entrypoint.sh"
dockerignore="$repo_root/.dockerignore"

fail() {
    echo "FAIL: $*" >&2
    exit 1
}

expect_failure() {
    if "$@" >/dev/null 2>&1; then
        fail "command unexpectedly succeeded: $*"
    fi
}

[[ -f "$build_dockerfile" ]] || fail "build Dockerfile missing"
[[ -f "$test_dockerfile" ]] || fail "test Dockerfile missing"
[[ -f "$demo_dockerfile" ]] || fail "demo Dockerfile missing"
[[ -x "$loop_matrix_driver" ]] || fail "loop matrix driver missing"
[[ -f "$dockerignore" ]] || fail ".dockerignore missing"
grep -Fxq 'target' "$dockerignore" || fail "Rust build output is not excluded from Docker context"
grep -Fxq "FROM --platform=\$TARGETPLATFORM ubuntu:18.04 AS build" "$build_dockerfile" ||
    fail "build userspace is not pinned to Ubuntu 18.04"
grep -Fq 'rust-toolchain.toml' "$build_dockerfile" ||
    fail "build does not use the pinned Rust toolchain"
grep -Fq 'ARG UBUNTU_VERSION' "$test_dockerfile" ||
    fail "test image does not parameterize Ubuntu"
grep -Fq 'ARG UBUNTU_VERSION' "$demo_dockerfile" ||
    fail "demo image does not parameterize Ubuntu"
grep -Fq 'real_lsblk=' "$demo_entrypoint" ||
    fail "demo does not execute the distribution lsblk"

expected_lts='18.04 20.04 22.04 24.04'
actual_lts=$("$matrix_driver" --print-lts)
[[ "$actual_lts" == "$expected_lts" ]] || fail "wrong LTS matrix: $actual_lts"
actual_loop_lts=$("$loop_matrix_driver" --print-lts)
[[ "$actual_loop_lts" == "$expected_lts" ]] || fail "wrong loop LTS matrix: $actual_loop_lts"

[[ $("$build_driver" --print-platform amd64) == "linux/amd64" ]] ||
    fail "wrong amd64 platform mapping"
[[ $("$build_driver" --print-platform arm64) == "linux/arm64" ]] ||
    fail "wrong arm64 platform mapping"
[[ $("$matrix_driver" --print-platform amd64) == "linux/amd64" ]] ||
    fail "wrong matrix amd64 platform mapping"
[[ $("$matrix_driver" --print-platform arm64) == "linux/arm64" ]] ||
    fail "wrong matrix arm64 platform mapping"
[[ $("$loop_matrix_driver" --print-platform amd64) == "linux/amd64" ]] ||
    fail "wrong loop matrix amd64 platform mapping"
[[ $("$loop_matrix_driver" --print-platform arm64) == "linux/arm64" ]] ||
    fail "wrong loop matrix arm64 platform mapping"

expect_failure "$build_driver" --print-platform i386
expect_failure "$matrix_driver" --print-platform i386
expect_failure "$loop_matrix_driver" --print-platform i386
expect_failure "$build_driver"
expect_failure "$matrix_driver"
expect_failure "$loop_matrix_driver"

echo "Container packaging driver tests passed"
