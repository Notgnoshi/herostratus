use std::path::Path;

use crate::cli::AddArgs;
use crate::config::Config;
use crate::git::clone::{clone_repository, get_clone_path};

pub fn add(args: &AddArgs, config: &mut Config, data_dir: &Path) -> eyre::Result<()> {
    // TODO: Resolve the right clone path in the following priority order
    // 1. args.path
    // 2. configuration file
    // 3. get_clone_path
    let clone_path = get_clone_path(data_dir, &args.url)?;
    let _repo = clone_repository(&clone_path, &args.url, args.branch.as_deref(), args.force)?;

    // TODO: Save the configuration parameters to a config file.

    Ok(())
}
