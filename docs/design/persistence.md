# Data storage

# Data directory

**IMPLEMENTED**

Herostratus will persist data in a platform-appropriate data directory using the
[directories](https://crates.io/crates/directories) crate.

On Linux (the only target platform), this defaults to `~/.local/share/herostratus/`, and can be
overridden with the `--data-dir` CLI option.

The data directory contains:

* `config.toml` - configures what repositories and branches to process, along with any custom rule
  settings.
* `git/` - contains the bare repositories cloned by Herostratus

Note that `herostratus add` requires every repository/branch pair be given a unique name. This name
will default to the last path component of the repository URL, but can be overridden from the CLI.
This name can be used as a directory name to store all persistent data in for that repository/branch
entry.

```
~/.local/share/herostratus/
    config.toml
    git/
        torvalds/linux/
        git/git/
        Notgnoshi/herostratus/
    cache/
        linux/
        git/
        herostratus-main/
        herostratus-example/
```

Note that the bare repository checkouts don't use the unique name from the `config.toml` file! This
enables using a single bare repository for multiple branches. But the cache _is_ specific for each
repository / branch pair, so it uses the unique name.

# Use cases

## Use Herostratus in a CI/CD context

Herostratus is intended for use in a repository CI/CD pipeline, for which persistent data needs to
be stored between runs. The easiest way to achieve this would be to store the data in the repository
itself (I'm imagining multiple different repositories triggering a downstream Herostratus pipeline,
rather than Herostratus running directly in each repository's pipeline).

So I think one design goal should be to use plaintext persistent storage so that it doesn't inflate
the Git repository size. Additionally, it should be append-only if possible.

## Persist repository checkouts

**IMPLEMENTED**. Stored in `~/.local/share/herostratus/git/<url/converted/to/path>/`

More attention needs to be paid to this in the intended CI/CD use case. I don't think I want to use
submodules (although that could actually be an easier way to implement checkpoints?) so the
checkouts won't persist. But in the intended use case of using downstream triggered pipelines to run
Herostratus, only one repository will be updated at a time, so Herostratus needs to persist data
about all known repositories, but only check a single specified repository each time it's run.

This improvement is tracked in <https://github.com/Notgnoshi/herostratus/issues/102>

## Remember which repositories/branches to process

**IMPLEMENTED**. Stored in `~/.local/share/herostratus/config.toml`

**Why?**

* Enables running Herostratus as a scheduled job
* Simplifies the CLI invocation(s)

User preference would probably be a TOML config file rather than stuffing it in a database.

Things that need to be stored:

* Path to checkout (either a bare repo that Herostratus cloned, or some other path)
* Reference to process
* Remote URL to fetch
* HTTPS / SSH authentication information
* User-contrib rules
* Rule filtering
* Commit filtering
* Mailmap settings

## Remember which commits/rules have been processed for each repository/branch

**PROPOSAL**

**Why?** Performance improvement. It can take quite long to process large-ish repositories like
Linux and Git.

Strategies:

1. For each commit, store which rules have been run on them
2. Maintain a mapping of `Set<RuleId>` -> `Set<CommitHash>`, where after processing, the mapping
   contains only one `Set<RuleId>` of every possible rule, and it maps to all processed commits.
3. Stamp the `HEAD` commit with a "checkpoint" that indicates which rules have been processed on all
   commits reachable from `HEAD`

From an edge-case and "purity" perspective, option #3 is the worst. But from a simplicity and
common-case perspective, it's the best (least data storage, simplest, easiest to understand, easiest
to implement).

**Decision:** Implement option #3 in `~/.local/share/herostratus/cache/checkpoint.json`

```json
{
    "<name>": {
        "last_processed_commit": "<hash>",
        "last_processed_rules": [1,2,3...]
    }
}
```

## Per-rule cached data

**PROPOSAL**

Some rules (like "longest commit message") require either rejecting the cache, or caching
rule-specific data. It would be inappropriate (and impossible to do correctly) to share cached data
between rules, as that introduces data dependencies between rules that inhibit future parallelism.

**Decision:**

* Each `Rule` gets a `Rule::rule_name() -> &'static str` method that returns the `Rule` name (use
  the struct name).
* For `Rule`s that need a cache, define a `Serialize + Deserialize + Default` struct and store/load
  it from `~/.local/share/herostratus/cache/<name>/<rule_name>.json`

**Caveat:** This assumes that the `Rule`s cached data type will not change between versions.
Considerable care needs to be taken to avoid renaming/removing fields, or adding non-defaultable
fields.

**Question:** What do we do if the cache can't be deserialized? We should either give up, or
reprocess that rule from scratch. So I think the processing algorithm will need to get quite a bit
smarter to handle these cases (and some serious tests around it).

## Per-user cached data

**DRAFT**

Some rules (like "most prolific bug writer") require caching data per-user. This will need to be
[mailmap](https://git-scm.com/docs/gitmailmap) aware, and will need more thought.

Some rules may even want to only enable themselves if there are enough contributors, (avoid most
prolific author if there's only one or two authors).

The same ownership / sharing rules for the per-rule cache still apply here; two rules cannot share
the same user data cache, because that introduces data dependencies. So I think perhaps the user
cache also gets stored in `~/.local/share/herostratus/cache/<name>/<rule_name>.json`, likely as a
`HashMap<UserId, UserCacheRecord>`, where the `UserId` is some mailmap-aware unique identifier for
the user, probably stored in `~/.local/share/herostratus/cache/<name>/users.json`.

## Cache API for `Rule` implementations

```rust
trait Rule {
    // Default to () if no cache is needed
    type RuleCache: Default + Serialize + Deserialize = ();

    // Rule expected to store the cache and return it with updated values upon finalization
    fn init_cache(&mut self, cache: Self::RuleCache) {}
    fn fini_cache(&self) -> Self::RuleCache {}
}
```

## Remember Granted Achievements

**DRAFT**

**Why?**

* Enable revoking unique achievements if they need to be granted to someone else (like "largest
  commit/bug")
* Enable easier access to Herostratus data by the user (enable them to build whatever they want on
  top).
* Enable easier integration implementations

This should be an append-only log of events (including grant/revoke) to facilitate storage in a Git
repository.

## Mapping from each possible Herostratus achievement to their corresponding GitLab achievement IDs

**DRAFT**

**Why?** When you create a GitLab achievement, it returns an ID for each created achievement. So you
need to store them, so that you can grant them to users. And Herostratus (at least the GitLab
integration part) will need to store them, so that it can map between Herostratus achievements and
GitLab achievements.

**Proposal:** Store them in `~/.local/share/herostratus/cache/gitlab.json`:

```json
{
    <achievement id>: {}
}
```
