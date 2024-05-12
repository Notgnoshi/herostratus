use std::path::Path;

use crate::cli::AddArgs;
use crate::git::clone::{clone_repository, get_clone_path};

pub fn add(args: &AddArgs, data_dir: &Path) -> eyre::Result<()> {
    // TODO: Resolve the right clone path from the arguments (including config file?, CLI argument
    // overrides)
    //
    // TODO: Resolve the right reference to process. If possible, only fetch the configured
    // reference(s).

    let clone_path = get_clone_path(data_dir, &args.url)?;
    let _repo = clone_repository(&clone_path, &args.url, None, args.force)?;

    // TODO: Save the configuration parameters to a config file.

    Ok(())
}
