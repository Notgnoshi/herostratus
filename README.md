# Herostratus
![lint workflow](https://github.com/Notgnoshi/herostratus/actions/workflows/lint.yml/badge.svg?event=push)
![release workflow](https://github.com/Notgnoshi/herostratus/actions/workflows/release.yml/badge.svg?event=push)

**Herostratus** *n.* **1.** An [ancient Greek](https://en.wikipedia.org/wiki/Herostratus) known for
seeking fame through crime and destruction. **2.** A Git repository achievements engine.

## Usage

### Trying it out

```sh
git clone git@github.com:Notgnoshi/herostratus.git
cd herostratus
cargo run -- check .
cargo run -- check . origin/test/fixup
```

The `check` subcommand is stateless. It reads/writes no configuration, and can not fetch from the
upstream remote. Read on for stateful configuration that enables running and re-running Herostratus
on a group of repositories:

### Setting it up

The following example configures Herostratus to run on its own
[test/simple](https://github.com/Notgnoshi/herostratus/tree/test/simple) and
[test/fixup](https://github.com/Notgnoshi/herostratus/tree/test/fixup) branches.

```sh
$ herostratus add git@github.com:Notgnoshi/herostratus.git test/simple
$ herostratus add git@github.com:Notgnoshi/herostratus.git test/fixup
```

> [!TIP]
> You may find the `--data-dir`, `--get-data-dir`, `--config-file`, and `--get-config` options
> useful when setting up Herostratus to run on one more more repository over time.

After this, you can use the `fetch-all` and `check-all` subcommands to update and check the cloned
repositories.
```sh
$ # Fetch from the remote tracking branch for test/simple and test/fixup
$ herostratus fetch-all
$ # Check the test/simple and test/fixup branches for achievements
$ herostratus check-all
Achievement { name: "I meant to fix that up later, I swear!", commit: 2721748d8fa0b0cc3302b41733d37e30161eabfd }
Achievement { name: "I meant to fix that up later, I swear!", commit: a987013884fc7dafbe9eb080d7cbc8625408a85f }
Achievement { name: "I meant to fix that up later, I swear!", commit: 60b480b554dbd5266eec0f2378f72df5170a6702 }
```

> [!WARNING]
> This output format will change as Herostratus becomes more usable

## Development

### Build and test
The usual `cargo build` and `cargo test`. There are a few integration tests that take too long to
run every time. These can be run with `cargo test -- --ignored`.

### Test Branches
There are orphan test branches in this repository used for integration tests.

For example, the `test/simple` branch was created like this:
```sh
git checkout --orphan test/simple
git rm -rf .
for i in `seq 0 4`; do
    git commit --allow-empty -m "test/simple: $i"
done
```

### Contribution
Contribution is welcome! See [CONTRIBUTING.md](CONTRIBUTING.md).
