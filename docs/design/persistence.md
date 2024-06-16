# Data storage
**Status:** In consideration

## Use cases

There are four things that need to be stored

### 1. Remember which repositories/branches to process

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

### 2. Remember which commits/rules have been processed for each repository/branch

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

### 3. Granted Achievements

**Why?**
* Avoid granting duplicates
* Enable easier access to Herostratus data by the user (enable them to build whatever they want on
  top).
* Enable easier integration implementations

### 4. Mapping from each possible Herostratus achievement to their corresponding GitLab achievement IDs

**Why?** When you create a GitLab achievement, it returns an ID for each created achievement. So you
need to store them, so that you can grant them to users. And Herostratus (at least the GitLab
integration part) will need to store them, so that it can map between Herostratus achievements and
GitLab achievements.

## Design

Use CLI subcommands to separate stateful from stateless operations in ways that's intuitive to the
user.

| Command                                  | Stateful? | Notes                                                                  |
|------------------------------------------|-----------|------------------------------------------------------------------------|
| `herostratus [check] <path> [reference]` | stateless | For testing. Process the repository at the given path.                 |
| `herostratus add <URL/PATH> [branch]`    | stateful  | Add the given repository to the config so that it can be checked later |
| `herostratus check-all`                  | stateful  | Fetch and check all configured repositories                            |
| `herostratus fetch-all`                  | stateful  | Fetch without checking all configured repositories                     |
| `herostratus remove <path> [reference]`  | stateful  | Remove the given repository / branch from the config                   |
