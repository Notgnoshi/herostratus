use std::process::Output;

use predicate::str;
use predicates::prelude::*;

fn capture_output(output: &Output) {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    // Test output capture relies on magic in the print! and println! macros
    print!("{stdout}");
    print!("{stderr}");
}

#[test]
fn search_current_repo_for_test_simple_branch() {
    // TODO: Consider caching this, so it doesn't get so expensive as
    // https://github.com/assert-rs/assert_cmd/issues/6 suggests.
    let mut cmd = assert_cmd::Command::cargo_bin("herostratus").unwrap();
    cmd.arg("check").arg(".").arg("origin/test/simple");

    let output = cmd.output().unwrap();
    capture_output(&output);
    assert!(output.status.success());
}

#[test]
fn search_current_repo_for_branch_that_does_not_exist() {
    let mut cmd = assert_cmd::Command::cargo_bin("herostratus").unwrap();
    cmd.arg("check")
        .arg(".")
        .arg("origin/test/this-branch-will-never-exist");

    let output = cmd.output().unwrap();
    capture_output(&output);
    assert!(!output.status.success());
}

#[test]
fn search_current_repo_for_fixup_commits() {
    let mut cmd = assert_cmd::Command::cargo_bin("herostratus").unwrap();
    cmd.arg("check").arg(".").arg("origin/test/fixup");

    let output = cmd.output().unwrap();
    capture_output(&output);
    assert!(output.status.success());

    let assertion = str::contains("60b480b554dbd5266eec0f2378f72df5170a6702")
        .and(str::contains("a987013884fc7dafbe9eb080d7cbc8625408a85f"))
        .and(str::contains("2721748d8fa0b0cc3302b41733d37e30161eabfd"));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(assertion.eval(&stdout));
}

#[test]
#[ignore = "Slow; performs git clone"]
fn clone_herostratus() {
    let temp = tempfile::tempdir().unwrap();
    let data_dir = temp.path();
    // let data_dir = PathBuf::from("/tmp/herostratus");

    let mut cmd = assert_cmd::Command::cargo_bin("herostratus").unwrap();
    let expected_bare_repo = data_dir
        .join("git")
        .join("Notgnoshi")
        .join("herostratus.git");
    let url = "https://github.com/Notgnoshi/herostratus.git";
    cmd.arg("--data-dir")
        .arg(data_dir)
        .arg("--log-level=DEBUG")
        .arg("add")
        .arg(url);

    assert!(!data_dir.join("git").exists());

    let output = cmd.output().unwrap();
    capture_output(&output);
    assert!(output.status.success());
    assert!(expected_bare_repo.exists());

    // Adding the same URL again fails ...
    let mut cmd = assert_cmd::Command::cargo_bin("herostratus").unwrap();
    cmd.arg("--data-dir")
        .arg(data_dir)
        .arg("--log-level=DEBUG")
        .arg("add")
        .arg(url);

    let output = cmd.output().unwrap();
    capture_output(&output);
    assert!(!output.status.success());

    // ... unless the --force flag is given
    let mut cmd = assert_cmd::Command::cargo_bin("herostratus").unwrap();
    cmd.arg("--data-dir")
        .arg(data_dir)
        .arg("--log-level=DEBUG")
        .arg("add")
        .arg("--force")
        .arg(url);

    let output = cmd.output().unwrap();
    capture_output(&output);
    assert!(output.status.success());
}
