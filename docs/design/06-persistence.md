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

**IMPLEMENTED**

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

**IMPLEMENTED**

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

## User database

**PROPOSAL**: Store a mailmap-aware user database in
`~/.local/share/herostratus/cache/<repositories>/users.csv`

The user database will be a CSV file with the following columns:

* email (mailmap resolved)
* name (mailmap resolved)

There will need to be a `MailMapResolver` that can combine mailmaps from the repo, mailmaps from the
Herostratus config, and _possibly_ heuristic mailmaps to map users.noreply.github.com to real
emails. The user database should be generated by the `RuleEngine` (that is, it should _not_ be
generated by the `Rule`s themselves) as new commits are processed. It should be passed into
`Rule::init_cache()`, `Rule::process()`, and `Rule::fini_cache()` so that rules can access it after
they know all commits have been processed.

## Per-user cached data

**REJECTED**

I don't think we need a dedicated per-user cache, we can "just" have a `HashMap<Email, UserData>`
field in the `RuleCache`s that need it.

## Cache API for `Rule` implementations

**IMPLEMENTED**

```rust
trait Rule {
    // Default to () if no cache is needed
    type RuleCache: Default + Serialize + Deserialize = ();

    // Rule expected to store the cache and return it with updated values upon finalization
    fn init_cache(&mut self, cache: Self::RuleCache) {}
    fn fini_cache(&self) -> Self::RuleCache {}
}
```

There are a number of issues with this design:

* Associated types with defaults aren't stable yet.
* Associated types aren't object safe, so we can't use `Box<dyn Rule>`.
* `inventory::submit!` / `inventory::collect!` don't work with _either_ generics / associated types
  since it generates `impl ::inventory::Collect for RuleFactory { ... }` which is missing any hooks
  for `impl<T> for Foo<T> {}` generics

Only the implementor of the `Rule` trait cares about the definition of the cache type, so perhaps we
can type-erase it such that the caller only sees that it's `Serialize + Deserialize + Default`?

It might be possible to define two traits `RulePlugin` and `Rule`, where `RulePlugin` is the
object-safe trait that `inventory` collects (and `process_rules` operates on), and `Rule` is the
"simpler" trait that users actually implement, with an associated or generic type for the cache.

```rust
trait RulePlugin {
    // serde_json::Value is used for type-erasure; it doesn't actually require the cache is JSON
    fn init_cache_erased(&mut self, cache: serde_json::Value) -> eyre::Result<()>;
    fn fini_cache_erased(&self) -> eyre::Result<serde_json::Value>;
}

trait Rule<Cache = ()> where Cache: Default + serde::Serialize + for<'de> serde::Deserialize<'de> {
    fn init_cache(&mut self, cache: Cache) -> eyre::Result<()>;
    fn fini_cache(&self) -> eyre::Result<Cache>;
}
```

with a blanket implementation to tie them together

```rust
impl<R, C> RulePlugin for R where
    R: Rule<C>,
    C: Default + serde::Serialize + for<'de> serde::Deserialize<'de>,
{
    fn init_cache_erased(&mut self, cache: serde_json::Value) -> eyre::Result<()> {
        let typed_cache: C = serde_json::from_value(cache)?;
        self.init_cache(typed_cache)
    }

    fn fini_cache_erased(&self) -> eyre::Result<serde_json::Value> {
        let concrete_cache = self.fini_cache()?;
        let erased_cache = serde_json::to_value(typed_cache)?;
        Ok(erased_cache)
    }
}
```

## Remember Granted Achievements

**IMPLEMENTED**: Store an append-only CSV log of achievement events in
`~/.local/share/herostratus/cache/<repository>/achievements.csv`

**Why?**

* Enable revoking unique achievements if they need to be granted to someone else (like "largest
  commit/bug")
* Enable easier access to Herostratus data by the user (enable them to build whatever they want on
  top).
* Enable easier integration implementations

This should be an append-only log of events (including grant/revoke) to facilitate storage in a Git
repository. This should be a CSV file with the following columns:

* timestamp
* event type (grant / revoke)
* achievement ID
* commit hash
* author email (mailmap aware)

## Mapping from each possible Herostratus achievement to their corresponding GitLab achievement IDs

**REJECTED**

I've decided not to use the GitLab achievements API, as I want Herostratus to maintain its sense of
whimsy and fun, and the appearance of being a "real" or "official" achievement system undermines
this.

In place of this achievement system, I'm planning on generating a static site that can be hosted on
GitHub / GitLab Pages to display all of the achievements for a set of configured repositories.
