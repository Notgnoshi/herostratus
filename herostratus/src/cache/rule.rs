use std::path::Path;

use crate::cache::utils::JsonFileCache;

pub type RuleCache = JsonFileCache<serde_json::Value>;

impl RuleCache {
    fn repo_cache_dir<P: AsRef<Path>>(data_dir: P, repo_name: &str) -> std::path::PathBuf {
        data_dir.as_ref().join("cache").join(repo_name)
    }

    fn rule_cache_path<P: AsRef<Path>>(
        data_dir: P,
        repo_name: &str,
        rule_name: &str,
    ) -> std::path::PathBuf {
        Self::repo_cache_dir(data_dir, repo_name).join(format!("rule_{rule_name}.json"))
    }

    pub(crate) fn new_for_rule<P: AsRef<Path>>(
        data_dir: P,
        repo_name: &str,
        rule_name: &str,
        data: serde_json::Value,
    ) -> Self {
        let path = Self::rule_cache_path(data_dir, repo_name, rule_name);
        Self::new_in(path, data)
    }

    pub(crate) fn from_rule_name<P: AsRef<Path>>(
        data_dir: P,
        repo_name: &str,
        rule_name: &str,
    ) -> eyre::Result<Self> {
        let path = Self::rule_cache_path(data_dir, repo_name, rule_name);
        Self::load(path)
    }
}
