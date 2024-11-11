use std::path::PathBuf;

use clap::Parser;

/// Generate a commit containing its own short hash
#[derive(Debug, Parser)]
#[clap(verbatim_doc_comment, version)]
pub struct Args {
    /// Set the application log level
    ///
    /// You can also set the value of the HEROSTRATUS_LOG environment variable like so
    ///     HEROSTRATUS_LOG=debug
    ///     HEROSTRATUS_LOG=info,herostratus::git=trace
    /// If HEROSTRATUS_LOG is non-empty, the value of --log-level will be ignored.
    #[clap(short, long, verbatim_doc_comment, default_value_t=tracing::Level::DEBUG)]
    pub log_level: tracing::Level,

    /// The path to the repository to generate the quine commit in.
    ///
    /// The quine commit will be an empty commit made on the HEAD of given repository.
    #[clap()]
    pub repository: PathBuf,

    /// The length of the short-hash prefix to match
    #[clap(short, long, default_value_t = 7)]
    pub prefix_length: u8,

    /// Number of threads to run. Defaults to the number of cores
    #[clap(short, long, default_value_t = 0)]
    pub jobs: usize,
}
