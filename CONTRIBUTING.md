# Contributing to Herostratus

## Project goals

This is a silly project to gamify things that shouldn't be gamified.

* The achievements should be whimsical and silly, and absolutely not be used in a serious manner
* It should be fast
* Its preferred UI is the CLI
* It should be easyish to set up by a new user
* It should be easyish to add new rules
* Its target runtime environment is modern Linux x86_64
* It should provide optional integrations:
    * It should provide an integration with [GitLab achievements](https://docs.gitlab.com/ee/user/profile/achievements.html)
    * It could provide an integration with GitHub

## Changelog

This project keeps a [CHANGELOG](CHANGELOG.md). Not every PR needs to add an entry to the changelog,
but notable features should be added before a release is made.

**Please keep the changelog focused on the user impact, instead of the actual changes made.**

These changelog entries are automatically added to the generated releases:
<https://github.com/Notgnoshi/herostratus/releases>

## Releases

Releases are automated by the CI/CD pipeline, and are triggered by merging a change that bumps the
version number in the project [Cargo.toml](Cargo.toml).

The process for making a new release is:
1. Submit a PR making the release
    1. Bump the version number in `Cargo.toml`
    2. Add the new version number to the `CHANGELOG.md`, moving all of the entries under the `#
       Herostratus - Unreleased - (YYYY-MM-DD)` header down under the header for the new release.
    3. Perform any last-minute release notes polish
2. Merge it!

## Deployment

**TODO:** Find a suitable deployment strategy.

## Tests

This project values tests. Please consider adding tests with contributions.

## Build warnings

Compiler warnings / Clippy lints are treated as errors in the CI/CD pipeline. Lints may be
judiciously ignored on a case-by-case basis.

New lints added by the latest stable Rust toolchain should be resolved.

## MSRV

Herostratus is a CLI application, and thus there is no need to maintain compatibility with older
Rust toolchains, or older versions of dependencies.

The Minimum Supported Rust Version (MSRV) is the latest stable toolchain.

## Logging

Logs are great.

* INFO level logs should not be spammy
* Prefer logs that are useful for troubleshooting, both from a developer perspective and a user one
* Prefer logs that provide information. E.g., instead of `"Failed to process repository"`, indicate
  _what_ and _why_ with `"failed to process repository '{repo}' because: '{e:?}'"`

## Git

All PRs are to be rebased on top of `main` prior to merging. Multi-commit PRs will be merged with a
merge commit. Prefer small PRs that can be merged with minimal risk.

**This project values its Git history.**

Subjective judgment calls about what's valuable or "too messy" will be made by me (@Notgnoshi). Note
that I _am_ willing to work with PR authors who don't normally develop according to the my
preferences.

Unfortunately, the principles that make for the best commit messages aren't really possible to lint
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

**I don't desire a perfect history. Just a good one.**
