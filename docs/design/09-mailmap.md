# Mailmap support

# Status

**IMPLEMENTED**

# Scope

How Herostratus resolves commit author/committer identities using Git's
[mailmap](https://git-scm.com/docs/gitmailmap) mechanism, so that achievements are correctly
attributed even when a person commits under multiple names or email addresses.

See also: <https://github.com/Notgnoshi/herostratus/issues/17>

# Why

The `Achievement` struct does not yet include author identity. Before user-aware achievements (e.g.,
"most prolific swearer"), unique/stealable achievements, user database, or the achievement event log
from [06-persistence.md](/docs/design/06-persistence.md) can work, identity resolution must be in
place.

# Mailmap sources

Merged in order of increasing priority (later sources override earlier ones):

## 1. Repository mailmap + Git config

`gix::Repository::open_mailmap()` handles this automatically:

* `.mailmap` in the working tree, or `HEAD:.mailmap` for bare repositories
* `mailmap.blob` and `mailmap.file` from Git config

Uses `HEAD` (or the configured ref), matching Git's own behavior. The mailmap represents the
_current_ canonical mapping, not a historical one.

## 2. Herostratus configuration

Users can provide additional mailmap files in `config.toml`, useful when the repository lacks a
`.mailmap` or the user wants mappings across repositories they don't control.

```toml
# Applied to all repositories
mailmap_file = "/path/to/my/mailmap"

[repositories.linux]
path = "git/torvalds/linux"
url = "https://github.com/torvalds/linux.git"
# Merged after the global mailmap_file, so per-repository entries take precedence
mailmap_file = "/path/to/linux-specific/mailmap"
```

### GitHub noreply addresses

GitHub noreply addresses (`12345+user@users.noreply.github.com`) contain the GitHub username, not
anything that maps to the user's real email. Heuristic resolution (name matching, API lookups) is
fragile, and silently wrong attributions are worse than split identities. Use explicit mailmap
entries instead:

```
Proper Name <proper@email.xx> <12345+username@users.noreply.github.com>
```

# Implementation

## Loading

Use `gix::Repository::open_mailmap_into()` to populate a `gix_mailmap::Snapshot`, then call
`snapshot.merge()` with entries parsed from the global and per-repository Herostratus mailmap files.

## Resolving identities

```rust
let signature = commit.author()?;
let resolved = snapshot.resolve(signature);
// resolved.name and resolved.email are the canonical identity
```

The `RuleEngine` builds the snapshot once per repository and resolves identities as commits are
processed. This is consistent with the "user database" proposal in
[06-persistence.md](/docs/design/06-persistence.md) -- the `RuleEngine` resolves identities
centrally, and passes them to rules.

## Changes to `Achievement`

```rust
pub struct Achievement {
    pub descriptor_id: usize,
    pub name: &'static str,
    pub commit: gix::ObjectId,
    pub author_email: String,  // mailmap-resolved
    pub author_name: String,   // mailmap-resolved
}
```

## Cache invalidation

Changing a mailmap could change which canonical identity is associated with previously processed
commits. This means previously granted achievements may need to be re-attributed.

This might mean:

* We regenerate the user database
* We trigger reprocessing all commits?? (I'd prefer to avoid this, and have rules with cached
  userdata do a mailmap resolution on the fly when processing cached data?)
* We regenerate the achievement event log?? (I had wanted it to be append-only, but maybe we need to
  allow re-attribution?)

This is out of scope for the initial implementation.
