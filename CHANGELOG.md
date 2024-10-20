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

# Herostratus - 0.1.0 - (2024-10-20)

This is Herostratus's first release! It represents the _bare minimum_, and only generates _two_
different achievement types.

| ID                       | Description                                    |
|--------------------------|------------------------------------------------|
| H1-fixup                 | Find fixup! and similar commits                |
| H2-shortest-subject-line | Find the commit with the shortest subject line |

## Running on an existing checkout

Herostratus provides a test mode to statelessly run on an existing checkout.

```sh
git clone git@github.com:Notgnoshi/herostratus.git
cd herostratus
cargo run -- check $PWD origin/test/fixup
```

this results in

```sh
Achievement { name: "I meant to fix that up later, I swear!", commit: 2721748d8fa0b0cc3302b41733d37e30161eabfd }
Achievement { name: "I meant to fix that up later, I swear!", commit: a987013884fc7dafbe9eb080d7cbc8625408a85f }
Achievement { name: "I meant to fix that up later, I swear!", commit: 60b480b554dbd5266eec0f2378f72df5170a6702 }
```

> [!WARNING]
> This output format will change as Herostratus becomes more usable

[`test/fixup`](https://github.com/Notgnoshi/herostratus/tree/test/fixup) is a branch used for
integration testing that looks like

```sh
$ git log --oneline origin/test/fixup
83f0448 (origin/test/fixup) Placeholder
2721748 amend! Initial commit
be52110 Placeholder
a987013 fixup! Initial commit
3d6d9bd Placeholder
60b480b squash! Initial commit
107bdb3 Initial commit
```

## Adding a new repository

Assuming users want to track achievements over time, you can also configure Herostratus to run on a
predefined set of repositories.

```sh
herostratus add git@github.com:Notgnoshi/herostratus.git test/simple
herostratus add git@github.com:Notgnoshi/herostratus.git test/fixup
```

> [!TIP]
> There are several configuration options for SSH and HTTPS authentication, as well as customizing
> where the repositories are cloned. See `herostratus --help` and `herostratus add --help` for
> details.

Herostratus supports the following clone URL protocols
* SSH in both URL formats
  * Example: `ssh://git@github.com/Notgnoshi/herostratus.git`
  * Example: `git@github.com:Notgnoshi/herostratus.git`
* HTTPS
* FILE
  * Example: `file://$HOME/src/herostratus`
  * **NOTE:** this isn't how you re-use an existing checkout, use `herostratus add --path` for that

## Fetch all configured repositories

```sh
herostratus fetch-all
```

## Running on all configured repositories

```sh
herostratus check-all
```

# Herostratus - 0.1.0-rc3 - (2024-08-19)

* The `add` and `fetch-all` subcommands now support cloning both SSH and HTTPS URLs.

  * Host SSH agent (the default)
  * SSH private and public keys, including those protected by a passphrase
  * HTTPS username + password

  Assuming the user has their SSH agent configured, and can "just" `git clone` the same SSH URL, so
  can Herostratus!

  ```sh
  herostratus add git@github.com:Notgnoshi/herostratus.git test/fixup
  ```
* The `add` subcommand now handles sharing the same clone directory between two repositories with
  the same remote.

  ```sh
  herostratus add git@github.com:Notgnoshi/herostratus.git test/simple --name hero-1
  herostratus add git@github.com:Notgnoshi/herostratus.git test/fixup --name hero-2
  ```

  Each (URL, Branch) pair must have a unique name. If a unique name is not specified, the second
  invocation will overwrite the first.
* After a repository has been `add`ed, Herostratus can fetch from the default `origin` remote with

  ```sh
  herostratus fetch-all
  ```

  It will fetch whatever branches have been `add`ed, as opposed to fetching everything.

  If the repository hasn't been cloned yet (you passed `--skip-clone` to `add`), it will be cloned
  at this time.
* The `check-all` subcommand now fetches by default and has a `--no-fetch` flag to disable. This is
  the expected way to fetch-and-check added repositories.

  ```sh
  # One time setup
  herostratus add <URL> [BRANCH]
  herostratus add <URL> [BRANCH]
  ...

  # Repeatedly by the user (on a timer, or when they're bored)
  herostratus check-all
  ```

# Herostratus - 0.1.0-rc2 - (2024-07-14)

## Changed
The Herostratus CLI interface has been changed to use subcommands:

* `herostratus check <path> [reference]` - statelessly check the repository at the given path
* `herostratus add <url> [branch]` - clone the given repository for later processing
* `herostratus remove` - remove the given repository
* `herostratus fetch-all` - fetch each cloned repository
* `herostratus check-all` - check each cloned repository

This is a fairly major milestone in the project roadmap, and enables both quickly processing any
given local checkout, as well as the ability to remember state about the checkouts that have already
been cloned, which is intended to support periodic runs of herostratus as a background service at
some point in the future.

Not all subcommands are implemented, and the ones that are implemented need to be fleshed out more:

* `check` -- finished
* `add` -- needs better error / edge case handling, and support of SSH/HTTPS clone URLs
* `check-all` -- needs better error / edge case handling

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
