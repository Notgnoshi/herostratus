use predicate::str;
use predicates::prelude::*;

#[test]
fn search_current_repo_for_test_simple_branch() {
    // TODO: Consider caching this, so it doesn't get so expensive as
    // https://github.com/assert-rs/assert_cmd/issues/6 suggests.
    let mut cmd = assert_cmd::Command::cargo_bin("herostratus").unwrap();
    cmd.arg(".").arg("origin/test/simple");

    cmd.assert().success();
}

#[test]
fn search_current_repo_for_branch_that_does_not_exist() {
    let mut cmd = assert_cmd::Command::cargo_bin("herostratus").unwrap();
    cmd.arg(".").arg("origin/test/this-branch-will-never-exist");

    cmd.assert().failure();
}

#[test]
fn search_current_repo_for_fixup_commits() {
    let mut cmd = assert_cmd::Command::cargo_bin("herostratus").unwrap();
    cmd.arg(".").arg("origin/test/fixup");

    cmd.assert()
        .stdout(
            str::contains("60b480b554dbd5266eec0f2378f72df5170a6702")
                .and(str::contains("a987013884fc7dafbe9eb080d7cbc8625408a85f"))
                .and(str::contains("2721748d8fa0b0cc3302b41733d37e30161eabfd")),
        )
        .success();
}
