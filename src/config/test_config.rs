use std::collections::HashMap;
use std::path::PathBuf;

use crate::config::{
    config_path, read_config, serialize_config, write_config, Config, RepositoryConfig,
};
use crate::test::fixtures::config::empty;

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
        branch: None,
        remote_url: String::from("git@github.com:Notgnoshi/herostratus.git"),
    };
    repositories.insert(String::from("herostratus"), config);
    let config = Config { repositories };

    let fixture = empty().unwrap();
    write_config(&fixture.data_dir, &config).unwrap();

    let contents = std::fs::read_to_string(config_path(&fixture.data_dir)).unwrap();
    let expected = "[repositories.herostratus]\n\
                    path = \"git/Notgnoshi/herostratus\"\n\
                    remote_url = \"git@github.com:Notgnoshi/herostratus.git\"\n\
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
