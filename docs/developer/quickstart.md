# Developer quickstart

## How to build, test, and run

```sh
cargo build
cargo test
# Run 'herostratus check' on the current directory; a good smoke test
cargo run -- check .
```

See [docs/developer](docs/developer) for more developer documentation.

## Format

This project uses non-default `rustfmt` settings, because I dislike what the defaults do with module
imports:

```sh
cargo fmt -- --config group_imports=StdExternalCrate,imports_granularity=Module
```

## Clippy

This project uses `clippy` with all the default lints.

```sh
cargo clippy --all-targets
```

## More on tests

### nextest

I prefer to use [cargo-nextest](https://nexte.st/) for running the tests. It's not strictly
necessary, but does give a better experience than `cargo test`.

```sh
cargo nextest run
```

### Integration tests

There are both unit tests and integration tests. The integration tests execute the `herostratus`
binary, and make assertions on its output. See [herostratus/tests](herostratus/tests).

### Ignored tests

There are a few tests that are gated behind the `#[cfg(feature = "ci")]` feature flag. These are
tests that

* might take too long to run for a nominal developer experience
* require SSH configuration (not available in CI)

### Mutation testing

Using [cargo-mutants](https://mutants.rs/) for mutation testing is phenomenally easy, and even
though Herostratus has pretty good test coverage, `cargo-mutants` did highlight a few bugs, and
several gaps in tests.

```sh
cargo mutants --in-place --package herostratus
```

While not every issue it points out is worth fixing, it is sometimes a useful tool.

### Test Branches

There are [orphan test branches](https://github.com/Notgnoshi/herostratus/branches/all?query=test)
in this repository used for integration tests.

For example, the `test/simple` branch was created like this:

```sh
git checkout --orphan test/simple
git rm -rf .
for i in `seq 0 4`; do
    git commit --allow-empty -m "test/simple: $i"
done
```
