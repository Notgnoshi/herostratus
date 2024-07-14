use std::path::PathBuf;

use tempfile::{tempdir, TempDir};

pub struct DataDir {
    pub tempdir: TempDir,
    pub data_dir: PathBuf,
}

pub fn empty() -> eyre::Result<DataDir> {
    let tempdir = tempdir()?;
    let data_dir = tempdir.path().to_path_buf();
    Ok(DataDir { tempdir, data_dir })
}
