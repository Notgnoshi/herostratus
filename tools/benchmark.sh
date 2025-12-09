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
}

## Execute a command, and measure its wall-clock execution time in seconds
#
# Does not capture command's stderr/stdout output. Saves the execution time to the global variable
# EXEC_RUNTIME.
EXEC_RUNTIME="0"
exec_with_timing() {
    debug "Executing: $*"
    local -r start=$(date +%s.%2N)
    "$@"
    local -r end=$(date +%s.%2N)
    EXEC_RUNTIME=$(awk "BEGIN {print $end - $start}")
}

## Run a single benchmark on the given repository
#
## Outputs the results to stdout as a single row in a markdown table.
run_benchmark() {
    local -r data_dir="$1"
    local -r name="$2"
    local -r url="$3"
    local -r branch="$4"

    exec_with_timing herostratus --data-dir "$data_dir" add --name "$name" "$url" "$branch" >&2
    local -r add_time="$EXEC_RUNTIME"

    local -r stdout="$data_dir/check-$name.out"
    local -r stderr="$data_dir/check-$name.err"

    exec_with_timing herostratus --data-dir "$data_dir" --color check-all --no-fetch \
        >"$stdout" \
        2> >(tee "$stderr" >&2)
    local -r check_time="$EXEC_RUNTIME"

    local num_achievements
    num_achievements="$(grep "Generated.*achievements" "$stderr" | sed -En 's/^.*Generated ([0-9]+) achievements.*$/\1/p')"

    local num_commits
    num_commits="$(grep "Generated.*achievements" "$stderr" | sed -En 's/^.*processing ([0-9]+) commits.*$/\1/p')"
    echo "| $name | $branch | $num_commits | ${add_time}s | ${check_time}s | $num_achievements |"
}

## Same as run_benchmarks, but output on stdout
run_benchmarks_wrapper() {
    local -r data_dir="$REPO/data"

    echo "| Repository | Branch | # Commits | Clone time | Processing time| # Achievements |"
    echo "|------------|--------|-----------|------------|----------------|----------------|"
    for ((i = 0; i < ${#BENCHMARK_NAMES[*]}; ++i)); do
        # Don't want to re-run duplicate check or fetch on previous benchmarks
        rm -f "$data_dir/config.toml"
        run_benchmark "$data_dir" "${BENCHMARK_NAMES[$i]}" "${BENCHMARK_URLS[$i]}" "${BENCHMARK_BRANCHES[$i]}"
    done
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
