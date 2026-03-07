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

    /// Number of hex characters in the nonce field [4..16]
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

    /// Parent commit hash (fortune-teller mode).
    ///
    /// The generated commit will have this as its parent, placing it on the same branch.
    #[clap(long)]
    parent: Option<String>,

    /// Target hash prefix to match (fortune-teller mode).
    ///
    /// Generate a commit whose hash starts with this hex prefix. This is the short hash from a
    /// previous commit's message -- the "prediction" that we are fulfilling.
    #[clap(long)]
    target_prefix: Option<String>,
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

    if args.target_prefix.is_some() && args.parent.is_none() {
        eyre::bail!("--target-prefix requires --parent");
    }

    if let Some(ref tp) = args.target_prefix {
        validate_target_prefix(tp)?;
    }

    let (name, email) = resolve_author(&args)?;
    let timestamp = args.timestamp.unwrap_or_else(|| {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock is before Unix epoch")
            .as_secs() as i64
    });

    let parent = args.parent.as_deref();

    let (template, target) = if let Some(ref target_hex) = args.target_prefix {
        let parent = parent.unwrap();

        tracing::info!(
            name,
            email,
            timestamp,
            prefix_len = args.prefix_len,
            parent,
            target_prefix = target_hex,
            "Building fortune-teller commit template"
        );

        let template = commit::CommitTemplate::new_fortune_teller(
            args.prefix_len,
            parent,
            &name,
            &email,
            timestamp,
        );
        let target = parse_target_prefix(target_hex)?;
        (template, Some(target))
    } else {
        tracing::info!(
            name,
            email,
            timestamp,
            prefix_len = args.prefix_len,
            ?parent,
            "Building quine commit template"
        );

        let template =
            commit::CommitTemplate::new(args.prefix_len, parent, &name, &email, timestamp);
        (template, None)
    };

    let nprocs = std::thread::available_parallelism()?;
    let num_threads = args.threads.unwrap_or(nprocs.get());

    let start_time = std::time::Instant::now();
    let result = search::search(&template, num_threads, target);
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

            if let Some((_, target_len)) = target {
                let target_hex = args.target_prefix.as_ref().unwrap();
                eyre::ensure!(
                    verify_hex.starts_with(target_hex),
                    "Verification failed: hash {verify_hex} does not start with target {target_hex}"
                );
                tracing::info!(
                    hash = %result_hex,
                    target = target_hex,
                    nonce = %result.hex_prefix,
                    nonce_len = args.prefix_len,
                    target_len,
                    ?elapsed,
                    "Found fortune-teller commit!"
                );
            } else {
                eyre::ensure!(
                    verify_hex.starts_with(&result.hex_prefix),
                    "Verification failed: hash {verify_hex} does not start with prefix {}",
                    result.hex_prefix
                );
                tracing::info!(
                    hash = %result_hex,
                    prefix = %result.hex_prefix,
                    ?elapsed,
                    "Found quine commit!"
                );
            }

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

/// Validate a target prefix is valid lowercase hex.
fn validate_target_prefix(hex: &str) -> eyre::Result<()> {
    if hex.len() < 4 || hex.len() > 16 {
        eyre::bail!(
            "--target-prefix must be 4-16 hex characters (got {})",
            hex.len()
        );
    }
    if !hex.chars().all(|c| c.is_ascii_hexdigit()) {
        eyre::bail!("--target-prefix must be valid hex: {hex}");
    }
    if hex != hex.to_ascii_lowercase() {
        eyre::bail!("--target-prefix must be lowercase hex: {hex}");
    }
    Ok(())
}

/// Parse a hex prefix string into a (value, length) pair for the search.
fn parse_target_prefix(hex: &str) -> eyre::Result<(u64, u32)> {
    let len = hex.len() as u32;
    let value = u64::from_str_radix(hex, 16)
        .map_err(|_| eyre::eyre!("--target-prefix must be valid hex: {hex}"))?;
    Ok((value, len))
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
