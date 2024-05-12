use std::path::Path;

use crate::cli::AddArgs;

pub fn add(_args: &AddArgs, _data_dir: &Path) -> eyre::Result<()> {
    eyre::bail!("add not implemented");
}
