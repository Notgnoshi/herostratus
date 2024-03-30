mod achievement;
mod achievements;
mod git;

use std::io::IsTerminal;

use clap::Parser;
use eyre::WrapErr;
use tracing::Level;
use tracing_subscriber::EnvFilter;

#[derive(Parser)]
#[clap(about, verbatim_doc_comment, version)]
struct CliArgs {
    /// A path to a work tree or bare repository, or a clone URL
    repository: String,

    /// The reference or revision to search for achievements
    ///
    /// Examples:
    /// * v0.1.0 (tag)
    /// * HEAD (symbolic ref)
    /// * origin/main (remote branch)
    /// * main (branch)
    /// * bf266ef (short rev)
    /// * bf266effe9701f07ebeb0935bd2c48c5f02bc483 (full rev)
    #[clap(verbatim_doc_comment)]
    reference: String,

    /// Set the application log level
    ///
    /// You can also set the value of the HEROSTRATUS_LOG environment variable like so
    ///     HEROSTRATUS_LOG=debug
    ///     HEROSTRATUS_LOG=info,herostratus::git=trace
    /// If HEROSTRATUS_LOG is non-empty, the value of --log-level will be ignored.
    #[clap(short, long, verbatim_doc_comment, default_value_t = tracing::Level::INFO)]
    log_level: Level,
}

fn main() -> eyre::Result<()> {
    let use_color = std::io::stdout().is_terminal();
    if use_color {
        color_eyre::install()?;
    }

    let args = CliArgs::parse();

    let filter = EnvFilter::builder()
        .with_default_directive(args.log_level.into())
        .with_env_var("HEROSTRATUS_LOG")
        .from_env_lossy();
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_ansi(use_color)
        .with_writer(std::io::stderr)
        .init();

    let repo = git::fetch_or_find(&args.repository)
        .wrap_err(format!("Could not find or clone {:?}", args.repository))?;

    let oid = git::rev_parse(&args.reference, &repo)
        .wrap_err(format!("Failed to resolve reference {:?}", args.reference))?;
    let oids = git::rev_walk(oid, &repo).wrap_err(format!("Failed to walk OID {oid:?}"))?;
    let oids = oids.filter_map(|o| match o {
        Ok(o) => Some(o),
        Err(e) => {
            tracing::error!("Skipping OID: {e:?}");
            None
        }
    });

    for achievement in achievement::process_rules(oids, &repo, achievements::builtin_rules()) {
        tracing::info!("Found achievement: {achievement:?}");
    }

    // for oid in oids {
    //     // I'm not sure why this would happen, nor why the iterator wouldn't just return None.
    //     // Maybe it's because returning None gives no context?
    //     let oid = oid.wrap_err("Failed to get next OID")?;
    //     let commit = repo
    //         .find_commit(oid)
    //         .wrap_err(format!("Failed to find commit with OID {oid:?}"))?;
    //     tracing::debug!(
    //         "commit: {:?} summary: {:?}",
    //         commit.id(),
    //         commit.summary().unwrap_or("??")
    //     );
    // }

    Ok(())
}
