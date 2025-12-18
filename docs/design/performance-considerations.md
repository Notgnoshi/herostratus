# Performance considerations

# Status

**DRAFT**

# Scope

One of the project goals for Herostratus is to be performant. This document discusses various
methods of improving Herostratus's performance.

# Caching

The best way to improve performance is to do less work. Cache the last commit processed, and stop
processing commits when you reach it. See [persistence.md](/docs/design/persistence.md).

This gets tricky when you consider that adding more rules over time is likely to happen, so you'll
want the previously processed commits to be reprocessed when new rules are added.

I expect this to be the most impactful performance improvement, but it's also really tricky to
figure out, because of how intertwined persistence is with other features.

# Parallelism

See: [parallelism.md](/docs/design/parallelism.md). I expect that the parallelism strategy I pick
will be to run the achievement `Rule`s in parallel over a serial commit iterator.

# Benchmarks

Measure what matters. If performance is important, it should be quantified.

There are two forms of benchmarks in this repository:

* `cargo bench` - microbenchmarks of small repositories that is easy to run `callgrind` on for
  drilling into optimizations
* `./tools/benchmark.sh` - run `Herostratus` against a set of repositories and measure the wall
  clock time; updates the `## Benchmarks` section of the README.

# Computation sharing I

There are various `gix` operations that can consume a cache (like diffing trees). Rules could
conceivable share these caches.

However, I think a better approach would be to diff the commits outside the `Rule`s, and to define a
`Rule::is_interested_in_diff() -> bool` method that indicates whether the `Rule` is interested in
handling `Rule::on_change(&Change)` calls for each modification.

Then only one diff has to be calculated per commit instead of one per `Rule` per commit.

# Computation sharing II

The computation performed by some `Rule`s may be useful to other `Rule`s. For example, the rules
`H2-shortest-subject-line` and `H3-longest-subject-line` both need to calculate the length of the
subject line.

So I think one `Rule` should be able to generate multiple different types of achievements given one
computation pass. One complication for this though, is that each achievement can be individually
enabled/disabled.
