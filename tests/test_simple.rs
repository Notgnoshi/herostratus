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

#[test]
#[ignore = "Slow; performs git clone"]
fn clone_herostratus() {
    let temp = tempfile::tempdir().unwrap();
    let cache_dir = temp.path();
    // let cache_dir = PathBuf::from("/tmp/herostratus");

    let mut cmd = assert_cmd::Command::cargo_bin("herostratus").unwrap();
    let expected_bare_repo = cache_dir
        .join("git")
        .join("Notgnoshi")
        .join("herostratus.git");
    let url = "https://github.com/Notgnoshi/herostratus.git";
    cmd.arg("--cache-dir")
        .arg(cache_dir)
        .arg("--log-level=DEBUG")
        .arg(url)
        // This assumes that the user running these tests has at some point checked out 'main',
        // which is very likely true. But we can't ensure anything about how up-to-date 'main' is.
        .arg("origin/main");

    assert!(!cache_dir.join("git").exists());

    cmd.assert().success();

    assert!(expected_bare_repo.exists());

    // Running the command again should be successful
    let mut cmd = assert_cmd::Command::cargo_bin("herostratus").unwrap();
    cmd.arg("--cache-dir")
        .arg(cache_dir)
        .arg("--log-level=DEBUG")
        .arg(url)
        // This assumes that the user running these tests has at some point checked out 'main',
        // which is very likely true. But we can't ensure anything about how up-to-date 'main' is.
        .arg("origin/main");

    cmd.assert()
        .stderr(str::contains(format!(
            "Found existing {}",
            expected_bare_repo.display()
        )))
        .success();
}
