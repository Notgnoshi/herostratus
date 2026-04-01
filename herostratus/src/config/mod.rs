#[allow(clippy::module_inception)]
mod config;
mod forge;

pub use config::{
    Config, HTTPS_PASSWORD_ENV, REMOTE_USERNAME_ENV, RepositoryConfig, RulesConfig, config_path,
    deserialize_config, read_config, serialize_config, write_config,
};
pub use forge::infer_commit_url_prefix;
