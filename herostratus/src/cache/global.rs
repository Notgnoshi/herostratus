use std::collections::HashMap;
use std::path::Path;

use crate::cache::EntryCache;
use crate::cache::utils::JsonFileCache;

pub type GlobalCache = JsonFileCache<HashMap<String, EntryCache>>;

impl GlobalCache {
    pub fn from_data_dir<P: AsRef<Path>>(data_dir: P) -> eyre::Result<Self> {
        let cache_path = data_dir.as_ref().join("cache.json");
        Self::load(cache_path)
    }

    /// Get the cache for a specific repository
    ///
    /// NOTE: `name` is the same name as in the
    /// [Config::repositories](crate::config::Config::repositories) map, and uniquely identifies a
    /// repository / branch pair.
    pub fn get_entry_cache(&mut self, name: &str) -> &mut EntryCache {
        let key = name.to_string();
        self.data.entry(key).or_default()
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
        assert!(cache.data.is_empty());

        let _ = cache.get_entry_cache("NAME1");
        let _ = cache.get_entry_cache("NAME2");
        let entry = cache.get_entry_cache("NAME1");
        entry.last_processed_rules = vec![418];
        assert_eq!(cache.data.len(), 2);
        cache.save().unwrap();

        let mut cache = GlobalCache::from_data_dir(tmp.path()).unwrap();
        assert_eq!(cache.data.len(), 2);

        let entry = cache.get_entry_cache("NAME1");
        assert_eq!(entry.last_processed_rules, [418]);
    }
}
