# Test data

# Status

**IMPLEMENTED**

# Repository sources

The primary CLI tool should be able to process repositories in the following forms:
1. Existing on-disk worktrees
2. Existing on-disk bare repositories
3. HTTP, HTTPS, SSH remote clone URLs
    1. These should be cloned into something like `~/.cache/herostratus/git/`

It should not require the branch be checked out.

# Test data

There should be orphan branches containing test commits in this repository. These will be prefixed
with `test/`. They can be made like

```sh
git checkout --orphan test/simple
git rm -rf .
for i in `seq 0 4`; do
    git commit --allow-empty -m "test/simple: $i"
done
```

See existing test branches here:
<https://github.com/Notgnoshi/herostratus/branches/all?query=test%2F>
