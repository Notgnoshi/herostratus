use std::path::Path;
use std::time::Instant;

use crate::achievement::{grant, Achievement};
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
    let achievements = grant(&args.reference, &repo)?;

    process_achievements(achievements)
}

pub fn check_all(_args: &CheckAllArgs, config: &Config, _data_dir: &Path) -> eyre::Result<()> {
    tracing::info!("Checking repositories ...");
    let start = Instant::now();
    for (name, config) in config.repositories.iter() {
        let span = tracing::debug_span!("check", name = name);
        let _enter = span.enter();
        let repo = find_local_repository(&config.path)?;
        let reference = config
            .branch
            .clone()
            .unwrap_or_else(|| String::from("HEAD"));
        let achievements = grant(&reference, &repo)?;
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
