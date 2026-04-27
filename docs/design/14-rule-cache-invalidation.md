# Rule cache invalidation

# Status

**PROPOSAL**

# Scope

When a rule's behavior, cache schema, or achievement metadata changes incompatibly between releases,
the persisted state in `cache/` and `export/events/` becomes stale. Past breaking changes (H10 and
H13 in 1.0.0) were handled by manually wiping the data directory. This document proposes a
developer-driven mechanism that selectively invalidates caches and events for rules whose version
has been bumped.

# Background

See [06-persistence.md](06-persistence.md) for the existing persistence model. Relevant facts:

* The checkpoint at `cache/<repo>/checkpoint.json` records the last processed commit and the rule
  IDs processed up to that commit. The existing "retire and continue" logic already backfills
  newly-added rules.
* Per-rule caches live at `cache/<repo>/<rule_name>.json`.
* The events log at `export/events/<repo>.csv` is ideally append-only, intended to be
  Git-committable.
* Meta-achievements (H11) have no cache and are recomputed each run.

# Proposal

## Per-rule version

Each rule declares `const VERSION: u32 = 1;` on the `Rule` trait, plumbed through the object-safe
`RulePlugin` via the existing blanket impl. Bump the version when the cache shape, commit-level
criteria, or returned `Meta` changes. Do not bump for behavior-preserving refactors.

## Checkpoint format

`rules` changes from `Vec<usize>` to `Vec<(usize, u32)>`:

```json
{ "commit": "abc123...", "rules": [[1, 1], [2, 1], [3, 2]] }
```

A custom `Deserialize` accepts either the old (`[1, 2, 3]`) or new shape. Old-format checkpoints are
decoded as if every rule were at version 1, so upgrading from 1.0.0 triggers no invalidation.
Checkpoints are always written in the new shape.

## Classification and invalidation

At pipeline start, each enabled rule is classified as:

* **Unchanged** - in checkpoint at same version; retire at checkpoint commit.
* **New** - not in checkpoint; full-history backfill.
* **Invalidated** - in checkpoint at a different version; full-history backfill, plus the pre-walk
  steps below.

From the walker's perspective, `New` and `Invalidated` are identical.

Pre-walk steps (only if at least one rule is `Invalidated`):

1. Delete each invalidated rule's `cache/<repo>/<rule_name>.json`.
2. Rewrite `export/events/<repo>.csv` in place, dropping rows whose `achievement_id` belongs to any
   invalidated rule or any meta-achievement. Rule IDs expand to `achievement_id`s via compiled-in
   `meta`.
3. Load the `AchievementLog` from the pruned CSV and continue normally.

Meta-achievement rows are pruned on any invalidation because meta results derive from the log and
cannot be trusted once upstream grants have changed.

## User-visible behavior

No CLI changes. Invalidation is logged at WARN, e.g.:

```
WARN Rule H10-most-profound version changed (1 -> 2); wiping cache and re-processing full history
```

File deletions are logged at DEBUG level.

# Testing

* **Unit:** `Checkpoint` deserialize accepts both shapes and always writes the tupled shape.
* **Integration:** forward migration (1.0.0-style checkpoint, no invalidation triggered); and
  end-to-end invalidation (seeded checkpoint `[(1, 0), (2, 1)]` against a binary where rule 1 is at
  version 1, asserting rule-1 grants regenerate, rule-2 grants persist, meta grants regenerate, and
  rule 1's cache file is replaced).

Integration tests simulate a version bump by writing version `0` into the checkpoint rather than
introducing a test-only rule. Version 0 is a safe sentinel because production rules start at 1.
