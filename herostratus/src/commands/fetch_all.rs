use std::path::Path;
use std::time::Instant;

use crate::cli::FetchAllArgs;
use crate::config::Config;
use crate::git::clone::{clone_repository, find_local_repository_gix, pull_branch_gix};

pub fn fetch_all(_args: &FetchAllArgs, config: &Config, _data_dir: &Path) -> eyre::Result<usize> {
    tracing::info!("Fetching repositories ...");
    let start = Instant::now();
    let mut fetched_commits = 0;
    for (name, config) in config.repositories.iter() {
        let _span = tracing::debug_span!("fetch", name = name).entered();
        let mut skip_fetch = false;
        let repo = match find_local_repository_gix(&config.path) {
            Ok(repo) => repo,
            // Handle the case where 'add --skip-clone' was used
            Err(e) => {
                tracing::error!(
                    "Failed to find repository '{:?}': {e}. Attempting to clone it ...",
                    config.path.display()
                );
                let force = false;
                skip_fetch = true;
                let git2_repo = clone_repository(config, force)?;
                crate::git::git2_to_gix(&git2_repo)
            }
        };

        if !skip_fetch {
            fetched_commits += pull_branch_gix(config, &repo)?;
        }
    }
    tracing::info!(
        "... fetched {} repositories after {:.2?}",
        config.repositories.len(),
        start.elapsed()
    );

    Ok(fetched_commits)
}
