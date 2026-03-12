use herostratus::config::{Config, RepositoryConfig, config_path, read_config};
use herostratus_tests::cmd::{CommandExt, TestHarness};

#[test]
fn test_clone_herostratus() {
    let h = TestHarness::new();
    let data_dir = h.path();

    let expected_bare_repo = data_dir
        .join("git")
        .join("Notgnoshi")
        .join("herostratus.git");

    let url = "https://github.com/Notgnoshi/herostratus.git";
    let mut cmd = h.command();
    cmd.arg("add").arg(url);

    assert!(!data_dir.join("git").exists());
    assert!(!config_path(data_dir).exists());

    let output = cmd.captured_output();
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
        reference: None,
        ..Default::default()
    };
    assert_eq!(repo_config, &expected);

    // Adding the same URL again in the same data_dir succeeds
    let mut cmd = h.command();
    cmd.arg("add").arg(url);

    let output = cmd.captured_output();
    assert!(output.status.success());

    // And it didn't add a second repository to the config
    let actual_config = read_config(data_dir).unwrap();
    assert_eq!(actual_config.repositories.len(), 1);
}

#[test]
fn test_clone_herostratus_branch() {
    let h = TestHarness::new();
    let clone_dir = h
        .path()
        .join("git")
        .join("Notgnoshi")
        .join("herostratus.git");

    let url = "https://github.com/Notgnoshi/herostratus.git";
    let mut cmd = h.command();
    cmd.arg("add").arg(url).arg("test/fixup");

    assert!(!clone_dir.exists());
    assert!(!config_path(h.path()).exists());

    let output = cmd.captured_output();
    assert!(output.status.success());
    assert!(clone_dir.exists());

    let default_config = Config::default();
    let actual_config = read_config(h.path()).unwrap();
    assert_ne!(
        default_config, actual_config,
        "Adding the repo modified the config"
    );
    assert!(actual_config.repositories.contains_key("herostratus.git"));
    let repo_config = &actual_config.repositories["herostratus.git"];
    let expected = RepositoryConfig {
        path: clone_dir.clone(),
        url: url.to_string(),
        reference: Some(String::from("test/fixup")),
        ..Default::default()
    };
    assert_eq!(repo_config, &expected);
}

#[test]
#[cfg_attr(feature = "ci", ignore = "Requires SSH (not available in CI)")]
fn clone_herostratus_ssh() {
    let h = TestHarness::new();
    let clone_dir = h
        .path()
        .join("git")
        .join("Notgnoshi")
        .join("herostratus.git");

    let url = "git@github.com:Notgnoshi/herostratus.git";
    let mut cmd = h.command();
    cmd.arg("add").arg(url);

    assert!(!clone_dir.exists());
    assert!(!config_path(h.path()).exists());

    let output = cmd.captured_output();
    assert!(output.status.success());
    assert!(clone_dir.exists());

    let contents = std::fs::read_to_string(config_path(h.path())).unwrap();
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
    let h = TestHarness::new();
    let data_dir = h.path();
    let clone_dir = data_dir
        .join("git")
        .join("Notgnoshi")
        .join("herostratus.git");

    let url1 = "git@github.com:Notgnoshi/herostratus.git";
    let mut cmd = h.command();
    cmd.arg("add").arg(url1).arg("--skip-clone");

    let output = cmd.captured_output();
    assert!(output.status.success());

    let contents = std::fs::read_to_string(config_path(data_dir)).unwrap();
    let expected = format!(
        "[repositories.\"herostratus.git\"]\n\
         path = \"{}\"\n\
         url = \"git@github.com:Notgnoshi/herostratus.git\"\n\
        ",
        clone_dir.display()
    );
    assert_eq!(contents, expected);

    let url2 = "https://github.com/Notgnoshi/herostratus.git";
    let mut cmd = h.command();
    cmd.arg("add").arg(url2).arg("--skip-clone");
    let output = cmd.captured_output();
    assert!(output.status.success());

    // The URL gets replaced, because the name didn't change
    let contents = std::fs::read_to_string(config_path(data_dir)).unwrap();
    let expected = format!(
        "[repositories.\"herostratus.git\"]\n\
         path = \"{}\"\n\
         url = \"https://github.com/Notgnoshi/herostratus.git\"\n\
        ",
        clone_dir.display()
    );
    assert_eq!(contents, expected);

    let actual_config = read_config(data_dir).unwrap();
    assert_eq!(actual_config.repositories.len(), 1);

    // Adding the same URL again with a different name adds a second instance with the same clone
    // dir
    let url3 = "https://github.com/Notgnoshi/herostratus.git";
    let mut cmd = h.command();
    cmd.arg("add")
        .arg(url3)
        .arg("--skip-clone")
        .arg("--name")
        .arg("unique-name");
    let output = cmd.captured_output();
    assert!(output.status.success());

    let actual_config = read_config(data_dir).unwrap();
    assert_eq!(actual_config.repositories.len(), 2);

    assert_eq!(
        actual_config.repositories["herostratus.git"].path,
        actual_config.repositories["unique-name"].path
    );
}

#[test]
fn test_two_branches_share_one_bare_repo() {
    let h = TestHarness::new();

    let clone_dir = h
        .path()
        .join("git")
        .join("Notgnoshi")
        .join("herostratus.git");

    let url = "https://github.com/Notgnoshi/herostratus.git";
    let mut cmd = h.command();
    cmd.arg("add")
        .arg(url)
        .arg("--name")
        .arg("herostratus-1")
        .arg("test/simple");

    let output = cmd.captured_output();
    assert!(output.status.success());

    let contents = std::fs::read_to_string(config_path(h.path())).unwrap();
    let expected = format!(
        "[repositories.herostratus-1]\n\
         path = \"{}\"\n\
         reference = \"test/simple\"\n\
         url = \"https://github.com/Notgnoshi/herostratus.git\"\n\
        ",
        clone_dir.display()
    );
    assert_eq!(contents, expected);

    let mut cmd = h.command();
    cmd.arg("add")
        .arg(url)
        .arg("test/fixup")
        .arg("--name")
        .arg("herostratus-2");

    let output = cmd.captured_output();
    assert!(output.status.success());

    // NOTE: The TOML file doesn't preserve order or comments, so parse the config file, and
    // compare config values
    let config = read_config(h.path()).unwrap();
    assert!(config.repositories.contains_key("herostratus-1"));
    assert!(config.repositories.contains_key("herostratus-2"));

    let config1 = &config.repositories["herostratus-1"];
    let config2 = &config.repositories["herostratus-2"];
    assert_eq!(config1.reference.as_deref(), Some("test/simple"));
    assert_eq!(config2.reference.as_deref(), Some("test/fixup"));
    assert_eq!(config1.path, config2.path);
    assert_eq!(config1.url, config2.url);
}
