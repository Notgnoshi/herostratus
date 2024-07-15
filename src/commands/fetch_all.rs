use std::path::Path;

use crate::cli::FetchAllArgs;
use crate::config::Config;

pub fn fetch_all(_args: &FetchAllArgs, _config: &Config, _data_dir: &Path) -> eyre::Result<()> {
    eyre::bail!("fetch-all not implemented");
}
