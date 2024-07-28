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
///
// See also the RepositoryConfig
/// These CLI arguments are saved to Herostratus's generated config.toml, where they may be tweaked
/// further.
///
/// NOTE: The config file will only be modified if cloning is successful. Use '--skip-clone' to
/// modify the config file without cloning.
#[derive(Debug, clap::Args)]
pub struct AddArgs {
    /// A valid clone URL
    ///
    /// See `https://git-scm.com/docs/git-clone/` for documentation. URLs must be prefixed by a
    /// supported protocol (ssh://, https://, or file://).
    #[clap(verbatim_doc_comment)]
    pub url: String,

    /// The branch to clone, and check
    ///
    /// If given, only the specified branch will be fetched.
    ///
    /// If given, this will be the branch that will be checked for achievements instead of the
    /// default HEAD.
    pub branch: Option<String>,

    /// The path to clone the repository
    ///
    /// Given a URL like `https://github.com/Notgnoshi/herostratus.git`, the repository will be
    /// cloned to `cgit/Notgnoshi/herostratus.git/` in the application data directory by default.
    #[clap(short, long, verbatim_doc_comment)]
    pub path: Option<PathBuf>,

    /// Override the repository name
    ///
    /// Multiple pairs of remote URLs and branches may be configured, so long as they have unique
    /// names.
    #[clap(long)]
    pub name: Option<String>,

    /// Forcefully overwrite an existing clone, if it exists
    #[clap(long)]
    pub force: bool,

    /// Skip cloning; just add the repository to Herostratus's config file
    #[clap(long)]
    pub skip_clone: bool,

    /// The SSH or HTTPS remote username
    ///
    /// If not set, it will default to 'git'.
    #[clap(long)]
    pub remote_username: Option<String>,

    /// The path to an appropriate SSH private key
    ///
    /// Often, if a private key is given, the public key need not be specified if it can be
    /// inferred. If a private key is not specified for an SSH URL, Herostratus will attempt to use
    /// your SSH agent.
    #[clap(long)]
    pub ssh_private_key: Option<PathBuf>,

    /// The path to an appropriate SSH public key
    #[clap(long)]
    pub ssh_public_key: Option<PathBuf>,

    /// The SSH key passphrase, if required
    #[clap(long)]
    pub ssh_passphrase: Option<String>,

    /// The password to use for HTTPS clone URLs
    ///
    /// It's very likely that you will also need to set `remote_username`.
    ///
    /// If the password is not set for an HTTPS clone URL, Herostratus will attempt to use your
    /// configured Git `credential.helper`.
    #[clap(long)]
    pub https_password: Option<String>,
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
