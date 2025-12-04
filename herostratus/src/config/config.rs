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
    /// Rules to exclude.
    ///
    /// May be the rule ID (2), human ID (shortest-subject-line), pretty ID
    /// (H2-shortest-subject-line), or the string "all" to exclude all rules.
    pub exclude: Option<Vec<String>>,

    /// Rules to include. Applied *after* `rules.exclude`.
    ///
    /// May be the rule ID, human ID, or pretty ID. You would use this to re-include a rule that
    /// was excluded via `rules.exclude = "all"`.
    pub include: Option<Vec<String>>,

    // TODO: There's bound to be some kind of serde voodoo to reduce the copy-pasta and effort it
    // takes to add a configuration for a new rule. Or maybe this is better because it's simple?
    pub h2_shortest_subject_line: Option<H002Config>,
    pub h3_longest_subject_line: Option<H003Config>,
}

/// Configuration for cloning, fetching, and processing a repository
#[derive(Debug, Default, Clone, PartialEq, Deserialize, Serialize)]
pub struct RepositoryConfig {
    pub path: PathBuf,
    pub reference: Option<String>,
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

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::path::PathBuf;

    use herostratus_tests::fixtures::config::empty;

    use super::*;

    #[test]
    fn default_config_toml_contents() {
        let default = Config::default();
        let contents = serialize_config(&default).unwrap();
        let expected = "[repositories]\n";
        assert_eq!(contents, expected);
    }

    #[test]
    fn read_write_config() {
        let mut repositories = HashMap::new();
        let config = RepositoryConfig {
            path: PathBuf::from("git/Notgnoshi/herostratus"),
            reference: None,
            url: String::from("git@github.com:Notgnoshi/herostratus.git"),
            ..Default::default()
        };
        repositories.insert(String::from("herostratus"), config);
        let config = Config {
            repositories,
            ..Default::default()
        };

        let fixture = empty().unwrap();
        write_config(&fixture.data_dir, &config).unwrap();

        let contents = std::fs::read_to_string(config_path(&fixture.data_dir)).unwrap();
        let expected = "[repositories.herostratus]\n\
                    path = \"git/Notgnoshi/herostratus\"\n\
                    url = \"git@github.com:Notgnoshi/herostratus.git\"\n\
                   ";
        assert_eq!(contents, expected);

        let read_config = read_config(&fixture.data_dir).unwrap();
        assert_eq!(read_config, config);
    }

    #[test]
    fn generates_default_config_if_missing() {
        let fixture = empty().unwrap();
        let config_file = config_path(&fixture.data_dir);
        assert!(!config_file.exists());

        let config = read_config(&fixture.data_dir).unwrap();
        let default_config = Config::default();
        assert_eq!(config, default_config);
    }

    #[test]
    fn config_exclude_rules() {
        let config_toml = "[repositories.herostratus]\n\
                       path = \"git/Notgnoshi/herostratus\"\n\
                       url = \"git@github.com:Notgnoshi/herostratus.git\"\n\
                       [rules]\n\
                       exclude = [\"H4-non-unicode\"]\n\
                      ";

        let config = deserialize_config(config_toml).unwrap();
        assert_eq!(config.rules.unwrap().exclude.unwrap(), ["H4-non-unicode"]);
    }

    #[test]
    fn rule_specific_config() {
        let config_toml = "[repositories.herostratus]\n\
                       path = \"git/Notgnoshi/herostratus\"\n\
                       url = \"git@github.com:Notgnoshi/herostratus.git\"\n\
                       [rules]\n\
                       h2_shortest_subject_line.length_threshold = 80\n\
                      ";

        let config = deserialize_config(config_toml).unwrap();
        assert_eq!(
            config
                .rules
                .unwrap()
                .h2_shortest_subject_line
                .unwrap()
                .length_threshold,
            80
        );

        let config_toml = "[repositories.herostratus]\n\
                       path = \"git/Notgnoshi/herostratus\"\n\
                       url = \"git@github.com:Notgnoshi/herostratus.git\"\n\
                       [rules.h2_shortest_subject_line]\n\
                       length_threshold = 80\n\
                      ";

        let config = deserialize_config(config_toml).unwrap();
        assert_eq!(
            config
                .rules
                .unwrap()
                .h2_shortest_subject_line
                .unwrap()
                .length_threshold,
            80
        );
    }
}
