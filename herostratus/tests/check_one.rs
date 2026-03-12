use std::collections::HashMap;

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

/// check-one fetches new upstream commits by default
///
/// 1. Create an upstream repo, add it, and establish a checkpoint
/// 2. Add a new commit to upstream
/// 3. check-one (default, fetches) sees the new commit
/// 4. check-one --no-fetch after another upstream commit does NOT see it
#[test]
fn check_one_fetches_by_default() {
    let upstream = Builder::new().commit("initial").build().unwrap();
    let url = format!("file://{}", upstream.tempdir.path().display());

    let h = TestHarness::new();
    let mut cmd = h.command();
    cmd.arg("add").arg("--name").arg("fetchy").arg(&url);
    let output = cmd.captured_output();
    assert!(output.status.success());

    // Establish a checkpoint so the initial commit is already processed
    h.update_config(|c| c.disable("all").enable("H5-empty-commit"));
    let mut cmd = h.command();
    cmd.arg("check-one").arg("--no-fetch").arg("fetchy");
    let output = cmd.captured_output();
    assert!(output.status.success());

    // Add a new commit to upstream (new author so H5 fires again)
    let new_commit = upstream
        .commit("second")
        .author("Alice", "alice@example.com")
        .create()
        .unwrap();

    // check-one without --no-fetch should fetch and process the new commit
    let mut cmd = h.command();
    cmd.arg("check-one").arg("fetchy");
    let output = cmd.captured_output();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains(&new_commit.to_string()),
        "check-one should fetch and grant H5 for new commit {new_commit}: {stdout}"
    );

    // Add another commit to upstream
    let missed_commit = upstream
        .commit("third")
        .author("Bob", "bob@example.com")
        .create()
        .unwrap();

    // check-one --no-fetch should NOT see the latest upstream commit
    let mut cmd = h.command();
    cmd.arg("check-one").arg("--no-fetch").arg("fetchy");
    let output = cmd.captured_output();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stdout.contains(&missed_commit.to_string()),
        "check-one --no-fetch should not see unfetched commit {missed_commit}: {stdout}"
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

/// check-one writes an achievements.csv catalog to the export directory containing only the
/// enabled rules, sorted by ID.
#[test]
fn exports_achievements_csv_with_enabled_rules() {
    let upstream = Builder::new().commit("initial").build().unwrap();
    let url = format!("file://{}", upstream.tempdir.path().display());

    let h = TestHarness::new();
    let mut cmd = h.command();
    cmd.arg("add").arg("--name").arg("export-test").arg(&url);
    let output = cmd.captured_output();
    assert!(output.status.success());

    // Enable only H1 (fixup) and H5 (empty-commit)
    h.update_config(|c| {
        c.disable("all")
            .enable("H1-fixup")
            .enable("H5-empty-commit")
    });

    let mut cmd = h.command();
    cmd.arg("check-one").arg("--no-fetch").arg("export-test");
    let output = cmd.captured_output();
    assert!(
        output.status.success(),
        "check-one failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    // Verify the catalog CSV was written
    let csv_path = h.path().join("export/achievements.csv");
    assert!(csv_path.exists(), "achievements.csv should be created");

    let mut reader = csv::Reader::from_path(&csv_path).unwrap();
    let rows: Vec<csv::StringRecord> = reader.records().map(|r| r.unwrap()).collect();

    // Should contain exactly the 2 enabled rules
    assert_eq!(
        rows.len(),
        2,
        "expected 2 rows (H1 + H5), got {}: {:?}",
        rows.len(),
        rows
    );

    // Rows should be sorted by ID, H1 first then H5
    assert_eq!(&rows[0][0], "1", "first row should be H1");
    assert_eq!(&rows[0][1], "fixup");
    assert_eq!(&rows[1][0], "5", "second row should be H5");
    assert_eq!(&rows[1][1], "empty-commit");
}

/// check-all exports a repositories.csv with per-repo metadata including commit URL prefixes
/// inferred from URLs or explicitly configured.
#[test]
fn exports_repositories_csv_with_commit_url_prefixes() {
    // All repos share the same underlying git repo (content doesn't matter for this test)
    let upstream = Builder::new().commit("initial").build().unwrap();
    let repo_path = upstream.tempdir.path();
    let file_url = format!("file://{}", repo_path.display());

    let h = TestHarness::new();

    // github-repo: inferred GitHub prefix
    let mut cmd = h.command();
    cmd.arg("add")
        .arg("--name")
        .arg("github-repo")
        .arg("--skip-clone")
        .arg("--path")
        .arg(repo_path)
        .arg("https://github.com/owner/repo.git");
    let output = cmd.captured_output();
    assert!(output.status.success());

    // gitlab-repo: inferred GitLab prefix
    let mut cmd = h.command();
    cmd.arg("add")
        .arg("--name")
        .arg("gitlab-repo")
        .arg("--skip-clone")
        .arg("--path")
        .arg(repo_path)
        .arg("https://gitlab.example.com/owner/repo.git");
    let output = cmd.captured_output();
    assert!(output.status.success());

    // explicit-prefix: explicit commit URL prefix
    let mut cmd = h.command();
    cmd.arg("add")
        .arg("--name")
        .arg("explicit-prefix")
        .arg("--skip-clone")
        .arg("--path")
        .arg(repo_path)
        .arg("--commit-url-prefix")
        .arg("https://code.example.com/owner/repo/commit/")
        .arg("https://code.example.com/owner/repo.git");
    let output = cmd.captured_output();
    assert!(output.status.success());

    // local-repo: file:// URL, no prefix inferrable
    let mut cmd = h.command();
    cmd.arg("add")
        .arg("--name")
        .arg("local-repo")
        .arg(&file_url);
    let output = cmd.captured_output();
    assert!(output.status.success());

    // Enable only H5 and run check-all
    h.update_config(|c| c.disable("all").enable("H5-empty-commit"));
    let mut cmd = h.command();
    cmd.arg("check-all").arg("--no-fetch");
    let output = cmd.captured_output();
    assert!(
        output.status.success(),
        "check-all failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    // Read and verify repositories.csv
    let csv_path = h.path().join("export/repositories.csv");
    assert!(csv_path.exists(), "repositories.csv should be created");

    let mut reader = csv::Reader::from_path(&csv_path).unwrap();
    let rows: HashMap<String, csv::StringRecord> = reader
        .records()
        .map(|r| {
            let r = r.unwrap();
            (r[0].to_string(), r)
        })
        .collect();

    assert_eq!(rows.len(), 4, "expected 4 repository rows: {rows:?}");

    // Verify commit URL prefixes
    assert_eq!(
        &rows["github-repo"][2],
        "https://github.com/owner/repo/commit/"
    );
    assert_eq!(
        &rows["gitlab-repo"][2],
        "https://gitlab.example.com/owner/repo/-/commit/"
    );
    assert_eq!(
        &rows["explicit-prefix"][2],
        "https://code.example.com/owner/repo/commit/"
    );
    assert_eq!(&rows["local-repo"][2], "");

    // All should have commits_checked=1, ref=HEAD, non-empty last_checked
    for (name, row) in &rows {
        assert_eq!(&row[3], "HEAD", "{name} should have ref=HEAD");
        assert_eq!(&row[4], "1", "{name} should have commits_checked=1");
        assert!(
            !row[5].is_empty(),
            "{name} should have non-empty last_checked"
        );
    }
}
