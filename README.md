# Herostratus
![lint workflow](https://github.com/Notgnoshi/herostratus/actions/workflows/lint.yml/badge.svg?event=push)
![release workflow](https://github.com/Notgnoshi/herostratus/actions/workflows/release.yml/badge.svg?event=push)

**Herostratus** *n.* **1.** An ancient Greek known for winning fame through crime and destruction.
**2.** A Git repository achievements engine.

## Usage
**TODO:** This will change a lot

## Development

### Build and test
The usual `cargo build` and `cargo test`.

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
Contribution is welcome. See [CONTRIBUTING.md](CONTRIBUTING.md) for contribution standards.
