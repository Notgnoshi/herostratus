use std::path::Path;
use std::time::Instant;

use crate::cli::FetchAllArgs;
use crate::config::Config;
use crate::git::clone::{clone_repository, find_local_repository, pull_branch};

pub fn fetch_all(_args: &FetchAllArgs, config: &Config, _data_dir: &Path) -> eyre::Result<()> {
    tracing::info!("Fetching repositories ...");
    let start = Instant::now();
    for (name, config) in config.repositories.iter() {
        let span = tracing::debug_span!("fetch", name = name);
        let _enter = span.enter();
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
                clone_repository(config, force)?
            }
        };

        if !skip_fetch {
            pull_branch(config, &repo)?
        }
    }
    tracing::info!(
        "... fetched {} repositories after {:.2?}",
        config.repositories.len(),
        start.elapsed()
    );

    Ok(())
}
