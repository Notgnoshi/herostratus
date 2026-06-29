use std::path::Path;

use herostratus_tests::cmd::{CommandExt, TestHarness};
use herostratus_tests::fixtures::repository::Builder;

#[test]
fn add_and_fetch() {
    // 1. Create an upstream repo
    let temp_upstream_repo = Builder::new().commit("Initial commit").build().unwrap();
    let url = format!("file://{}", temp_upstream_repo.tempdir.path().display());

    // 2. Add it to herostratus, skipping the clone
    let h = TestHarness::new();
    let mut cmd = h.command();
    cmd.arg("add").arg("--skip-clone").arg(url);
    let output = cmd.captured_output();
    assert!(output.status.success());

    // 3. Fetch the repo, which clones it under the hood, since it doesn't already exist
    let mut cmd = h.command();
    cmd.arg("fetch-all");
    let output = cmd.captured_output();
    assert!(output.status.success());

    // 4. Add a commit to the upstream
    temp_upstream_repo.commit("Second commit").create().unwrap();

    // 5. Fetch the new commit
    let mut cmd = h.command();
    cmd.arg("fetch-all");
    let output = cmd.captured_output();
    assert!(output.status.success());
}

/// Simulate an ephemeral CI scenario where the bare repo is wiped between runs, but the data_dir
/// (with checkpoint) persists. After the first run, more than DEFAULT_SHALLOW_DEPTH new commits
/// are added, so the shallow clone does NOT contain the checkpoint commit and DeepeningRevWalk
/// must deepen to reach it. The rule set is unchanged, so the clone is shallow.
#[test]
fn shallow_clone_with_checkpoint_recovery() {
    // 1. Create an upstream repo with a handful of commits
    let mut builder = Builder::new();
    let num_initial_commits: i64 = 5;
    for i in 0..num_initial_commits {
        builder = builder
            .commit(&format!("commit {i}"))
            .time(1_000_000 + i * 100)
            .finish();
    }
    let temp_upstream = builder.build().unwrap();
    let url = format!("file://{}", temp_upstream.tempdir.path().display());

    // 2. Use TestHarness to get a managed data_dir
    let h = TestHarness::new();

    // 3. Add the repo to herostratus config (skip clone; check-all will clone)
    let mut cmd = h.command();
    cmd.arg("add").arg("--skip-clone").arg(&url);
    let output = cmd.captured_output();
    assert!(output.status.success());

    // 4. Run check-all (full clone, processes all initial commits, creates checkpoint)
    let mut cmd = h.command();
    cmd.arg("check-all");
    let output = cmd.captured_output();
    assert!(output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains(&format!("processing {num_initial_commits} commits")),
        "First run should process {num_initial_commits} commits: {stderr}"
    );

    // 5. Verify the checkpoint file exists
    let cache_dir = h.path().join("cache");
    let checkpoint = find_checkpoint(&cache_dir);
    assert!(
        checkpoint.exists(),
        "Checkpoint should exist after first check-all: {}",
        checkpoint.display()
    );

    // 6. Add more than DEFAULT_SHALLOW_DEPTH new commits so the checkpoint commit falls outside
    //    the shallow window and DeepeningRevWalk must deepen at least once to reach it. Derived
    //    from the constant so a future depth bump keeps this test exercising the deepen path
    //    instead of silently fitting inside one shallow clone.
    let num_new_commits: i64 = (herostratus::git::clone::DEFAULT_SHALLOW_DEPTH + 10) as i64;
    for i in 0..num_new_commits {
        temp_upstream
            .commit(&format!("new commit {i}"))
            .time(1_000_000 + (num_initial_commits + i) * 100)
            .create()
            .unwrap();
    }

    // 7. Delete the cloned bare repo on disk (simulate ephemeral CI wipe)
    let git_dir = h.path().join("git");
    assert!(git_dir.exists());
    std::fs::remove_dir_all(&git_dir).unwrap();

    // 8. Run check-all again. This should:
    //    - Shallow clone because the checkpoint covers all current rules
    //    - The checkpoint is older than the shallow window, so DeepeningRevWalk deepens to reach it
    //    - Pipeline processes the new commits and hits the checkpoint
    let mut cmd = h.command();
    cmd.arg("check-all");
    let output = cmd.captured_output();

    assert!(output.status.success(), "Second check-all should succeed");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains(&format!("processing {num_new_commits} commits")),
        "Second run should process only the {num_new_commits} new commits: {stderr}"
    );
}

/// Find the checkpoint.json file under the cache directory.
///
/// The repo name is derived from the URL, so we search for it rather than hard-coding.
fn find_checkpoint(cache_dir: &Path) -> std::path::PathBuf {
    if !cache_dir.exists() {
        panic!("Cache directory does not exist: {}", cache_dir.display());
    }

    // Walk the cache directory tree to find checkpoint.json
    fn find_recursive(dir: &Path) -> Option<std::path::PathBuf> {
        for entry in std::fs::read_dir(dir).ok()? {
            let entry = entry.ok()?;
            let path = entry.path();
            if path.is_dir() {
                if let Some(found) = find_recursive(&path) {
                    return Some(found);
                }
            } else if path
                .file_name()
                .map(|n| n == "checkpoint.json")
                .unwrap_or(false)
            {
                return Some(path);
            }
        }
        None
    }

    find_recursive(cache_dir)
        .unwrap_or_else(|| panic!("No checkpoint.json found under {}", cache_dir.display()))
}
