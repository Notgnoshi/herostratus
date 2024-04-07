#!/bin/bash
set -o errexit
set -o pipefail
set -o nounset
set -o noclobber

CARGO_MANIFEST="${1:-Cargo.toml}"
MANIFEST_KEY="${2:-version}"

cargo metadata \
    --format-version=1 \
    --manifest-path "$CARGO_MANIFEST" \
    --no-deps |
    jq ".packages[0].$MANIFEST_KEY" |
    tr -d '"'
