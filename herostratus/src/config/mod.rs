#[allow(clippy::module_inception)]
mod config;
mod forge;

pub use config::{
    Config, RepositoryConfig, RulesConfig, config_path, deserialize_config, read_config,
    serialize_config, write_config,
};
pub use forge::infer_commit_url_prefix;
