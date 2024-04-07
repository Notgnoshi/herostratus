#!/bin/bash

set -o errexit
set -o pipefail
set -o nounset
set -o noclobber

SCRIPT_DIR=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" &>/dev/null && pwd)

usage() {
    cat <<EOF
Usage: $0 [--help] <VERSION>

The script will exit with non-zero status if it fails to parse the given version number.

Note - semver doesn't actually define any rules relating to a prefix; but we use pretty
frequently internally (e.g. making a tag like vx.y.z), so this script supports finding
a prefix before a valid semantic version.

If the version does parse, then the different version components will be spit out in the following
format:

    $ $0 v2.3.1-rc2+g1234
    prefix: v
    major: 2
    minor: 3
    patch: 1
    prerelease: rc2
    buildmetadata: g1234
EOF
}

main() {
    local version=""
    while test $# -gt 0; do
        case "$1" in
        --help | -h)
            usage
            exit
            ;;
        *)
            if [[ -z "$version" ]]; then
                version="$1"
            else
                echo "Unexpected argument '$1'" >&2
                exit 1
            fi
            ;;
        esac
        shift
    done

    if [[ -z "$version" ]]; then
        echo "Missing required <VERSION> argument" >&2
        exit 1
    fi

    # shellcheck disable=SC1091
    source "${SCRIPT_DIR}/parse-semver-version.sh"

    # shellcheck disable=SC2034
    local -A version_components
    # Exits the script if it fails to parse the version components
    get_tag_semver_components "$version" version_components

    echo "prefix: ${version_components[prefix]}"
    echo "major: ${version_components[major]}"
    echo "minor: ${version_components[minor]}"
    echo "patch: ${version_components[patch]}"
    echo "prerelease: ${version_components[prerelease]}"
    echo "buildmetadata: ${version_components[buildmetadata]}"
}

main "$@"
