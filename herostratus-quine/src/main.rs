mod cli;
mod git;
mod job;

use std::io::IsTerminal;

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
    args.prefix_length = u8::min(args.prefix_length, 40);
    if args.jobs == 0 {
        args.jobs = std::thread::available_parallelism()?.into();
    }

    let filter = EnvFilter::builder()
        .with_default_directive(args.log_level.into())
        .with_env_var("HEROSTRATUS_LOG")
        .from_env_lossy();
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_ansi(use_color)
        .with_writer(std::io::stderr)
        .init();

    tracing::debug!("{args:?}");

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
    let commit = git::generate_initial_commit(&repo, args.prefix_length)
        .wrap_err("Failed to generate initial commit")?;

    // The commit hash is the SHA1 hash of this string
    let raw_commit = git::get_raw_commit(&commit);

    // Smoke test!
    let verify_hash = git::sha1(&raw_commit);
    if commit.id() != verify_hash {
        let err = Err(eyre::eyre!("raw commit:\n{raw_commit}"))
            .wrap_err(format!("actual hash: {}", commit.id()))
            .wrap_err(format!("calculated hash: {}", verify_hash))
            .wrap_err("Failed to verify SHA1 hash of initial quine commit");
        return err;
    }

    // Now we have the raw string being hashed, time to spin up a shit-ton of workers to brute
    // force different variations of it!

    Ok(())
}
