use std::path::Path;

use crate::achievement::{grant, Achievement};
use crate::cli::{CheckAllArgs, CheckArgs};
use crate::config::Config;
use crate::git::clone::find_local_repository;

// Stateless; do not allow filesystem modification, or reading from application data
pub fn check(args: &CheckArgs) -> eyre::Result<()> {
    tracing::info!(
        "Processing repository {:?}, reference {:?} for achievements ...",
        args.path.display(),
        args.reference
    );
    let repo = find_local_repository(&args.path)?;
    let achievements = grant(&args.reference, &repo)?;

    process_achievements(achievements)
}

pub fn check_all(_args: &CheckAllArgs, _config: &Config, _data_dir: &Path) -> eyre::Result<()> {
    eyre::bail!("check-all not implemented");
}

/// A common achievement sink that both check and check_all can use
fn process_achievements(achievements: impl Iterator<Item = Achievement>) -> eyre::Result<()> {
    // TODO: Support different output formats
    for achievement in achievements {
        println!("{achievement:?}");
    }
    Ok(())
}
