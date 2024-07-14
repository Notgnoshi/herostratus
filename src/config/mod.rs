#[allow(clippy::module_inception)]
mod config;
#[cfg(test)]
mod test_config;

pub use config::{
    config_path, read_config, serialize_config, write_config, Config, RepositoryConfig,
};
