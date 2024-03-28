# Herostratus

**Herostratus** *n.* **1.** An ancient Greek known for winning fame through crime and destruction.
**2.** A Git repository achievements engine.

## How to run
**NOTE:** This will change a lot:
```sh
$ cargo run -- --log-level DEBUG $PWD test/simple
2024-03-28T20:55:46.492658Z DEBUG herostratus::git: Searching "~/src/herostratus" for a Git repository
2024-03-28T20:55:46.525620Z  INFO herostratus::git: Found git repository at "~/src/herostratus/.git/"
2024-03-28T20:55:46.525913Z  INFO herostratus::git: Resolved "test/simple" to Commit b2829b8df987c380e75c512b9c38b455e51db874
2024-03-28T20:55:46.526141Z DEBUG herostratus: commit: b2829b8df987c380e75c512b9c38b455e51db874 summary: "test/simple: 4"
2024-03-28T20:55:46.526163Z DEBUG herostratus: commit: 6f7c968f1b22a61581dddd564641d7c0671cfadb summary: "test/simple: 3"
2024-03-28T20:55:46.526180Z DEBUG herostratus: commit: f90bae2518eb7acecb723e5cee461c6519db9144 summary: "test/simple: 2"
2024-03-28T20:55:46.526196Z DEBUG herostratus: commit: eecea7a03d3054abe509d6c0f8baff557c96a03f summary: "test/simple: 1"
2024-03-28T20:55:46.526212Z DEBUG herostratus: commit: 6802ff50641c9e31a5dea84e2e66846efc31790b summary: "test/simple: 0"
```

## How to test
Run the usual `cargo test`.

## Test Branches
There are orphan test branches in this repository used for integration tests.

For example, the `test/simple` branch was created like this:
```sh
git checkout --orphan test/simple
git rm -rf .
for i in `seq 0 4`; do
    git commit --allow-empty -m "test/simple: $i"
done
```
