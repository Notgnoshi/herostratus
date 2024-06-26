# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project
adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

These changelog entries will automatically be added to the project release notes. **Please keep the
focus on the user impact** rather than the actual changes made.

# Herostratus - Unreleased - (YYYY-MM-DD)
<!-- Please add new changelog entries here -->

## Added
## Changed
## Deprecated
## Removed
## Fixed
## Security

# Herostratus - 0.1.0-rc1 - (2024-04-07)
This is the first release of Herostratus! This release is largely centered around project
bootstrapping; Herostratus isn't quite ready to use yet.

## Added
* Cargo project bootstrapping. You can run Herostratus with

  ```sh
  cargo run -- $CLONE_URL_OR_PATH $REF_OR_REV
  ```
  * `$CLONE_URL_OR_PATH` can be any non-authenticated clone URL (https, or local file paths)
  * `$REF_OR_REV` is typically the name of a reference to parse. Herostratus will resolve the
    reference to a revision, and visit all reachable commits from the revision.

  The output is pretty bare bones. There's only a single achievement rule defined for prototyping
  purposes.

  For example, you can run Herostratus on itself, using the
  <https://github.com/Notgnoshi/herostratus/tree/test/fixup> test branch
  ```sh
  $ cargo run -- . origin/test/fixup
  Achievement { name: "I meant to fix that up later, I swear!", commit: 2721748d8fa0b0cc3302b41733d37e30161eabfd }
  Achievement { name: "I meant to fix that up later, I swear!", commit: a987013884fc7dafbe9eb080d7cbc8625408a85f }
  Achievement { name: "I meant to fix that up later, I swear!", commit: 60b480b554dbd5266eec0f2378f72df5170a6702 }
  ```

* Automated CI/CD pipelines to build, test, and release Herostratus.

  The process to create a release is:
  1. Add release notes to the `CHANGELOG.md`
  2. Bump the version in `Cargo.toml`

  and the pipeline will do the rest!
