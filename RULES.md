# Supported rules

## Achievement kinds

Each achievement has a **kind** that controls how it is granted and whether it can be revoked.

| Kind              | Description                                                                                                   |
| ----------------- | ------------------------------------------------------------------------------------------------------------- |
| Per-user          | Each user can earn this achievement independently. Granted at most once per user.                             |
| Per-user, repeat  | Each user can earn this achievement multiple times, at rule-defined milestones.                               |
| Global            | Only one user holds this achievement at a time. Once granted, it is permanent.                                |
| Global, revocable | Only one user holds this achievement at a time. A new leader supersedes the previous holder, revoking theirs. |

## Rules

| ID                         | Kind              | Description                                  | Config Options                                         |
| -------------------------- | ----------------- | -------------------------------------------- | ------------------------------------------------------ |
| `H1-fixup`                 | Per-user          | You merged a fixup! commit                   |                                                        |
| `H2-shortest-subject-line` | Global, revocable | Shortest subject line                        | `rules.h2_shortest_subject_line.length_threshold = 10` |
| `H3-longest-subject-line`  | Global, revocable | Longest subject line                         | `rules.h3_longest_subject_line.length_threshold = 72`  |
| `H4-non-unicode`           | Per-user          | Commit message contains a non-utf-8 byte     |                                                        |
| `H5-empty-commit`          | Per-user          | Create an empty commit containing no changes |                                                        |
| `H6-whitespace-only`       | Per-user          | Commit whitespace-only changes               |                                                        |
| `H7-first-profanity`       | Global            | Be the first person to swear in the repo     |                                                        |
| `H8-potty-mouth`           | Per-user          | Use profanity in a commit message            |                                                        |
| `H9-like-a-sailor`         | Per-user, repeat  | Use profanity in many commit messages        |                                                        |
| `H10-most-profound`        | Global, revocable | The author with the most profanity           |                                                        |
| `H11-achievement-farmer`   | Global, revocable | Farm the most achievements                   |                                                        |

## Notable example rules

* `H1-fixup`
  * Example of a rule that looks only at the commit message
* `H2-shortest-subject-line`
  * Example of a rule with a configuration option
  * Example of a rule that stores state between commits
* `H5-empty-commit` and `H6-whitespace-only`
  * Example of rules that look at the diff of a commit and its parent
* `H7-first-profanity`
  * Example of a Global rule (first person wins permanently)
* `H9-like-a-sailor`
  * Example of a Per-user, repeatable rule (grants at milestone thresholds)
* `H10-most-profound`
  * Example of a Global, revocable rule (new leader supersedes previous holder)
* `H11-achievement-farmer`
  * Example of a meta-achievement that's granted given the `AchievementLog` rather than
    `Observation`s
