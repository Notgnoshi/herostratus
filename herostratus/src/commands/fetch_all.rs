use std::path::Path;
use std::time::{Duration, Instant};

use crate::cli::FetchAllArgs;
use crate::config::{Config, RepositoryConfig};
use crate::git::clone::{clone_repository, find_local_repository, pull_branch};

#[derive(Clone, Debug, Default)]
pub struct FetchStat {
    pub name: String,
    /// Only set if fetched, and not if newly cloned
    pub num_commits_fetched: Option<u64>,
    pub elapsed: Duration,
}

/// Fetch (or clone) a single configured repository
pub fn fetch_one(name: &str, config: &RepositoryConfig) -> eyre::Result<FetchStat> {
    let _span = tracing::debug_span!("fetch", name = name).entered();
    let repo_start = Instant::now();
    let mut stat = FetchStat {
        name: name.to_string(),
        ..Default::default()
    };
    let mut skip_fetch = false;
    let repo = match find_local_repository(&config.path) {
        Ok(repo) => repo,
        // Handle the case where 'add --skip-clone' was used
        Err(e) => {
            tracing::error!(
                "Failed to find repository '{:?}': {e}. Attempting to clone it ...",
                config.path.display()
            );
            let force = false;
            skip_fetch = true;
            // TODO: Count number of commits cloned?
            clone_repository(config, force)?
        }
    };

    if !skip_fetch {
        let fetched = pull_branch(config, &repo)?;
        stat.num_commits_fetched = Some(fetched);
    }
    stat.elapsed = repo_start.elapsed();
    Ok(stat)
}

pub fn fetch_all(
    _args: &FetchAllArgs,
    config: &Config,
    _data_dir: &Path,
) -> eyre::Result<Vec<FetchStat>> {
    tracing::info!("Fetching repositories ...");
    let mut stats = Vec::new();
    let start = Instant::now();
    for (name, repo_config) in config.repositories.iter() {
        stats.push(fetch_one(name, repo_config)?);
    }
    tracing::info!(
        "... fetched {} repositories after {:.2?}",
        config.repositories.len(),
        start.elapsed()
    );

    Ok(stats)
}
