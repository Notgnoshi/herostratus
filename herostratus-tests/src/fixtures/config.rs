use std::path::PathBuf;

use tempfile::{TempDir, tempdir};

pub struct DataDir {
    pub tempdir: TempDir,
    pub data_dir: PathBuf,
}

pub fn empty() -> eyre::Result<DataDir> {
    let tempdir = tempdir()?;
    let data_dir = tempdir.path().join("herostratus");
    Ok(DataDir { tempdir, data_dir })
}
