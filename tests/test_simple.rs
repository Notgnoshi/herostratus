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
