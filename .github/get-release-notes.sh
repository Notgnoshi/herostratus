#!/bin/bash
set -o errexit
set -o pipefail
set -o nounset
set -o noclobber

RED="\033[31m"
GREEN="\033[32m"
BLUE="\033[34m"
RESET="\033[0m"

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
    echo "Usage: $0 [--help] <VERSION> <DESCRIPTION>"
    echo
    echo "Positional Arguments:"
    echo
    echo "  <VERSION>       The version for which to generate the release notes"
    echo "  <DESCRIPTION>   The project description, because every release notes should describe"
    echo "                  what the project is for new users"
    echo
    echo "Options:"
    echo
    echo "  --help, -h      Show this help and exit"
}

parse_changelog() {
    local -r changelog="$1"
    local -r version="$2"

    # 0,/pat1/d deletes from line 1 to pat1 inclusive
    # /pat2/Q exits without printing on the first line to match pat2
    sed "0,/^# Herostratus - $version -/d;/^# /Q" "$changelog"
}

main() {
    local version=""
    local description=""

    while [[ $# -gt 0 ]]; do
        case "$1" in
        --help | -h)
            usage
            exit 0
            ;;
        -*)
            error "Unexpected option: $1"
            usage >&2
            exit 1
            ;;
        *)
            if [[ -z "$version" ]]; then
                version="$1"
            elif [[ -z "$description" ]]; then
                description="$1"
            fi
            ;;
        esac
        shift
    done

    if [[ -z "$version" ]]; then
        error "Missing required <VERSION> positional argument"
        exit 1
    elif [[ -z "$description" ]]; then
        error "Missing required <DESCRIPTION> positional argument"
        exit 1
    fi

    local repo_dir
    repo_dir=$(git rev-parse --show-toplevel)
    local -r changelog="$repo_dir/CHANGELOG.md"
    if [[ ! -f "$changelog" ]]; then
        error "Could not find '$changelog'"
        exit 1
    fi

    # This outputs the version header to stdout
    if ! grep "^# Herostratus - $version -" "$changelog"; then
        error "Could not find version '$version' in '$changelog'"
        exit 1
    fi
    # Add the project description to the release notes
    echo "$description"
    # This outputs from the version header (exclusive) to the next version header (exclusive)
    parse_changelog "$changelog" "$version"
}

main "$@"
