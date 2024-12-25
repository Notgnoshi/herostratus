# User contributed rules

# Status

**PROPOSAL**

# Goal

Enable users to run their own rules

# Approaches

## Force users to fork the project

Don't provide a mechanism for users to add their own rules. Force them to fork the project, and
write their own rules.

## Make it easy for users to contribute their own rules

Make it easy enough to contribute new rules, that users feel they can do so. This may require
maintaining a set of default and non-default rules. It may also require toning down the
[contribution standards](../../CONTRIBUTING.md)

## Wrap scripts

Define a `stdin`/`stdout` JSON API, and let users write their own achievement generation tools.

## Plugins

### dylib

Challenging because Rust doesn't provide a stable ABI, even between invocations of the same compiler
version (due to type layout randomization).

### WASM

WASM seems like it's the plugin mechanism of choice in Rust land. I personally find it awkward,
because it's offloading the stable ABI concerns from the language and OS to the users. But it seems
easier than dylibs.

# Proposal

_If_ I get around to implementing user-contrib rules before I burn out, WASM plugins seem like the
way to go.
