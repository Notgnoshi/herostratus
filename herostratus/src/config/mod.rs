#[allow(clippy::module_inception)]
mod config;

pub use config::{
    Config, RepositoryConfig, RulesConfig, config_path, deserialize_config, read_config,
    serialize_config, write_config,
};
