use std::io::IsTerminal;
use std::path::PathBuf;

use clap::Parser;
use eyre::WrapErr;
use tracing::Level;
use tracing_subscriber::EnvFilter;

#[derive(Parser)]
#[clap(about, verbatim_doc_comment, version)]
struct CliArgs {
    /// A path to a work tree or bare repository, or a clone URL
    #[clap(verbatim_doc_comment, required_unless_present = "get_data_dir")]
    repository: Option<String>,

    /// The reference or revision to search for achievements
    ///
    /// Examples:
    /// * v0.1.0 (tag)
    /// * HEAD (symbolic ref)
    /// * origin/main (remote branch)
    /// * main (branch)
    /// * bf266ef (short rev)
    /// * bf266effe9701f07ebeb0935bd2c48c5f02bc483 (full rev)
    #[clap(verbatim_doc_comment, required_unless_present = "get_data_dir")]
    reference: Option<String>,

    /// Set the application log level
    ///
    /// You can also set the value of the HEROSTRATUS_LOG environment variable like so
    ///     HEROSTRATUS_LOG=debug
    ///     HEROSTRATUS_LOG=info,herostratus::git=trace
    /// If HEROSTRATUS_LOG is non-empty, the value of --log-level will be ignored.
    #[clap(short, long, verbatim_doc_comment, default_value_t = tracing::Level::INFO)]
    log_level: Level,

    /// Override the application data directory
    ///
    /// Will default to a platform-dependent directory consistent with the XDG spec. You can use
    /// `--get-data-dir` to determine where Herostratus will save data.
    #[clap(long)]
    data_dir: Option<PathBuf>,

    /// Get the application data directory and exit
    #[clap(long)]
    get_data_dir: bool,

    /// If <REPOSITORY> is a clone URL, do not use the cached clone if present
    #[clap(long)]
    force_clone: bool,

    /// If <REPOSITORY> is a clone URL, and a cached repository is found, skip fetching
    #[clap(long)]
    skip_fetch: bool,
}

fn main() -> eyre::Result<()> {
    let use_color = std::io::stdout().is_terminal();
    if use_color {
        color_eyre::install()?;
    }

    let args = CliArgs::parse();
    let proj_dir = directories::ProjectDirs::from("com", "Notgnoshi", "Herostratus").ok_or(
        eyre::eyre!("Failed to determine Herostratus data directory"),
    )?;
    let data_dir = proj_dir.data_local_dir();
    let data_dir = args.data_dir.unwrap_or(data_dir.to_owned());

    if args.get_data_dir {
        println!("{}", data_dir.display());
        return Ok(());
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

    let repo = herostratus::git::fetch_or_find(
        args.repository.as_ref().unwrap(),
        &data_dir,
        args.force_clone,
        args.skip_fetch,
    )
    .wrap_err(format!(
        "Could not find or clone {:?}",
        args.repository.unwrap()
    ))?;

    let achievements = herostratus::achievement::grant(&args.reference.unwrap(), &repo)
        .wrap_err("Failed to grant achievements")?;

    for achievement in achievements {
        println!("{achievement:?}");
    }

    Ok(())
}
