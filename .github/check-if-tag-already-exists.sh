#!/bin/bash

set -o errexit
set -o pipefail
set -o nounset
set -o noclobber

SCRIPT_DIR=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" &>/dev/null && pwd)

usage() {
    cat <<EOF
Usage: $0 [--help] [--tag-prefix TAG] [--branch-prefix BRANCH] <VERSION>

Check if a git tag exists in the current repo that should prevent creating a
specific new tag.

--help, -h              Show this help and exit
--tag-prefix, -t        Tag prefix. Defaults to 'v'
--branch-prefix, -b     Release branch prefix. Defaults to 'release/'
--target-branch         Target branch. Defaults to 'master'

The following checks are performed
* A tag which is equivalent to the target version does not exist
* A non-prerelease tag that matches the target version does not already exist
* A separate release branch does not exist for the major.minor of the target
    version

A semantic version is defined as equivalent if the major, minor, patch
and prerelease information are all the same. This means that two versions may
not be differentiated by only the build metadata, regardless of if only one of
the versions being compared contains it.

Exits with non-zero if you should not be allowed to create the target version
EOF
}

check_if_tag_already_exists() {
    local -n components="$1"
    local tag_prefix="$2"

    # If a tag matching the prerelease and/or metadata exactly is found, then don't allow
    #
    # If a tag with prerelease and/or metadata is found, but our current version has a different
    # prerelease/metadata component, allow it
    #
    # If there is no prerelease/metadata and the tag matches exactly, then don't allow
    #
    # Allowed:
    # Existing      | New
    # --------------|-----------
    # v1.0.0        | v0.0.1
    # v1.0.0        | v0.0.0-rc1
    # v1.0.0-rc1    | v1.0.0-rc2
    # v1.0.0-rc1+g1 | v1.0.0-rc2+g1
    #
    # Disallowed:
    # Existing     | New
    # -------------|------------
    # v1.0.0       | v1.0.0
    # v1.0.0-rc1   | v1.0.0-rc1
    # v1.0.0+g1234 | v1.0.0+gabcd

    # If you're making a prerelease, check to see that a "real" release hasn't been made, and that
    # no duplicate prerelease has been made
    if [[ -n "${components[prerelease]}" ]]; then
        local pattern="${tag_prefix}${components[major]}.${components[minor]}.${components[patch]}-${components[prerelease]}"
        local tags
        tags="$(git for-each-ref --format='%(refname:short)' "refs/tags/${pattern}*")"
        if [[ -n "$tags" ]]; then
            echo "Found existing prerelease tags:" >&2
            for tag in $tags; do
                echo "    $tag" >&2
            done
            exit 1
        fi

        # Also check if there's a "real" release for that version
        pattern="${tag_prefix}${components[major]}.${components[minor]}.${components[patch]}"
        tags="$(git for-each-ref --format='%(refname:short)' "refs/tags/${pattern}")"
        if [[ -n "$tags" ]]; then
            echo "You tried to make a prerelease of a version that was already released:" >&2
            for tag in $tags; do
                echo "    $tag" >&2
            done
            exit 1
        fi
        tags="$(git for-each-ref --format='%(refname:short)' "refs/tags/${pattern}+*")"
        if [[ -n "$tags" ]]; then
            echo "You tried to make a prerelease of a version that was already released:" >&2
            for tag in $tags; do
                echo "    $tag" >&2
            done
            exit 1
        fi
    # If there's no prerelease, we can't even re-use the same major.minor.patch, even if the build
    # metadata is different
    else
        local pattern="${tag_prefix}${components[major]}.${components[minor]}.${components[patch]}"
        local tags
        tags="$(git for-each-ref --format='%(refname:short)' "refs/tags/${pattern}")"
        if [[ -n "$tags" ]]; then
            echo "Found existing release tags:" >&2
            for tag in $tags; do
                echo "    $tag" >&2
            done
            exit 1
        fi
        tags="$(git for-each-ref --format='%(refname:short)' "refs/tags/${pattern}+*")"
        if [[ -n "$tags" ]]; then
            echo "Found existing release tag with metadata:" >&2
            for tag in $tags; do
                echo "    $tag" >&2
            done
            exit 1
        fi
    fi

}

check_if_maintenance_release_made_on_non_maintenance_branch() {
    local -n components="$1"
    local branch_prefix="$2"
    local target_branch="$3"

    local major_minor="${components[major]}.${components[minor]}"

    # Has there been a release branch for the specified version been made?
    local branches
    branches="$(git for-each-ref --format='%(refname:short)' "refs/heads/${branch_prefix}${major_minor}*")"
    if [[ -n "$branches" ]]; then
        echo "Found release branch for $major_minor: ${branches}"
        for branch in $branches; do
            if [[ "$branch" = "$target_branch" ]]; then
                echo "...but that branch was the target branch, so that's okay."
                return 0
            fi
        done
        echo "If a release branch has been made for $major_minor, you can't release a new $major_minor version unless done from that release branch"
        exit 1
    fi
}

main() {
    local version=""
    local tag_prefix="v"
    local branch_prefix="release/"
    local target_branch="main"

    while test $# -gt 0; do
        case "$1" in
        --help | -h)
            usage
            exit
            ;;
        --tag-prefix | -t)
            tag_prefix="$2"
            shift
            ;;
        --branch-prefix | -b)
            branch_prefix="$2"
            shift
            ;;
        --target-branch)
            target_branch="$2"
            shift
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
    # nameref array passing confuses shellcheck
    local -A version_components
    # Exits the script if it fails to parse the version components
    get_tag_semver_components "$version" version_components

    check_if_tag_already_exists version_components "$tag_prefix"
    check_if_maintenance_release_made_on_non_maintenance_branch version_components "$branch_prefix" "$target_branch"
}

main "$@"
