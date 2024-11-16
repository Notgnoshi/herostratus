mod cli;
mod git;
mod job;

use std::io::IsTerminal;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::Instant;

use clap::Parser;
use cli::Args;
use eyre::WrapErr;
use tracing_subscriber::EnvFilter;

fn main() -> eyre::Result<()> {
    let use_color = std::io::stdout().is_terminal();
    if use_color {
        color_eyre::install()?;
    }

    let mut args = Args::parse();

    let filter = EnvFilter::builder()
        .with_default_directive(args.log_level.into())
        .with_env_var("HEROSTRATUS_LOG")
        .from_env_lossy();
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_ansi(use_color)
        .with_writer(std::io::stderr)
        .init();

    // prefix is in characters in the hash, where each one is a nibble
    // this tool uses a u128 as the hash prefix, so don't allow anything that would overflow that.
    const PREFIX_LIMIT: u8 = 128 / 4;
    if args.prefix_length > PREFIX_LIMIT {
        tracing::warn!("Hash prefixes larger than {PREFIX_LIMIT} nibbles aren't supported");
    }
    let prefix_length = u8::min(args.prefix_length, PREFIX_LIMIT);

    if args.jobs == 0 {
        args.jobs = std::thread::available_parallelism()?.into();
    }

    let repo =
        git2::Repository::discover(&args.repository).wrap_err("Failed to discovery repository")?;

    // Eventually, this will need to support SHA1 and SHA256.
    // See: https://git-scm.com/docs/hash-function-transition
    let output = std::process::Command::new("git")
        .arg("rev-parse")
        .arg("--show-object-format")
        .current_dir(&args.repository)
        .output()
        .wrap_err("Failed to check the repository object format")?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stdout = stdout.trim();
    tracing::debug!(
        "Found repository at {:?} with object format {:?}",
        repo.path(),
        stdout
    );
    if stdout != "sha1" {
        eyre::bail!("Only repositories with SHA1 object format are supported, not {stdout:?}");
    }

    // For empty commits, the commit hash is the SHA1 hash of the raw commit contents (including
    // the author and committer names and timestamps). Since *making* 16^7 commits would be too
    // expensive, we make the initial commit, grab the raw commit contents, and then brute force a
    // shit ton of hashes.
    let commit = git::generate_initial_commit(&repo, prefix_length)
        .wrap_err("Failed to generate initial commit")?;

    // The commit hash is the SHA1 hash of this string
    let raw_commit = git::get_raw_commit(&commit);
    debug_assert_eq!(git::sha1(&raw_commit), commit.id());

    // Now we have the raw string being hashed, time to spin up a shit-ton of workers to brute
    // force different variations of it!
    //
    // Split up the range 0000000..FFFFFFF into N workers (0..16^prefix_length)

    // TODO: Continuous u128 ranges don't result in continuous hash prefix ranges, because the
    // hashes are formatted like ABCD for [0xAB, 0xCD], so for a u128 prefix like 0xABC would get
    // formatted into a hash string like AB0C.
    //
    // I think I need to start over and throw out the u128 "optimization"
    let min_prefix: u128 = 0;
    let max_prefix: u128 = u128::MAX >> ((PREFIX_LIMIT - prefix_length) * 4);
    let worker_chunk_size = max_prefix / args.jobs as u128;
    debug_assert_eq!(max_prefix.trailing_ones(), prefix_length as u32 * 4);
    tracing::debug!("Brute forcing the {prefix_length} nibble prefix range {min_prefix:#x}..={max_prefix:#x} with {worker_chunk_size:#x} bit chunks");

    let start = Instant::now();

    let mut worker_start;
    let mut worker_end = 0;
    let mut handles = Vec::new();
    let is_running = Arc::new(AtomicBool::new(true));
    for worker in 0..args.jobs {
        // start and end are inclusive, so that we don't have overflow at the end with a full
        // 128-bit prefix
        worker_start = if worker == 0 {
            min_prefix
        } else {
            worker_end + 1
        };
        worker_end = if worker == args.jobs - 1 {
            max_prefix
        } else {
            worker_start + worker_chunk_size
        };

        let handle = job::spawn_worker_thread(
            worker,
            worker_start,
            worker_end,
            prefix_length,
            raw_commit.clone(),
            is_running.clone(),
        );
        handles.push(handle);
    }

    let results = job::join_all(handles, is_running);
    tracing::info!("Workers finished after {:?}", start.elapsed());
    // tracing::debug!("Looking for {} {:x?}", commit.id(), commit.id().as_bytes());

    if !results.is_empty() {
        tracing::info!("Found results: {results:x?}");
        // TODO: Edit the commit with the calculated hash prefix, verify
    }

    Ok(())
}
