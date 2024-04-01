# Herostratus

**Herostratus** *n.* **1.** An ancient Greek known for winning fame through crime and destruction.
**2.** A Git repository achievements engine.

## How to run
**NOTE:** This will change a lot:
```sh
$ cargo run -- $PWD origin/test/fixup
Achievement { name: "I meant to fix that up later, I swear!", commit: 2721748d8fa0b0cc3302b41733d37e30161eabfd }
Achievement { name: "I meant to fix that up later, I swear!", commit: a987013884fc7dafbe9eb080d7cbc8625408a85f }
Achievement { name: "I meant to fix that up later, I swear!", commit: 60b480b554dbd5266eec0f2378f72df5170a6702 }
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
