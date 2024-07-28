mod common;

use common::CommandExt;
use herostratus::config::{config_path, read_config, Config, RepositoryConfig};

#[test]
#[ignore = "Slow; Performs clone"]
fn required_clone_herostratus() {
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
        url: url.to_string(),
        branch: None,
        ..Default::default()
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
#[ignore = "Slow; Performs clone"]
fn required_clone_herostratus_branch() {
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
        url: url.to_string(),
        branch: Some(String::from("test/fixup")),
        ..Default::default()
    };
    assert_eq!(repo_config, &expected);

    let repo = herostratus::git::clone::find_local_repository(&clone_dir).unwrap();
    let head = repo.head().unwrap();
    let head = head.name().unwrap();
    assert_eq!(head, "refs/heads/test/fixup");
}

#[test]
#[ignore = "Slow; Performs clone; Requires SSH (not available in CI)"]
fn clone_herostratus_ssh() {
    let (mut cmd, temp) = common::herostratus(None);
    let clone_dir = temp
        .as_ref()
        .unwrap()
        .path()
        .join("git")
        .join("Notgnoshi")
        .join("herostratus.git");

    let url = "git@github.com:Notgnoshi/herostratus.git";
    cmd.arg("add").arg(url);

    assert!(!clone_dir.exists());
    assert!(!config_path(temp.as_ref().unwrap().path()).exists());

    let output = cmd.captured_output().unwrap();
    assert!(output.status.success());
    assert!(clone_dir.exists());

    let contents = std::fs::read_to_string(config_path(temp.as_ref().unwrap().path())).unwrap();
    let expected = format!(
        "[repositories.\"herostratus.git\"]\n\
         path = \"{}\"\n\
         url = \"git@github.com:Notgnoshi/herostratus.git\"\n\
        ",
        clone_dir.display()
    );
    assert_eq!(contents, expected);
}

#[test]
fn add_the_same_repo_twice() {
    let (mut cmd1, temp) = common::herostratus(None);
    let (mut cmd2, _) = common::herostratus(Some(temp.as_ref().unwrap().path()));
    let clone_dir = temp
        .as_ref()
        .unwrap()
        .path()
        .join("git")
        .join("Notgnoshi")
        .join("herostratus.git");

    let url1 = "git@github.com:Notgnoshi/herostratus.git";
    cmd1.arg("add").arg(url1).arg("--skip-clone");

    let output1 = cmd1.captured_output().unwrap();
    assert!(output1.status.success());

    let contents = std::fs::read_to_string(config_path(temp.as_ref().unwrap().path())).unwrap();
    let expected = format!(
        "[repositories.\"herostratus.git\"]\n\
         path = \"{}\"\n\
         url = \"git@github.com:Notgnoshi/herostratus.git\"\n\
        ",
        clone_dir.display()
    );
    assert_eq!(contents, expected);

    let url2 = "https://github.com/Notgnoshi/herostratus.git";
    cmd2.arg("add").arg(url2).arg("--skip-clone");
    let output2 = cmd2.captured_output().unwrap();
    assert!(output2.status.success());

    // The URL gets replaced
    let contents = std::fs::read_to_string(config_path(temp.as_ref().unwrap().path())).unwrap();
    let expected = format!(
        "[repositories.\"herostratus.git\"]\n\
         path = \"{}\"\n\
         url = \"https://github.com/Notgnoshi/herostratus.git\"\n\
        ",
        clone_dir.display()
    );
    assert_eq!(contents, expected);
}

#[test]
#[ignore = "Slow; Performs clone;"]
fn required_two_branches_share_one_bare_repo() {
    let (mut cmd1, temp) = common::herostratus(None);
    let (mut cmd2, _) = common::herostratus(Some(temp.as_ref().unwrap().path()));

    let clone_dir = temp
        .as_ref()
        .unwrap()
        .path()
        .join("git")
        .join("Notgnoshi")
        .join("herostratus.git");

    let url = "https://github.com/Notgnoshi/herostratus.git";
    cmd1.arg("add")
        .arg(url)
        .arg("--name")
        .arg("herostratus-1")
        .arg("test/simple");

    let output1 = cmd1.captured_output().unwrap();
    assert!(output1.status.success());

    let contents = std::fs::read_to_string(config_path(temp.as_ref().unwrap().path())).unwrap();
    let expected = format!(
        "[repositories.herostratus-1]\n\
         path = \"{}\"\n\
         branch = \"test/simple\"\n\
         url = \"https://github.com/Notgnoshi/herostratus.git\"\n\
        ",
        clone_dir.display()
    );
    assert_eq!(contents, expected);

    cmd2.arg("add")
        .arg(url)
        .arg("test/fixup")
        .arg("--name")
        .arg("herostratus-2")
        .arg("--skip-clone");

    let output2 = cmd2.captured_output().unwrap();
    assert!(output2.status.success());

    // NOTE: The TOML file doesn't preserve order or comments, so parse the config file, and
    // compare config values
    let config = read_config(temp.as_ref().unwrap().path()).unwrap();
    assert!(config.repositories.contains_key("herostratus-1"));
    assert!(config.repositories.contains_key("herostratus-2"));

    let config1 = &config.repositories["herostratus-1"];
    let config2 = &config.repositories["herostratus-2"];
    assert_eq!(config1.branch.as_deref(), Some("test/simple"));
    assert_eq!(config2.branch.as_deref(), Some("test/fixup"));
    assert_eq!(config1.path, config2.path);
    assert_eq!(config1.url, config2.url);
}
