# Test data
**Status:** In consideration

The primary CLI tool should be able to process repositories in the following forms:
1. Existing on-disk worktrees
2. Existing on-disk bare repositories
3. HTTP, HTTPS, SSH remote clone URLs
    1. These should be cloned into something like `~/.cache/herostratus/git/`

Or maybe, it _just_ consumes remotes, and you pass the on-disk work trees and bare repositories _as_
local remotes that herostratus can fetch from? That might make for a more consistent, and easier to
test application?

Perhaps the primary CLI tool should _only_ look at on-disk repositories, and the cloning should be
handled by a wrapper?

It should not require the branch be checked out.

There should be orphan branches containing test commits in this repository
