mod common;

use common::CommandExt;
use predicates::prelude::*;
use predicates::str;

#[test]
fn search_current_repo_for_test_simple_branch() {
    let (mut cmd, _temp) = common::herostratus(None);
    cmd.arg("check").arg(".").arg("origin/test/simple");

    let output = cmd.captured_output().unwrap();
    assert!(output.status.success());
}

#[test]
fn search_current_repo_for_branch_that_does_not_exist() {
    let (mut cmd, _temp) = common::herostratus(None);
    cmd.arg("check")
        .arg(".")
        .arg("origin/test/this-branch-will-never-exist");

    let output = cmd.captured_output().unwrap();
    assert!(!output.status.success());
}

#[test]
fn search_current_repo_for_fixup_commits() {
    let (mut cmd, _temp) = common::herostratus(None);
    cmd.arg("check").arg(".").arg("origin/test/fixup");

    let output = cmd.captured_output().unwrap();
    assert!(output.status.success());

    // These are the three fixup! commits in the test/fixup branch
    let assertion = str::contains("60b480b554dbd5266eec0f2378f72df5170a6702")
        .and(str::contains("a987013884fc7dafbe9eb080d7cbc8625408a85f"))
        .and(str::contains("2721748d8fa0b0cc3302b41733d37e30161eabfd"));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(assertion.eval(&stdout));
}
