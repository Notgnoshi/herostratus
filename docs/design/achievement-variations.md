# Achievement variations

# Status

**DRAFT**

# Scope

This document identifies different _kinds_ of achievements that can be granted, and how they differ,
culminating in a rough design and a decision for how Herostratus will implement them.

# Variations

These variations are not mutually exclusive; any given achievement may fall into multiple
categories.

## 1. Simple achievments

This is the simplest kind of achievement. It is granted when a specific condition is met, entirely
stateless, user agnostic, and repeatable.

For example:

* "Swear in a commit message"
* "Make a commit containing invalid UTF-8"

## 2. Stateful achievements

These are achievements that maintain state between each commit that they process.

For example:

* "Longest commit message"
* "Most commits in a single day"
* "Make a commit and then revert it within X minutes"

NOTE: This implies a persistent cache for these style of achievements, because a future feature will
be to avoid reprocessing commits that have already been processed! See:
[persistence.md](/docs/design/persistence.md).

## 3. Unique achievements

Some achievements are unique, meaning that only one user can hold them at a time.

When a new user earns an achievement held by another user, we may have to revoke it from the
previous holder, or grant it again, or update the old achievement to indicate that it has been
"stolen". How exactly this works will likely depend a great deal on the integration layer (e.g.,
independent website vs. GitLab achivement integration).

For example:

* "Shortest commit message"
* "Most prolific swearer"

NOTE: This implies persistence of granted achievements. See
[persistence.md](/docs/design/persistence.md).

## 4. User awareness

Some achievements need to be aware of which user generated which commit (and be mailmap aware).

For example:

* "Most prolific swearer"
* "First commit by a user"
* "Overtake another user as the most prolific contributor"

NOTE: This implies mailmap awareness, which probably implies persistence of users and their email
addresses.

## 5. Recurrence

Some achievements might be able to be granted multiple times to the same user.

For example:

* "Swear in a commit message" (can be granted one time for each 5 swears)
* "Make N commits in a single day" (can be granted for each multiple of 5)

NOTE: This implies persistence of granted achievements, and possibly extra data attached to each
achievement (e.g., a "count" of how many times it has been granted, or how many commits match the
criteria).

Of all the variations, this is the one that I care the least about, so I may choose to skip it.

## 6. Diff vs Message

Git commits are snapshots of the repository tree at a given point in time. Thus, there is no "diff"
for a commit message. The diff shown by `git diff` or GitHub in PRs is determined by a particular
diff algorithm, and isn't guaranteed to be the same across different tools.

So any achievement rule that cares about the diff must compute it, and work with the diff
programmatically. This can be fairly expensive. See
[performance-considerations.md](/docs/design/performance-considerations.md) for an approach that
hopefully reduces the performance impact of diffing commits. Other achievements though, might care
just about the commit message, which is far cheaper to access.

A middle ground might be achievements that care about the number of additions/removals, or the file
paths that changed, but not the changes themselves.
