use std::path::PathBuf;

#[derive(Debug, clap::Parser)]
#[clap(about, verbatim_doc_comment, version)]
pub struct Args {
    /// Set the application log level
    ///
    /// You can also set the value of the HEROSTRATUS_LOG environment variable like so
    ///     HEROSTRATUS_LOG=debug
    ///     HEROSTRATUS_LOG=info,herostratus::git=trace
    /// If HEROSTRATUS_LOG is non-empty, the value of --log-level will be ignored.
    #[clap(short, long, verbatim_doc_comment, default_value_t=tracing::Level::INFO)]
    pub log_level: tracing::Level,

    /// Override the application data directory
    ///
    /// Will default to a platform-dependent directory consistent to XDG. Use `--get-data-dir` to
    /// query where Herostratus will save data to.
    #[clap(long)]
    pub data_dir: Option<PathBuf>,

    /// Query the application data directory and exit
    #[clap(long)]
    pub get_data_dir: bool,

    /// Override the application config file
    ///
    /// Will default to `herostratus.toml` located in the application data directory.
    #[clap(short = 'C', long)]
    pub config_file: Option<PathBuf>,

    // TODO: Add a get_default_config?
    /// Query the current application configuration and exit
    ///
    /// Merges CLI argument and the TOML config file.
    #[clap(long)]
    pub get_config: bool,

    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Debug, clap::Subcommand)]
pub enum Command {
    Check(CheckArgs),
    Add(AddArgs),
    CheckAll(CheckAllArgs),
    FetchAll(FetchAllArgs),
    Remove(RemoveArgs),
}

/// Statelessly process the given file path and reference
#[derive(Debug, clap::Args)]
pub struct CheckArgs {
    /// An absolute or relative path to a Git repository
    ///
    /// May be a bare repository. May be the path to the .git directory.
    pub path: PathBuf,

    /// The Git reference or revision to process
    ///
    /// All commits reachable from this reference will be processed
    #[clap(default_value = "HEAD")]
    pub reference: String,

    /// How many commits to process
    #[clap(short, long)]
    pub depth: Option<usize>,
    // TODO: Rule filtering
    // TODO: Commit filtering
}

/// Add a repository to be processed later
#[derive(Debug, clap::Args)]
pub struct AddArgs {
    /// A valid clone URL
    ///
    /// See `https://git-scm.com/docs/git-clone/` for documentation. URLs must be prefixed by a
    /// supported protocol (ssh://, https://, or file://).
    #[clap(verbatim_doc_comment)]
    pub url: String,

    /// The path to clone the repository
    ///
    /// Given a URL like `https://github.com/Notgnoshi/herostratus.git`, the repository will be
    /// cloned to `cgit/Notgnoshi/herostratus.git/` in the application data directory by default.
    #[clap(short, long, verbatim_doc_comment)]
    pub path: Option<PathBuf>,

    // TODO: Add optional branch. If given, will configure the reference to parse, and will do a
    // fetch of *just* that branch. This needs to be thought out more, because there should be a
    // way to clone the whole thing, and use a rev instead of a ref ...
    /// Skip cloning the repository
    #[clap(long)]
    pub skip_clone: bool,

    /// Forcefully overwrite an existing clone, if it exists
    #[clap(long)]
    pub force: bool,
    // TODO: Authentication
}

/// Process rules on all cloned repositories
#[derive(Debug, clap::Args)]
pub struct CheckAllArgs;

/// Fetch each repository
#[derive(Debug, clap::Args)]
pub struct FetchAllArgs;

/// Remove the given repository
#[derive(Debug, clap::Args)]
pub struct RemoveArgs {
    pub url: Option<String>,
    pub path: Option<PathBuf>,
}
