use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::cache::EntryCache;

#[derive(Debug)]
pub struct GlobalCache {
    cache_path: Option<PathBuf>,
    entry_caches: HashMap<String, EntryCache>,
}

impl GlobalCache {
    /// Create a new in-memory cache
    pub fn in_memory() -> Self {
        Self {
            cache_path: None,
            entry_caches: HashMap::new(),
        }
    }

    /// Load the cache from the given application data directory
    pub fn from_data_dir<P: AsRef<Path>>(data_dir: P) -> eyre::Result<Self> {
        let cache_path = data_dir.as_ref().join("cache.json");
        if cache_path.exists() {
            Self::from_file(cache_path)
        } else {
            Ok(Self {
                cache_path: Some(cache_path),
                entry_caches: HashMap::new(),
            })
        }
    }

    /// Read the cache from the given `cache.json` file
    fn from_file<P: AsRef<Path>>(path: P) -> eyre::Result<Self> {
        let path = path.as_ref();
        let contents = std::fs::read_to_string(path)?;
        let entry_caches: HashMap<String, EntryCache> = serde_json::from_str(&contents)?;
        tracing::debug!(
            "Loaded cache from '{path:?}' with {} entries ...",
            entry_caches.len()
        );
        Ok(Self {
            cache_path: Some(path.to_path_buf()),
            entry_caches,
        })
    }

    /// Write the cache to the given `cache.json` file
    fn to_file<P: AsRef<Path>>(&self, path: P) -> eyre::Result<()> {
        let path = path.as_ref();
        tracing::debug!(
            "Writing cache with {} entries to '{path:?}' ...",
            self.entry_caches.len()
        );
        let contents = serde_json::to_string_pretty(&self.entry_caches)?;
        std::fs::write(path, contents)?;

        Ok(())
    }

    /// Get the cache for a specific repository and reference pair
    ///
    /// NOTE: `name` is the same name as in the
    /// [Config::repositories](crate::config::Config::repositories) map.
    pub fn get_entry_cache(&mut self, name: &str, reference: &str) -> &mut EntryCache {
        let key = format!("{}#{}", name, reference);
        self.entry_caches.entry(key).or_default()
    }
}

impl Drop for GlobalCache {
    fn drop(&mut self) {
        if let Some(path) = &self.cache_path {
            let outcome = self.to_file(path);
            // Do not panic in Drop, as that results in A Bad Time.
            let _ = outcome.inspect_err(|e| {
                tracing::error!("Failed to write cache '{path:?}' to disk: {e:?}");
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::*;

    #[test]
    fn test_read_write_cache() {
        let tmp = tempdir().unwrap();
        let mut cache = GlobalCache::from_data_dir(tmp.path()).unwrap();
        assert!(cache.entry_caches.is_empty());

        let _ = cache.get_entry_cache("NAME", "BRANCH1");
        let _ = cache.get_entry_cache("NAME", "BRANCH2");
        assert_eq!(cache.entry_caches.len(), 2);
        drop(cache);

        let cache = GlobalCache::from_data_dir(tmp.path()).unwrap();
        println!("{cache:?}");
        assert_eq!(cache.entry_caches.len(), 2);
    }
}
