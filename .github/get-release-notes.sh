#!/bin/bash
set -o errexit
set -o pipefail
set -o nounset

VERSION="${1:?Usage: $0 <VERSION> <DESCRIPTION>}"
DESCRIPTION="${2:?Usage: $0 <VERSION> <DESCRIPTION>}"

CHANGELOG="$(git rev-parse --show-toplevel)/CHANGELOG.md"

# Output the version header
if ! grep "^# Herostratus - $VERSION -" "$CHANGELOG"; then
    echo "ERROR: Could not find version '$VERSION' in '$CHANGELOG'" >&2
    exit 1
fi

# Add the project description
echo "$DESCRIPTION"

# Extract everything between this version's header and the next
# 0,/pat1/d deletes from line 1 to pat1 inclusive
# /pat2/Q exits without printing on the first line to match pat2
sed "0,/^# Herostratus - $VERSION -/d;/^# /Q" "$CHANGELOG"
