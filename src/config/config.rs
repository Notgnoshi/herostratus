use std::collections::HashMap;
use std::io::Write;
use std::path::{Path, PathBuf};

use eyre::WrapErr;
use serde::{Deserialize, Serialize};

/// Configuration for each of the repositories that Herostratus processes
#[derive(Debug, Default, Clone, PartialEq, Deserialize, Serialize)]
pub struct Config {
    // Name -> Config pairs (Use HashMap over Vec for prettiness of TOML)
    pub repositories: HashMap<String, RepositoryConfig>,
}

/// Configuration for cloning, fetching, and processing a repository
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct RepositoryConfig {
    pub path: PathBuf,
    pub branch: Option<String>,
    pub remote_url: String,
}

pub fn config_path(data_dir: &Path) -> PathBuf {
    data_dir.join("config.toml")
}

/// Read, or generate default if missing, Herostratus's config file
pub fn read_config(data_dir: &Path) -> eyre::Result<Config> {
    let config_path = config_path(data_dir);
    let config = if !config_path.exists() {
        tracing::info!("'{}' did not exist. Generating ...", config_path.display());
        let config = Config::default();
        write_config(data_dir, &config).wrap_err("Failed to write default config")?;
        config
    } else {
        let contents =
            std::fs::read_to_string(&config_path).wrap_err("Failed to read config file")?;
        toml::from_str(&contents).wrap_err("Failed to parse config file")?
    };

    Ok(config)
}

pub fn serialize_config(config: &Config) -> eyre::Result<String> {
    toml::ser::to_string_pretty(config).wrap_err("Failed to serialize Config as TOML")
}

pub fn write_config(data_dir: &Path, config: &Config) -> eyre::Result<()> {
    let contents = serialize_config(config)?;
    let file = config_path(data_dir);
    let mut file = std::fs::File::create(file).wrap_err("Failed to open config file")?;
    file.write_all(contents.as_bytes())
        .wrap_err("Failed to write to config file")?;

    Ok(())
}
