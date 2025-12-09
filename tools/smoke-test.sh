#!/bin/bash
set -o errexit
set -o pipefail
set -o nounset
set -o noclobber

REPO=$(git rev-parse --show-toplevel)

rm -rf "$REPO/data"

# Use one with SSH
cargo run -- --data-dir "$REPO/data" add git@github.com:Notgnoshi/herostratus.git test/simple --name hero-1
# Use a few different branches against the same repo
cargo run -- --data-dir "$REPO/data" add git@github.com:Notgnoshi/herostratus.git test/fixup --name hero-2
# Use one with HTTPS
cargo run -- --data-dir "$REPO/data" add https://github.com/Notgnoshi/herostratus.git main --name hero-3 --path "$REPO/data/git/hero-3"

cat "$REPO/data/config.toml"

cargo run -- --data-dir "$REPO/data" check-all --summary
