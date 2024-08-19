#!/bin/bash
set -o errexit
set -o pipefail
set -o nounset
set -o noclobber

cargo run -- --data-dir data -l TRACE add git@github.com:Notgnoshi/herostratus.git test/simple --name hero-1
cargo run -- --data-dir data -l TRACE add git@github.com:Notgnoshi/herostratus.git test/fixup --name hero-2

bat data/config.toml

cargo run -- --data-dir data -l TRACE check-all
