use std::path::Path;

use crate::cli::FetchAllArgs;

pub fn fetch_all(_args: &FetchAllArgs, _data_dir: &Path) -> eyre::Result<()> {
    eyre::bail!("fetch-all not implemented");
}
