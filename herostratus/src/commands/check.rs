use std::path::Path;
use std::time::Instant;

use crate::achievement::{Achievement, grant};
use crate::cli::{CheckAllArgs, CheckArgs};
use crate::config::Config;
use crate::git::clone::find_local_repository;

// Stateless; do not allow filesystem modification, or reading from application data
pub fn check(args: &CheckArgs) -> eyre::Result<()> {
    tracing::info!(
        "Checking repository {:?}, reference {:?} for achievements ...",
        args.path.display(),
        args.reference
    );
    let repo = find_local_repository(&args.path)?;
    let achievements = grant(None, &args.reference, &repo)?;

    process_achievements(achievements)
}

pub fn check_all(args: &CheckAllArgs, config: &Config, data_dir: &Path) -> eyre::Result<()> {
    if !args.no_fetch {
        let _newly_fetched = crate::commands::fetch_all(&args.into(), config, data_dir)?;
    }

    tracing::info!("Checking repositories ...");
    let start = Instant::now();
    for (name, repo_config) in config.repositories.iter() {
        let _span = tracing::debug_span!("check", name = name).entered();
        let repo = find_local_repository(&repo_config.path)?;
        let reference = repo_config
            .branch
            .clone()
            .unwrap_or_else(|| String::from("HEAD"));
        let achievements = grant(Some(config), &reference, &repo)?;
        process_achievements(achievements)?;
    }
    tracing::info!(
        "... checked {} repositories after {:.2?}",
        config.repositories.len(),
        start.elapsed()
    );

    Ok(())
}

/// A common achievement sink that both check and check_all can use
fn process_achievements(achievements: impl Iterator<Item = Achievement>) -> eyre::Result<()> {
    // TODO: Support different output formats
    for achievement in achievements {
        println!("{achievement:?}");
    }
    Ok(())
}
