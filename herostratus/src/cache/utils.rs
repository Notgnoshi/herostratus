use std::path::{Path, PathBuf};

/// A file-backed cache utility to facilitate loading and saving caches of different types to disk
#[derive(Default)]
pub struct JsonFileCache<T>
where
    T: Default + serde::Serialize + for<'de> serde::Deserialize<'de>,
{
    path: Option<PathBuf>,
    pub data: T,
}

// TODO: Consider using Deref / DerefMut / AsRef / AsMut to make the JsonFileCache wrapper more transparent?

impl<T> JsonFileCache<T>
where
    T: Default + serde::Serialize + for<'de> serde::Deserialize<'de>,
{
    /// Create a new in-memory cache that won't be backed by a file on-disk
    pub fn in_memory() -> Self {
        Self::default()
    }

    pub fn new_in<P: AsRef<Path>>(path: P, data: T) -> Self {
        Self {
            path: Some(path.as_ref().to_path_buf()),
            data,
        }
    }

    /// Load the cache from the given file path, or initialize a new cache if the file does not
    /// exist
    pub fn load<P: AsRef<Path>>(path: P) -> eyre::Result<Self> {
        let path = path.as_ref().to_path_buf();

        let data = if path.exists() {
            let contents = std::fs::read_to_string(&path)?;
            let data: T = serde_json::from_str(&contents)?;
            tracing::debug!(
                "Loaded cache ({} bytes) from '{path:?}' ...",
                contents.len()
            );
            data
        } else {
            tracing::debug!("Initializing new cache from '{path:?}' ...");
            Default::default()
        };

        Ok(Self {
            path: Some(path),
            data,
        })
    }

    /// Save the cache to disk, if it is backed by a file
    pub fn save(&self) -> eyre::Result<()> {
        let Some(path) = &self.path else {
            return Ok(());
        };
        if let Some(parent) = path.parent()
            && !parent.exists()
        {
            std::fs::create_dir_all(parent)?;
        }

        let contents = serde_json::to_string(&self.data)?;
        tracing::debug!("Writing cache ({} bytes) to '{path:?}' ...", contents.len());
        std::fs::write(path, contents)?;
        Ok(())
    }
}
