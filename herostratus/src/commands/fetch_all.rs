use std::path::Path;
use std::time::{Duration, Instant};

use crate::cli::FetchAllArgs;
use crate::config::{Config, RepositoryConfig};
use crate::git::clone::{clone_repository, find_local_repository, pull_branch};

pub(crate) fn current_rule_versions(config: &Config) -> Vec<(usize, u32)> {
    let default_config = crate::config::RulesConfig::default();
    let config = config.rules.as_ref().unwrap_or(&default_config);
    crate::rules::builtin_rules(config)
        .iter()
        .map(|rule| (rule.meta().id, rule.version()))
        .collect()
}

#[derive(Clone, Debug, Default)]
pub struct FetchStat {
    pub name: String,
    /// Only set if fetched, and not if newly cloned
    pub num_commits_fetched: Option<u64>,
    pub elapsed: Duration,
}

/// Fetch (or clone) a single configured repository
pub fn fetch_one(
    name: &str,
    config: &RepositoryConfig,
    data_dir: &Path,
    current_rules: &[(usize, u32)],
) -> eyre::Result<FetchStat> {
    let _span = tracing::debug_span!("fetch", name = name).entered();
    let repo_start = Instant::now();
    let mut stat = FetchStat {
        name: name.to_string(),
        ..Default::default()
    };
    let mut skip_fetch = false;
    let mut repo = match find_local_repository(&config.path) {
        Ok(repo) => repo,
        // Handle the case where 'add --skip-clone' was used
        Err(e) => {
            tracing::error!(
                "Failed to find repository '{:?}': {e}. Attempting to clone it ...",
                config.path.display()
            );
            let force = false;
            skip_fetch = true;
            let checkpoint = crate::cache::CheckpointCache::from_data_dir(data_dir, name)?;

            // For performance, we only shallow clone and deepen if we have a valid checkpoint that
            // we expect to reach. Otherwise (if there were rules versions that were updated, or new
            // rules added) we do a full clone, because it's *far* faster to do a full clone than an
            // iterative shallow deepend as-needed.
            let shallow = if checkpoint.data.commit.is_some()
                && checkpoint.data.has_processed_all(current_rules)
            {
                let depth = crate::git::clone::DEFAULT_SHALLOW_DEPTH;
                tracing::info!(
                    "Checkpoint {:?} covers all current rules; shallow clone with depth={depth}",
                    checkpoint.data.commit
                );
                Some(depth)
            } else {
                tracing::info!("No usable checkpoint or rule set changed; cloning full history");
                None
            };
            clone_repository(config, force, shallow)?
        }
    };

    if !skip_fetch {
        let fetched = pull_branch(config, &mut repo)?;
        stat.num_commits_fetched = Some(fetched);
    }
    stat.elapsed = repo_start.elapsed();
    Ok(stat)
}

pub fn fetch_all(
    _args: &FetchAllArgs,
    config: &Config,
    data_dir: &Path,
) -> eyre::Result<Vec<FetchStat>> {
    tracing::info!("Fetching repositories ...");
    let mut stats = Vec::new();
    let start = Instant::now();
    // Maintenance note: There's now two places where we calculate which rules are enabled. I don't
    // *expect* diverging calculations, so I thought it was easier just to recalculate it here. Time
    // will tell if I'm right :)
    let current_rules = current_rule_versions(config);
    for (name, repo_config) in config.repositories.iter() {
        stats.push(fetch_one(name, repo_config, data_dir, &current_rules)?);
    }
    tracing::info!(
        "... fetched {} repositories after {:.2?}",
        config.repositories.len(),
        start.elapsed()
    );

    Ok(stats)
}
