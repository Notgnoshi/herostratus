# herostratus-quine

Brute-force search for a "quine" Git commit: a commit whose message contains a prefix of its own
SHA-1 hash. The commit is an orphan with no parent and an empty tree.

## Usage

```sh
cargo run --release --bin quine -- -n 7
```

The `-n` flag controls how many hex characters of the hash to match (default: 8). Author name,
email, and timestamp are auto-detected from your git config, but can be overridden:

```sh
cargo run --release --bin quine -- -n 8 --name "Name" --email "name@example.com" --timestamp 1000000
```

You can use `--parent` to set the commit's parent hash, which lets you add a quine commit to an
existing branch.

Use `-j` to control the number of worker threads (default: all available cores).

## Fortune teller mode

If you pass `--target-prefix` and `--parent`, the tool will attempt to generate a commit whose hash
starts with the given prefix. This supports generating commits that trigger the `H13-fortune-teller`
achievement.

## Importing the commit

The tool writes the raw commit content to stdout. To import it into a git repository:

```sh
cargo run --release --bin quine -- -n 7 > commit.raw
HASH=$(git hash-object -t commit -w --stdin < commit.raw)
git update-ref refs/heads/quine "$HASH"
```

## Examples

The <https://github.com/Notgnoshi/herostratus/commit/588b41b6e983c393df17689d7659145fbce16fa9>
commit on the [test/quine](https://github.com/Notgnoshi/herostratus/commits/test/quine/) orphan
branch is an example of a `n=10` quine commit.

The <https://github.com/Notgnoshi/herostratus/commit/0e0d4d0cd7a605721330be5d082dbf0eb62e909d>
commit starts with the `n=7` prefix from the previous quine commit
<https://github.com/Notgnoshi/herostratus/commit/0e0d4d0a3c8ae4d09761790162414bfc22010d7f> on the
[test/quine](https://github.com/Notgnoshi/herostratus/commits/test/quine/) orphan branch.

## Related projects

* [every-git-commit-shorthash](https://github.com/not-an-aardvark/every-git-commit-shorthash) -- A
  repository containing a commit for every possible 4-character hex prefix
* [lucky-commit](https://github.com/not-an-aardvark/lucky-commit) -- Customize your git commit
  hashes
* [git-quine](https://github.com/stfnw/git-quine) -- A git repository that is a quine of itself
* [quine-commit](https://github.com/broothie/quine-commit/commit/df2128c1b3fed98d646d86911adba677a97165ad)
  -- A commit containing its own hash
* [predict-commit](https://gitlab.com/pritambaral/predict-commit) -- Predict and embed a git commit
  hash
