#!/bin/bash
set -o errexit
set -o pipefail
set -o nounset
#set -o noclobber

REPO=$(git rev-parse --show-toplevel)

RED="\033[31m"
GREEN="\033[32m"
BLUE="\033[34m"
RESET="\033[0m"

## Parallel arrays because an array of tuples is too much to ask for in Bash
BENCHMARK_NAMES=(
    "herostratus"
    "git"
)
BENCHMARK_URLS=(
    "git@github.com:Notgnoshi/herostratus.git"
    "https://github.com/git/git.git"
)
BENCHMARK_BRANCHES=(
    "main"
    "master"
)

debug() {
    echo -e "${BLUE}DEBUG:${RESET} $*" >&2
}

info() {
    echo -e "${GREEN}INFO:${RESET} $*" >&2
}

error() {
    echo -e "${RED}ERROR:${RESET} $*" >&2
}

usage() {
    echo "Usage: $0 [--help]"
    echo
    echo "Benchmark Herostratus on a few different popular repositories"
    echo
    echo "  --help, -h      Show this help and exit"
    echo "  --no-edit, -n   Do not edit the README with benchmark results"
    echo "  --no-run, -r    Do not run the benchmarks (edit the readme with the results from last run)"
}

## Same as run_benchmarks, but output on stdout
run_benchmarks_wrapper() {
    local -r data_dir="$REPO/data"
    rm -f "$data_dir/config.toml"

    for ((i = 0; i < ${#BENCHMARK_NAMES[*]}; ++i)); do
        # Don't want to re-run duplicate check or fetch on previous benchmarks
        herostratus --data-dir "$data_dir" add --name "${BENCHMARK_NAMES[$i]}" "${BENCHMARK_URLS[$i]}" "${BENCHMARK_BRANCHES[$i]}" >&2
    done

    local -r stdout="$data_dir/check.out"
    local -r stderr="$data_dir/check.err"
    herostratus --data-dir "$data_dir" --color check-all --no-fetch --summary \
        >"$stdout" \
        2> >(tee "$stderr" >&2)

    sed -ne '/## Summary/,$p' "$stdout"
}

## Run the benchmarks and save the results in markdown format to the given file
run_benchmarks() {
    local -r data_dir="$1"
    local -r results="$2"

    rm -rf "$data_dir"
    mkdir -p "$data_dir"

    run_benchmarks_wrapper "$data_dir" >"$results"

    info "Benchmark results:"
    cat "$results"
}

## Edit the README to include the given benchmark results
#
# Finds/replaces between two comment tags in the README.
edit_readme_with_results() {
    local -r results="$1"
    local -r readme="$REPO/README.md"

    sed -i 's/## Summary/## Benchmarks/' "$results"

    # NOTE: 'r $results' must be at the end of the line, and cannot have trailing space or comment
    sed -Ei "/START RESULTS/,/END RESULTS/{ /START RESULTS/{p; r $results
        }; /END RESULTS/p; d }" "$readme"
}

main() {
    local no_run="false"
    local no_edit="false"

    while [[ $# -gt 0 ]]; do
        case "$1" in
        --help | -h)
            usage
            exit 0
            ;;
        --no-run | -r)
            no_run="true"
            ;;
        --no-edit | -n)
            no_edit="true"
            ;;
        -*)
            error "Unexpected option: '$1'"
            exit 1
            ;;
        *)
            error "Unexpected positional argument: '$1'"
            exit 1
            ;;
        esac
        shift
    done

    if [[ "$PWD" != "$REPO" ]]; then
        error "This script must be run from the repository root!"
        exit 1
    fi
    if [[ "$no_run" == "false" ]]; then
        cargo build --release
        export PATH="$PATH:$REPO/target/release"
        run_benchmarks "$REPO/data" "$REPO/data/results.md"
    fi

    if [[ "$no_edit" == "false" ]]; then
        edit_readme_with_results "$REPO/data/results.md"
    fi
}

main "$@"
