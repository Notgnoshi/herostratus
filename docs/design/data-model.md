# Data model

# Status

**PROPOSAL**

# Scope

This document answers the following questions
* What data does an achievement contain or reference?
* What inputs are required for a Rule engine?

# Achievements

## Achievement uniqueness

1. Repeatable. E.g., swear in a commit message.
2. Unique. E.g., longest/shortest commit message.

## Achievement contents

* Achievement ID

  This can be used to look up the title, description, art, etc., or this data can be embedded in the
  achievement.
* User ID

  This needs to be .mailmap aware, and may need to have committer/author distinction?
* What repository the achievement is associated with
* What commit(s) the achievement is associated with

  There will always be a "primary" commit, but in the case of e.g. revert commits, there might be
  additional "context" commits.
* Achievement uniqueness

  A consumer of the rule engine will consume this, and determine whether it needs to revoke the
  achievement from another user to grant it to another one.

# Rules

## Mailmap

Rule generation does need to be mailmap aware, because there might be some rules like "be most
prolific contributor" that would change based on mailmaps.

## Rule initialization

Some rules might require (or be more efficient) if there's an initialization phase

```rust
fn init(&mut self, repository: &git2::Repository, config: &RulesConfig) {}
```

## Caching concerns

Some rules might not work well if previous runs are cached. For example, stateful rules like
"longest commit message" may either require rejecting the cache acceleration, or may require adding
rule-specific data to the cache.

## Rule variants

1. Context-free. E.g., swear in a commit message
    1. Commit message
    2. Commit message + diff
    3. Commit message + diff + submodule
2. Contextual
    1. User aware. E.g., be the most prolific contributor
    2. Commit history aware. E.g., revert a previous commit within 30min, or revert the same commit
       multiple times

## Rule configuration

There might be global configuration shared between rules like
* Exclude commits from these users
* Exclude commit messages matching these hashes or regexes

But there will also be rule-specific configuration like
* When calculating the shortest subject line, exclude any subject lines longer than 10 characters

## Rule inputs

* The repository
    * Any submodules of the repository
* The reference being processed
* The `&Config`
* The commit itself
* User mailmap
