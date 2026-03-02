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

Use `-j` to control the number of worker threads (default: all available cores).

## Importing the commit

The tool writes the raw commit content to stdout. To import it into a git repository:

```sh
cargo run --release --bin quine -- -n 7 > commit.raw
HASH=$(git hash-object -t commit -w --stdin < commit.raw)
git update-ref refs/heads/quine "$HASH"
```

## Example

The <https://github.com/Notgnoshi/herostratus/commit/588b41b6e983c393df17689d7659145fbce16fa9>
commit on the [test/quine](https://github.com/Notgnoshi/herostratus/tree/test/quine) orphan branch
is an example of a `n=10` quine commit.

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
