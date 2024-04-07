#!/bin/bash
# shellcheck disable=SC2154,SC2034

# Function to parse the components of a semantic version string with optional
# prefix.
#
# Params:
#   $1 - The string to parse
#   $2 - The name of the variable to store the output in
#
# Returns:
#   The resulting components are stored in an associative array named according
#   to the second argument passed to the function. (See usage example)
#
#   Exits with code 1 if the given text cannot be parsed
#
# Usage Example:
#
#   source ./parse-semver-version.sh
#
#   local -A tag_components
#   get_tag_semver_components v1.2.3-a+b tag_components
#
#   echo "$tag_components[prefix]"
#   echo "$tag_components[major].$tag_components[minor].$tag_components[patch]"
#   echo "$tag_components[prerelease] -- $tag_components[buildmetadata]"
get_tag_semver_components() {

    local tag="$1"
    # This is a "nameref" variable. $2 holds the name of the variable to update.
    # We declare it as an associative array in the calling function.
    local -n components="$2"

    # Taken from https://gist.github.com/rverst/1f0b97da3cbeb7d93f4986df6e8e5695, which itself is a
    # form of the PCRE regex from https://semver.org modified to work with Bash regular expressions,
    # and to allow an optional prefix for v1.0.0 style Git tags.
    #
    # Use the string "v1.2.3-0.ab.0001a.a+gabcd.xyz" to test.
    #
    # This parses the tag into the following capture groups:
    #             1              2                3                4              5 67                                           8  9                                                10 11            12
    #             (prefix      )?(major         ).(minor         ).(patch        )(-((prerelease                                )(  (                                          )) ))?( +(buildmetadata(               ) ))?
    local regex='^([a-zA-Z_/-]+)?(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)(-((0|[1-9][0-9]*|[0-9]*[a-zA-Z-][0-9a-zA-Z-]*)(\.(0|[1-9][0-9]*|[0-9]*[a-zA-Z-][0-9a-zA-Z-]*))*))?(\+([0-9a-zA-Z-]+(\.[0-9a-zA-Z-]+)*))?$'

    if [[ $tag =~ $regex ]]; then
        components[prefix]="${BASH_REMATCH[1]}"
        components[major]="${BASH_REMATCH[2]}"
        components[minor]="${BASH_REMATCH[3]}"
        components[patch]="${BASH_REMATCH[4]}"
        components[prerelease]="${BASH_REMATCH[6]}"
        components[buildmetadata]="${BASH_REMATCH[11]}"
    else
        echo "Failed to parse SemVer 2.0.0 tag '$tag'" >&2
        exit 1
    fi

    # The "components" array is "returned" via namerefs, similar to pass-by-reference in C++.
}
