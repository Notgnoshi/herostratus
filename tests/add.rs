mod common;

use common::CommandExt;
use herostratus::config::{config_path, read_config, Config, RepositoryConfig};

#[test]
#[ignore = "Slow; performs git clone"]
fn clone_herostratus() {
    let (mut cmd, temp) = common::herostratus(None);
    let data_dir = temp.as_ref().unwrap().path();

    let expected_bare_repo = data_dir
        .join("git")
        .join("Notgnoshi")
        .join("herostratus.git");

    let url = "https://github.com/Notgnoshi/herostratus.git";
    cmd.arg("add").arg(url);

    assert!(!data_dir.join("git").exists());
    assert!(!config_path(data_dir).exists());

    let output = cmd.captured_output().unwrap();
    assert!(output.status.success());
    assert!(expected_bare_repo.exists());

    let default_config = Config::default();
    let actual_config = read_config(data_dir).unwrap();
    assert_ne!(
        default_config, actual_config,
        "Adding the repo modified the config"
    );
    assert!(actual_config.repositories.contains_key("herostratus.git"));
    let repo_config = &actual_config.repositories["herostratus.git"];
    let expected = RepositoryConfig {
        path: expected_bare_repo,
        remote_url: url.to_string(),
        branch: None,
    };
    assert_eq!(repo_config, &expected);

    // Adding the same URL again in the same data_dir fails ...
    let (mut cmd, _temp) = common::herostratus(Some(data_dir));
    cmd.arg("add").arg(url);

    let output = cmd.captured_output().unwrap();
    assert!(!output.status.success());

    // ... unless the --force flag is given
    let (mut cmd, _temp) = common::herostratus(Some(data_dir));
    cmd.arg("add").arg("--force").arg(url);

    let output = cmd.captured_output().unwrap();
    assert!(output.status.success());
}

#[test]
#[ignore = "Slow; performs git clone"]
fn clone_herostratus_branch() {
    let (mut cmd, temp) = common::herostratus(None);
    let clone_dir = temp
        .as_ref()
        .unwrap()
        .path()
        .join("git")
        .join("Notgnoshi")
        .join("herostratus.git");

    let url = "https://github.com/Notgnoshi/herostratus.git";
    cmd.arg("add").arg(url).arg("test/fixup");

    assert!(!clone_dir.exists());
    assert!(!config_path(temp.as_ref().unwrap().path()).exists());

    let output = cmd.captured_output().unwrap();
    assert!(output.status.success());
    assert!(clone_dir.exists());

    let default_config = Config::default();
    let actual_config = read_config(temp.as_ref().unwrap().path()).unwrap();
    assert_ne!(
        default_config, actual_config,
        "Adding the repo modified the config"
    );
    assert!(actual_config.repositories.contains_key("herostratus.git"));
    let repo_config = &actual_config.repositories["herostratus.git"];
    let expected = RepositoryConfig {
        path: clone_dir.clone(),
        remote_url: url.to_string(),
        branch: Some(String::from("test/fixup")),
    };
    assert_eq!(repo_config, &expected);

    let repo = herostratus::git::clone::find_local_repository(&clone_dir).unwrap();
    let head = repo.head().unwrap();
    let head = head.name().unwrap();
    assert_eq!(head, "refs/heads/test/fixup");
}
