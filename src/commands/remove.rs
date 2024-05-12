use std::path::Path;

use crate::cli::RemoveArgs;

pub fn remove(_args: &RemoveArgs, _data_dir: &Path) -> eyre::Result<()> {
    eyre::bail!("remove not implemented");
}
