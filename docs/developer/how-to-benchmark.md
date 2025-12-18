# How to benchmark Herostratus

## Prerequisites

You need to have `valgrind`, `gungraun-runner`, and (optionally) `kcachegrind` installed.

```sh
# Ubuntu
sudo apt install valgrind kcachegrind
# Fedora
sudo dnf install valgrind kcachegrind

cargo install gungraun-runner
```

## How to run

There are benchmarks defined in [herostratus/benches](/herostratus/benches/main.rs). Run them with

```sh
cargo bench
```

## How to interpret the output

Look for console output like

```
Instructions:                   300058917|300189281            (-0.04343%) [-1.00043x]
L1 Hits:                        395178472|395182060            (-0.00091%) [-1.00001x]
LL Hits:                          1595251|1599447              (-0.26234%) [-1.00263x]
RAM Hits:                          133458|133793               (-0.25039%) [-1.00251x]
Total read+write:               396907181|396915300            (-0.00205%) [-1.00002x]
Estimated Cycles:               407825757|407862050            (-0.00890%) [-1.00009x]
```

that can be used to detect performance regressions or improvements.

## How to visualize with kcachegrind

The callgrind files are generated in
`target/gungraun/herostratus/herostratus_bench/check/check_self.v0_2_0/callgrind.check_self.v0_2_0.out`
and you can open them with kcachegrind.

## Helpful links

* <https://gungraun.github.io/gungraun/latest/html/benchmarks/binary_benchmarks.html>
* <https://kcachegrind.github.io/html/Documentation.html>
* <https://valgrind.org/docs/manual/cl-manual.html>
