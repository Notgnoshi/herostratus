use std::path::Path;

use gungraun::{BinaryBenchmarkConfig, Sandbox, binary_benchmark, binary_benchmark_group, main};

const WORKSPACE_ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/..");

// Sandbox puts each bench in a temp CWD, so we can avoid data-dir contamination between benches
#[binary_benchmark(config = BinaryBenchmarkConfig::default().sandbox(Sandbox::new(true)))]
#[bench::all_references("./data", "https://github.com/Notgnoshi/herostratus.git", None)]
#[bench::single_reference("./data", "https://github.com/Notgnoshi/herostratus.git", Some("main"))]
fn add_url<P: AsRef<Path>>(data_dir: P, url: &str, reference: Option<&str>) -> gungraun::Command {
    let mut cmd = gungraun::Command::new(env!("CARGO_BIN_EXE_herostratus"));
    cmd.arg("--data-dir")
        .arg(data_dir.as_ref())
        .arg("add")
        .arg(url);
    if let Some(reference) = reference {
        cmd.arg(reference);
    }

    cmd.build()
}

#[binary_benchmark]
#[bench::v0_2_0(WORKSPACE_ROOT, "v0.2.0")]
#[bench::whitespace_only(WORKSPACE_ROOT, "origin/test/whitespace-only")]
fn check_self<P: AsRef<Path>>(repo: P, reference: &str) -> gungraun::Command {
    gungraun::Command::new(env!("CARGO_BIN_EXE_herostratus"))
        .arg("check")
        .arg(repo.as_ref())
        .arg(reference)
        .build()
}

// See: https://gungraun.github.io/gungraun/latest/html/index.html
binary_benchmark_group!(name = add; benchmarks = add_url);
binary_benchmark_group!(name = check; benchmarks = check_self);
main!(binary_benchmark_groups = add, check);
