mod git;

use clap::Parser;
use eyre::WrapErr;
use tracing::Level;
use tracing_subscriber::EnvFilter;

#[derive(Parser)]
#[clap(about, verbatim_doc_comment, version)]
struct CliArgs {
    /// A path to a work tree or bare repository, or a clone URL
    repository: String,

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
    color_eyre::install()?;
    let args = CliArgs::parse();

    let filter = EnvFilter::builder()
        .with_default_directive(args.log_level.into())
        .with_env_var("HEROSTRATUS_LOG")
        .from_env_lossy();
    tracing_subscriber::fmt().with_env_filter(filter).init();

    let _repo = git::fetch_or_find(&args.repository)
        .wrap_err(format!("Could not find or clone {:?}", args.repository))?;

    Ok(())
}
