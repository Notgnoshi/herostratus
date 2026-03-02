use std::io::{IsTerminal, Write};

use clap::Parser;
use sha1::Digest;
use tracing_subscriber::EnvFilter;

mod commit;
mod search;

#[derive(Debug, clap::Parser)]
#[clap(
    about = "Find a Git commit whose message contains its own hash",
    version
)]
struct Args {
    /// Force colored output
    ///
    /// Otherwise, color will be used if stdout is a terminal.
    #[clap(long)]
    color: bool,

    /// Set the application log level
    #[clap(short, long, default_value_t = tracing::Level::INFO)]
    log_level: tracing::Level,

    /// Number of hex characters of the hash to match [4..16]
    #[clap(short = 'n', long, default_value_t = 8)]
    prefix_len: u32,

    /// Number of worker threads (default: available CPU cores)
    #[clap(short = 'j', long)]
    threads: Option<usize>,

    /// Override author/committer name (default: auto-detect from git config)
    #[clap(long)]
    name: Option<String>,

    /// Override author/committer email (default: auto-detect from git config)
    #[clap(long)]
    email: Option<String>,

    /// Override author/committer timestamp as seconds since Unix epoch (default: current time)
    #[clap(long)]
    timestamp: Option<i64>,
}

fn main() -> eyre::Result<()> {
    let args = Args::parse();
    let use_color = std::io::stderr().is_terminal() || args.color;
    if use_color {
        color_eyre::install()?;
    }

    let filter = EnvFilter::builder()
        .with_default_directive(args.log_level.into())
        .with_env_var("HEROSTRATUS_QUINE_LOG")
        .from_env_lossy();
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .with_ansi(use_color)
        .with_writer(std::io::stderr)
        .init();

    if args.prefix_len < 4 || args.prefix_len > 16 {
        eyre::bail!(
            "--prefix-len must be between 4 and 16 (got {})",
            args.prefix_len
        );
    }

    let (name, email) = resolve_author(&args)?;
    let timestamp = args.timestamp.unwrap_or_else(|| {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock is before Unix epoch")
            .as_secs() as i64
    });

    tracing::info!(
        name,
        email,
        timestamp,
        prefix_len = args.prefix_len,
        "Building commit template"
    );

    let template = commit::CommitTemplate::new(args.prefix_len, &name, &email, timestamp);
    let nprocs = std::thread::available_parallelism()?;
    let num_threads = args.threads.unwrap_or(nprocs.get());

    let start_time = std::time::Instant::now();
    let result = search::search(&template, num_threads);
    let elapsed = start_time.elapsed();

    match result {
        Some(result) => {
            // Verify by re-hashing the full object
            let full_object = template.build_full_object(result.hex_prefix.as_bytes());
            let verify_hash = sha1::Sha1::digest(&full_object);
            let verify_hex: String = verify_hash.iter().map(|b| format!("{b:02x}")).collect();

            let result_hex: String = result.raw_hash.iter().map(|b| format!("{b:02x}")).collect();
            eyre::ensure!(
                verify_hex == result_hex,
                "Verification failed: search returned {result_hex} but re-hash gives {verify_hex}"
            );
            eyre::ensure!(
                verify_hex.starts_with(&result.hex_prefix),
                "Verification failed: hash {verify_hex} does not start with prefix {}",
                result.hex_prefix
            );

            tracing::info!(hash = %result_hex, prefix = %result.hex_prefix, ?elapsed, "Found quine commit!");

            // Write just the commit content (without the git object header) to stdout so
            // it can be piped to:
            //   git hash-object -t commit -w --stdin
            let content_start = full_object
                .iter()
                .position(|&b| b == 0)
                .expect("git object must contain a NUL byte")
                + 1;
            std::io::stdout().write_all(&full_object[content_start..])?;
        }
        None => {
            tracing::error!(prefix_len = args.prefix_len, ?elapsed, "No match found");
            eyre::bail!(
                "No match found in the entire search space (prefix_len={})",
                args.prefix_len
            );
        }
    }

    Ok(())
}

/// Resolve author name and email from CLI args or git config.
fn resolve_author(args: &Args) -> eyre::Result<(String, String)> {
    if let (Some(name), Some(email)) = (&args.name, &args.email) {
        return Ok((name.clone(), email.clone()));
    }

    // Try to auto-detect from git config
    let config = gix::config::File::from_globals()
        .map_err(|e| eyre::eyre!("Failed to read git config: {e}"))?;

    let name = match &args.name {
        Some(n) => n.clone(),
        None => config
            .string("user.name")
            .ok_or_else(|| {
                eyre::eyre!("Could not detect user.name from git config; use --name to override")
            })?
            .to_string(),
    };

    let email = match &args.email {
        Some(e) => e.clone(),
        None => config
            .string("user.email")
            .ok_or_else(|| {
                eyre::eyre!("Could not detect user.email from git config; use --email to override")
            })?
            .to_string(),
    };

    Ok((name, email))
}
