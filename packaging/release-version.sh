#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 3 ]]; then
    echo "usage: $0 <event-name> <git-ref> <run-number>" >&2
    exit 2
fi

event_name=$1
git_ref=$2
run_number=$3
repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
workspace_version=$(sed -n '/^\[workspace\.package\]/,/^\[/ {
    s/^version = "\([^"]*\)"/\1/p
}' "$repo_root/Cargo.toml")

[[ -n "$workspace_version" ]] || {
    echo "unable to read workspace.package.version" >&2
    exit 1
}
[[ "$run_number" =~ ^[0-9]+$ ]] || {
    echo "run number must be numeric" >&2
    exit 1
}

case "$event_name" in
    workflow_dispatch)
        echo "${workspace_version}~ci${run_number}"
        ;;
    push)
        expected_ref="refs/tags/v${workspace_version}"
        [[ "$git_ref" == "$expected_ref" ]] || {
            echo "release ref $git_ref does not match $expected_ref" >&2
            exit 1
        }
        echo "$workspace_version"
        ;;
    *)
        echo "unsupported release event: $event_name" >&2
        exit 1
        ;;
esac
