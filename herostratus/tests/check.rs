use std::path::Path;

use herostratus::git::clone::find_local_repository;
use herostratus_tests::cmd::{CommandExt, exclude_all_rules_except, herostratus};
use predicates::prelude::*;
use predicates::str;

#[test]
fn search_current_repo_for_test_simple_branch() {
    let (mut cmd, _temp) = herostratus(None, None);
    cmd.arg("check").arg(".").arg("origin/test/simple");

    let output = cmd.captured_output();
    assert!(output.status.success());
}

#[test]
fn search_current_repo_for_branch_that_does_not_exist() {
    let (mut cmd, _temp) = herostratus(None, None);
    cmd.arg("check")
        .arg(".")
        .arg("origin/test/this-branch-will-never-exist");

    let output = cmd.captured_output();
    assert!(!output.status.success());
}

#[test]
fn search_depth() {
    let config = exclude_all_rules_except("H1-fixup");
    let (mut cmd, _temp) = herostratus(None, Some(config.clone()));
    // The fixup branch's HEAD is not a fixup commit, but its parent is.
    cmd.arg("check")
        .arg(".")
        .arg("origin/test/fixup")
        .arg("--depth=1");
    let output = cmd.captured_output();
    let stderr = String::from_utf8_lossy(&output.stderr); // herostratus logs to stderr
    assert!(output.status.success());

    let assertion = str::contains("processing 1 commits");
    assert!(assertion.eval(&stderr), "Found != 1 commits");

    let (mut cmd, _temp) = herostratus(None, Some(config));
    cmd.arg("check")
        .arg(".")
        .arg("origin/test/fixup")
        .arg("--depth=2");

    let output = cmd.captured_output();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success());

    let assertion = str::contains("processing 2 commits");
    assert!(assertion.eval(&stderr), "Found != 2 commits");
}

#[test]
fn search_current_repo_for_fixup_commits() {
    let config = exclude_all_rules_except("H1-fixup");
    let (mut cmd, _temp) = herostratus(None, Some(config));
    cmd.arg("check").arg(".").arg("origin/test/fixup");

    let output = cmd.captured_output();
    assert!(output.status.success());

    // H1 (fixup) is PerUser { recurrent: false }, so only the first fixup commit encountered in
    // walk order (newest-first) is granted. The other two by the same author are deduplicated.
    let stdout = String::from_utf8_lossy(&output.stdout);
    let fixup_hashes = [
        "60b480b554dbd5266eec0f2378f72df5170a6702",
        "a987013884fc7dafbe9eb080d7cbc8625408a85f",
        "2721748d8fa0b0cc3302b41733d37e30161eabfd",
    ];
    let matches: Vec<_> = fixup_hashes
        .iter()
        .filter(|h| stdout.contains(*h))
        .collect();
    assert_eq!(
        matches.len(),
        1,
        "expected exactly 1 fixup achievement, got {}: {stdout:?}",
        matches.len()
    );
}

/// Run check on all local **and** remote branches in the herostratus repository
///
/// The application should not crash nor exit with an error on any branch.
#[test]
#[cfg_attr(not(feature = "ci"), ignore = "Slow test, only run in CI")]
fn smoke_test_on_all_own_branches() {
    use std::os::unix::ffi::OsStringExt;

    let path = Path::new(".");
    let repo = find_local_repository(path).unwrap();

    let references = repo.references().unwrap();
    let local_branches = references.local_branches().unwrap();
    let remote_branches = references.remote_branches().unwrap();
    let branches = local_branches.chain(remote_branches);
    for reference in branches {
        let reference = reference.unwrap();
        let name = std::ffi::OsString::from_vec(reference.name().as_bstr().to_vec());

        let (mut cmd, _temp) = herostratus(None, None);
        cmd.arg("check").arg(".").arg(name);

        let output = cmd.captured_output();
        assert!(output.status.success());
    }
}
