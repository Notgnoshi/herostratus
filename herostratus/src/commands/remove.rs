use std::path::Path;

use crate::cli::RemoveArgs;
use crate::config::Config;

pub fn remove(_args: &RemoveArgs, _config: &mut Config, _data_dir: &Path) -> eyre::Result<()> {
    eyre::bail!("remove not implemented");
}
