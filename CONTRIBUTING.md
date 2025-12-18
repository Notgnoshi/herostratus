# Contributing to Herostratus

## Developer quickstart

See [docs/developer/quickstart.md](/docs/developer/quickstart.md)!

## Project goals

This is a silly project to gamify things that shouldn't be gamified.

* The achievements should be whimsical and silly, and absolutely not be used in a serious manner
* It should be fast
* Its preferred UI is the CLI
* It should be easyish to set up by a new user
* It should be easyish to add new rules
* Its target runtime environment is modern Linux x86_64
* It should provide optional integrations:
  * It should provide an integration with
    [GitLab achievements](https://docs.gitlab.com/ee/user/profile/achievements.html)
  * It could provide an integration with GitHub

## Releases

Releases are automated by the CI/CD pipeline. They are triggered by bumping the version number in
the [Cargo.toml](/Cargo.toml).

The process is:

1. Submit a PR making the release
   1. Bump the version number in `Cargo.toml`
   2. Add a new entry to the `CHANGELOG.md`
   3. Add any last-minute polish to the `CHANGELOG.md`
2. Make sure the CI/CD pipeline passes
3. Merge it!

The changelog entries are automatically added to the generated releases:
<https://github.com/Notgnoshi/herostratus/releases>.

**Please keep the release notes focused on the user impact**

## Tests

This project values tests, as well as documentation

## MSRV

The Minimum Supported Rust Version (MSRV) is the latest stable toolchain.

## Logging

Logs are great. Spammy logs are less great.

* The default log level for users is INFO. INFO should not be spammy
* DEBUG and TRACE logs are for developers. DEBUG should not be spammy, but TRACE sure can be!
* Prefer logs that are useful for troubleshooting, both from a developer perspective and a user one

  E.g., prefer `"failed to process repository '{repo}' because: '{e:?}'"` over
  `"Failed to process repository"`

## Git

This project uses `gitlint` in the CI/CD pipeline. You can run `gitlint` yourself with the same
rules as the pipeline by setting the following two environment variables

```sh
# Run from within the Herostratus repository directory
export GITLINT_CONFIG="$PWD/.github/gitlint/gitlint.ini"
export GITLINT_EXTRA_PATH="$PWD/.github/gitlint"
gitlint --commits main..HEAD
```

You can invoke gitlint through the CLI, your editor, or as a commit hook.

## Documentation

This project values documentation:

* API documentation in the form of rustdoc comments
* Design documentation in [docs/design](/docs/design)
* Developer documentation at [docs/developer](/docs/developer)
* User documentation (TODO)
