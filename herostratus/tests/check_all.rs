use std::path::PathBuf;

use herostratus::config::{RulesConfig, read_config};
use herostratus_tests::cmd::{CommandExt, herostratus};
use herostratus_tests::fixtures;
use predicates::prelude::*;
use predicates::str;

#[test]
fn add_self_and_then_check_all() {
    let self_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .canonicalize()
        .unwrap();
    let self_dir = format!("file://{}", self_dir.display());
    let (mut cmd, temp) = herostratus(None, None);
    cmd.arg("add").arg("--skip-clone").arg(self_dir);

    let output = cmd.captured_output();
    assert!(output.status.success());

    let (mut cmd, _) = herostratus(Some(temp.as_ref().unwrap().path()), None);
    // If 'add' skips the clone, using 'fetch-all' or 'check-all' without '--no-fetch' will clone
    cmd.arg("check-all");
    let output = cmd.captured_output();
    assert!(output.status.success());

    // who knows how many achievements 'HEAD' will have?
    let assertion =
        str::contains("Finalizing rules ...").and(str::contains("achievements after processing"));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(assertion.eval(&stderr));
}

#[test]
fn early_exit_cache() {
    let temp_upstream = fixtures::repository::bare().unwrap();
    let first_commit =
        fixtures::repository::add_empty_commit(&temp_upstream.repo, "commit1").unwrap();
    let url = format!("file://{}", temp_upstream.tempdir.path().display());

    tracing::error!("Adding repository");
    let (mut add_cmd, temp) = herostratus(None, None);
    let data_dir = temp.as_ref().unwrap().path();
    add_cmd.arg("add").arg(url);
    let output = add_cmd.captured_output();
    assert!(output.status.success());

    tracing::error!("Checking all repositories");
    let mut config = read_config(data_dir).unwrap();
    config.rules = Some(RulesConfig {
        exclude: Some(vec!["all".into()]),
        include: Some(vec!["H5-empty-commit".into()]),
        ..Default::default()
    });
    let (mut check1, _) = herostratus(Some(data_dir), Some(config));
    check1.arg("check-all");
    let output = check1.captured_output();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);

    let assertion = str::contains(first_commit.to_string());
    assert!(
        assertion.eval(&stdout),
        "First commit should grant an achievement"
    );

    // Add a new commit, and enable a new rule
    tracing::error!("Adding new commit to remote");
    let second_commit =
        fixtures::repository::add_empty_commit(&temp_upstream.repo, "fixup!").unwrap();
    let mut config = read_config(data_dir).unwrap();
    config.rules = Some(RulesConfig {
        exclude: Some(vec!["all".into()]),
        include: Some(vec!["H5-empty-commit".into(), "H1-fixup".into()]),
        ..Default::default()
    });

    tracing::error!("Checking all repositories");
    let (mut check2, _) = herostratus(Some(data_dir), Some(config));
    check2.arg("check-all");
    let output = check2.captured_output();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);

    // H5 should not be re-granted, and the new Rule doesn't grant an achievement for the first commit
    let assertion = str::contains(first_commit.to_string()).not();
    assert!(
        assertion.eval(&stdout),
        "First commit is not granted an achievement"
    );

    let assertion = str::contains(second_commit.to_string()).count(2);
    assert!(
        assertion.eval(&stdout),
        "Second commit should grant two achievements"
    );
}
