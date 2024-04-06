# Contributing to Herostratus

## Project goals

This is a silly project to gamify things that shouldn't be gamified.

* The achievements should be whimsical and silly, and absolutely not be used in a serious manner,
  especially not to measure developer contribution.
* It should be easyish to set up by a new user
    * Its target audience is software engineers, so it need not be too easy
* Its target runtime environment is modern GNU Linux x86_64
    * It could also support other environments, but they won't be prioritized
* It should be easyish to add new rules
* It should be fast
* Its preferred UI is the CLI
    * It could provide a standalone web UI
* It should provide optional integrations:
    * It should provide an integration with GitLab achievements
    * It could provide an integration with GitHub

## Changelog

This project keeps a [CHANGELOG](CHANGELOG.md). Not every PR needs to add an entry to the changelog,
but every notable feature should be added.

**Please keep the changelog focused on the user impact, instead of the actual changes made.**

## Releases

Releases are automated by the CI/CD pipeline, and are triggered by merging a change that bumps the
version number in the project [Cargo.toml](Cargo.toml). Each release is required to have an entry in
the changelog.

**TODO:** Add automated release pipeline.

## Deployment

**TODO:** Find a suitable deployment strategy.

## Tests

This project values tests. Please consider adding tests with contributions. That said, I do not
believe in 100% test coverage (or even chasing after a certain % number). Please use the following
principles:

* Tests should be preferred in the following priority: Unit, Integration, Manual.
    * This repository does contain a few orphan test branches prefixed with `test/`. These branches
      can be used for integration tests, but please ensure that the repository size does not
      explode.
* Okay tests are better than no tests. Bad tests are worse than no tests.
* Unit test the things that can provide value when tested
* Don't make code more complex just to make it testable

## Build warnings

Compiler warnings are treated as errors in the CI/CD pipeline. Similarly, the CI/CD pipeline runs
Clippy, and treats all lints as errors. Lints may be judiciously ignored on a case-by-case basis.

However, warnings are a normal part of the development workflow, and are thus left enabled for local
developers.

Outdated dependencies should be updated.

New lints added by the latest stable Rust toolchain should be resolved (but in atomic commits!).

## MSRV

Herostratus is a CLI application, and thus there is no need to maintain compatibility with older
Rust toolchains, or older versions of dependencies.

The Minimum Supported Rust Version (MSRV) is the latest stable toolchain.

## Logging

Logs are great.

* INFO level logs should not be spammy
* Prefer logs that provide information. E.g., instead of `"Failed to process repository"`, indicate
  _what_ and _why_ with `"failed to process repository '{repo}' because: '{e:?}'"`

## Git

This repository uses a "sawtooth" merge strategy. All PRs are to be rebased on top of `main` prior
to merging. Single commit PRs may be merged with a fast-forward merge. Multi-commit PRs shall be
merged with a merge commit.

**This project values its Git history.**

"Squash and merge" PRs will not be accepted, and neither will PRs using "Conventional Commits" to an
excessive degree. I may choose to accept such PRs, but rewrite the commits myself before merging. I
will do my best to maintain original commit authorship.

Subjective judgment calls will be made by me (@Notgnoshi). Heated arguments about what is "right"
will be ignored or shut down, but the I _am_ willing to work with PR authors who don't normally
develop according to the my preferences.

**The matter of Git history is a subjective one, with no universally correct practices.**

However, see <https://cbea.ms/git-commit/> and
<https://tbaggery.com/2008/04/19/a-note-about-git-commit-messages.html> for the set of guiding
principles that this project uses for its Git history.

Unfortunately, the principles that make for the best commit messages aren't possible to lint
automatically. So instead, this project uses `gitlint` in the CI/CD pipeline to lint the less
important, but _possible_ to lint mechanics.

You can run `gitlint` yourself with the same rules as the pipeline by setting the following two
environment variables
```sh
# Run from within the Herostratus repository directory
export GITLINT_CONFIG="$PWD/.github/gitlint/gitlint.ini"
export GITLINT_EXTRA_PATH="$PWD/.github/gitlint"
```
You can invoke gitlint through the CLI, your editor, or as a commit hook.

In my experience, enforcing these rules is enough to communicate that a project cares about its
commit history, which results in an overall better history. And I've found that projects that
maintain their history are _far_ easier to do code archaeology on (which is a personal value of
mine).

**I don't desire a perfect history. Just a good one.**
