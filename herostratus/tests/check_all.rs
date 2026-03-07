use std::path::PathBuf;

use herostratus::config::{RulesConfig, read_config};
use herostratus_tests::cmd::{CommandExt, herostratus};
use herostratus_tests::fixtures::repository::Builder;
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
    let temp_upstream = Builder::new().commit("commit1").build().unwrap();
    let first_commit = temp_upstream.repo.head_id().unwrap();
    let url = format!("file://{}", temp_upstream.tempdir.path().display());

    let (mut add_cmd, temp) = herostratus(None, None);
    let data_dir = temp.as_ref().unwrap().path();
    add_cmd.arg("add").arg(url);
    let output = add_cmd.captured_output();
    assert!(output.status.success());

    // -- Run 1: only H5 enabled, processes 1 commit --
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
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        stdout.contains(&first_commit.to_string()),
        "First commit should grant H5"
    );
    assert!(
        stderr.contains("processing 1 commits"),
        "Run 1 should process 1 commit: {stderr}"
    );

    // -- Run 2: same rules, no new commits -> checkpoint early exit --
    let mut config = read_config(data_dir).unwrap();
    config.rules = Some(RulesConfig {
        exclude: Some(vec!["all".into()]),
        include: Some(vec!["H5-empty-commit".into()]),
        ..Default::default()
    });
    let (mut check2, _) = herostratus(Some(data_dir), Some(config));
    check2.arg("check-all");
    let output = check2.captured_output();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        !stdout.contains(&first_commit.to_string()),
        "No achievements should be re-granted"
    );
    assert!(
        stderr.contains("processing 0 commits"),
        "Run 2 should early-exit and process 0 commits: {stderr}"
    );

    // -- Add a new commit and a new rule --
    let second_commit = temp_upstream.commit("fixup!").create().unwrap();
    let mut config = read_config(data_dir).unwrap();
    config.rules = Some(RulesConfig {
        exclude: Some(vec!["all".into()]),
        include: Some(vec!["H5-empty-commit".into(), "H1-fixup".into()]),
        ..Default::default()
    });

    // -- Run 3: new commit + new rule -> retire H5 at checkpoint, continue with H1 --
    let (mut check3, _) = herostratus(Some(data_dir), Some(config));
    check3.arg("check-all");
    let output = check3.captured_output();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    // H1 fires for the new fixup commit
    assert!(
        stdout.contains(&second_commit.to_string()),
        "Second commit should grant H1: {stdout}"
    );
    // The old commit should not produce achievements (H5 deduped, H1 doesn't match)
    assert!(
        !stdout.contains(&first_commit.to_string()),
        "First commit should not grant anything: {stdout}"
    );
    // New rule H1 must process all commits (retire-and-continue), so 2 commits processed
    assert!(
        stderr.contains("processing 2 commits"),
        "Run 3 should process 2 commits (retire + continue): {stderr}"
    );
}
