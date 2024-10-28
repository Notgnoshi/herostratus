use std::collections::HashMap;
use std::io::Write;
use std::path::{Path, PathBuf};

use eyre::WrapErr;
use serde::{Deserialize, Serialize};

use crate::rules::{H002Config, H003Config};

/// Configuration for each of the repositories that Herostratus processes
#[derive(Debug, Default, Clone, PartialEq, Deserialize, Serialize)]
pub struct Config {
    // Name -> Config pairs (Use HashMap over Vec for prettiness of TOML)
    pub repositories: HashMap<String, RepositoryConfig>,
    pub rules: Option<RulesConfig>,
}

#[derive(Debug, Default, Clone, PartialEq, Deserialize, Serialize)]
pub struct RulesConfig {
    /// Rules to exclude. Maybe the rule ID (2), human ID (shortest-subject-line), or pretty ID
    /// (H2-shortest-subject-line).
    pub exclude: Option<Vec<String>>,

    // TODO: There's bound to be some kind of serde voodoo to reduce the copy-pasta and effort it
    // takes to add a configuration for a new rule. Or maybe this is better because it's simple?
    pub h2_shortest_subject_line: Option<H002Config>,
    pub h3_longest_subject_line: Option<H003Config>,
}

/// Configuration for cloning, fetching, and processing a repository
#[derive(Debug, Default, Clone, PartialEq, Deserialize, Serialize)]
pub struct RepositoryConfig {
    pub path: PathBuf,
    pub branch: Option<String>,
    pub url: String,

    /// The username to authenticate with.
    ///
    /// If the username cannot be parsed from a clone URL, it will default to 'git'.
    pub remote_username: Option<String>,

    /// The path to the appropriate SSH private key to use for SSH authentication.
    ///
    /// If not set for an SSH clone URL, Herostratus will attempt to use your SSH agent.
    pub ssh_private_key: Option<PathBuf>,

    /// The path to the appropriate SSH public key to use for SSH authentication.
    ///
    /// Often, if a private key is specified, you do not need to specify the public key, as it can
    /// be inferred.
    pub ssh_public_key: Option<PathBuf>,

    /// The SSH key passphrase, if required.
    pub ssh_passphrase: Option<String>,

    /// The password to use to authenticate HTTPS clone URLs.
    ///
    /// If using HTTPS, it's very likely that you will also need to set `remote_username`.
    ///
    /// If not set for an HTTPS clone URL, Herostratus will attempt to use your configured Git
    /// `credential.helper`.
    pub https_password: Option<String>,
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
        deserialize_config(&contents).wrap_err("Failed to deserialize config file")?
    };

    Ok(config)
}

pub fn deserialize_config(contents: &str) -> eyre::Result<Config> {
    toml::from_str(contents).wrap_err("Failed to parse TOML")
}

pub fn serialize_config(config: &Config) -> eyre::Result<String> {
    toml::ser::to_string_pretty(config).wrap_err("Failed to serialize Config as TOML")
}

pub fn write_config(data_dir: &Path, config: &Config) -> eyre::Result<()> {
    let contents = serialize_config(config)?;

    if !data_dir.exists() {
        std::fs::create_dir_all(data_dir).wrap_err("Failed to create application data dir")?;
    }

    let file = config_path(data_dir);
    let mut file = std::fs::File::create(file).wrap_err("Failed to open config file")?;
    file.write_all(contents.as_bytes())
        .wrap_err("Failed to write to config file")?;

    Ok(())
}
