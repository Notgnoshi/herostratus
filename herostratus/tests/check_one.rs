use herostratus_tests::cmd::{CommandExt, TestHarness};
use herostratus_tests::fixtures::repository::Builder;

/// check-one processes only the named repository, leaving the other untouched.
///
/// 1. Create two upstream repos, add both with explicit names
/// 2. check-all --no-fetch to establish checkpoints for both
/// 3. Add new commits to both upstreams (with new authors, so H5 can fire again)
/// 4. check-one repo1 -- fetches + checks only repo1
/// 5. check-all on both -- repo1 early-exits (0 commits), repo2 processes its new commit
#[test]
fn check_one_processes_single_repo() {
    // -- Setup: two upstream repos with one commit each --
    let upstream1 = Builder::new().commit("repo1 initial").build().unwrap();
    let upstream2 = Builder::new().commit("repo2 initial").build().unwrap();
    let initial1 = upstream1.repo.head_id().unwrap();
    let initial2 = upstream2.repo.head_id().unwrap();
    let url1 = format!("file://{}", upstream1.tempdir.path().display());
    let url2 = format!("file://{}", upstream2.tempdir.path().display());

    // Add both repos
    let h = TestHarness::new();
    let mut cmd = h.command();
    cmd.arg("add").arg("--name").arg("repo1").arg(&url1);
    let output = cmd.captured_output();
    assert!(output.status.success());

    let mut cmd = h.command();
    cmd.arg("add").arg("--name").arg("repo2").arg(&url2);
    let output = cmd.captured_output();
    assert!(output.status.success());

    // -- Establish checkpoints: check-all --no-fetch with only H5 --
    h.update_config(|c| c.disable("all").enable("H5-empty-commit"));
    let mut cmd = h.command();
    cmd.arg("check-all").arg("--no-fetch");
    let output = cmd.captured_output();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    // Both initial commits should be granted H5 (they're empty commits)
    assert!(
        stdout.contains(&initial1.to_string()),
        "Initial run should grant H5 for repo1's initial commit {initial1}: {stdout}"
    );
    assert!(
        stdout.contains(&initial2.to_string()),
        "Initial run should grant H5 for repo2's initial commit {initial2}: {stdout}"
    );

    // -- Add new commits to both upstreams --
    // Use new authors so H5 (PerUser, non-recurrent) can fire again
    let second1 = upstream1
        .commit("repo1 second")
        .author("Alice", "alice@example.com")
        .create()
        .unwrap();
    let second2 = upstream2
        .commit("repo2 second")
        .author("Bob", "bob@example.com")
        .create()
        .unwrap();

    // -- check-one repo1: should fetch + check only repo1 --
    let mut cmd = h.command();
    cmd.arg("check-one").arg("repo1");
    let output = cmd.captured_output();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);

    // repo1's new commit should be granted H5
    assert!(
        stdout.contains(&second1.to_string()),
        "check-one should grant H5 for repo1's new commit {second1}: {stdout}"
    );
    // repo2's new commit must NOT appear -- check-one only processes repo1
    assert!(
        !stdout.contains(&second2.to_string()),
        "check-one should NOT process repo2's commit {second2}: {stdout}"
    );
    // repo1's initial commit should not be re-granted (checkpoint skip)
    assert!(
        !stdout.contains(&initial1.to_string()),
        "check-one should not re-grant repo1's initial commit {initial1}: {stdout}"
    );

    // -- check-all: repo1 should early-exit, repo2 should process its new commit --
    let mut cmd = h.command();
    cmd.arg("check-all");
    let output = cmd.captured_output();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);

    // repo2's new commit should now be granted (fetched + checked)
    assert!(
        stdout.contains(&second2.to_string()),
        "check-all should grant H5 for repo2's new commit {second2}: {stdout}"
    );
    // repo1's new commit must NOT appear again (already processed by check-one)
    assert!(
        !stdout.contains(&second1.to_string()),
        "check-all should not re-grant repo1's commit {second1}: {stdout}"
    );
}

/// check-one can look up a repository by its remote URL
#[test]
fn check_one_by_url() {
    let upstream = Builder::new().commit("initial").build().unwrap();
    let initial_commit = upstream.repo.head_id().unwrap();
    let url = format!("file://{}", upstream.tempdir.path().display());

    let h = TestHarness::new();
    let mut cmd = h.command();
    cmd.arg("add").arg("--name").arg("myrepo").arg(&url);
    let output = cmd.captured_output();
    assert!(output.status.success());

    // check-one by URL instead of name -- should find and process the repo
    h.update_config(|c| c.disable("all").enable("H5-empty-commit"));
    let mut cmd = h.command();
    cmd.arg("check-one").arg("--no-fetch").arg(&url);
    let output = cmd.captured_output();
    assert!(
        output.status.success(),
        "check-one by URL should succeed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains(&initial_commit.to_string()),
        "check-one by URL should grant H5 for {initial_commit}: {stdout}"
    );
}

/// check-one with an unknown repository name fails with a helpful error
#[test]
fn check_one_unknown_repo() {
    let upstream = Builder::new().commit("initial").build().unwrap();
    let url = format!("file://{}", upstream.tempdir.path().display());

    let h = TestHarness::new();
    let mut cmd = h.command();
    cmd.arg("add").arg("--name").arg("myrepo").arg(&url);
    let output = cmd.captured_output();
    assert!(output.status.success());

    let mut cmd = h.command();
    cmd.arg("check-one").arg("nonexistent");
    let output = cmd.captured_output();
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Repository \"nonexistent\" not found"),
        "Should report the exact repo that was not found: {stderr}"
    );
    assert!(
        stderr.contains("myrepo"),
        "Should list available repo names: {stderr}"
    );
}
