# Supported rules

| ID                         | Description                                  | Config Options                                         |
| -------------------------- | -------------------------------------------- | ------------------------------------------------------ |
| `H1-fixup`                 | You merged a fixup! commit                   |                                                        |
| `H2-shortest-subject-line` | Shortest subject line                        | `rules.h2_shortest_subject_line.length_threshold = 10` |
| `H3-longest-subject-line`  | Longest subject line                         | `rules.h3_longest_subject_line.length_threshold = 72`  |
| `H4-non-unicode`           | Commit message contains a non-utf-8 byte     |                                                        |
| `H5-empty-commit`          | Create an empty commit containing no changes |                                                        |
| `H6-whitespace-only`       | Commit whitespace-only changes               |                                                        |

## Notable example rules

* `H1-fixup`
  * Example of a rule that looks only at the commit message
* `H2-shortest-subject-line`
  * Example of a rule with a configuration option
  * Example of a rule that stores state between commits
* `H5-empty-commit` and `H6-whitespace-only`
  * Example of rules that look at the diff of a commit and its parent
