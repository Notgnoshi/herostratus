use herostratus::config::Config;
use herostratus_tests::cmd::{CommandExt, TestHarness};
use herostratus_tests::fixtures::repository::Builder;

const NOREPLY_NAME: &str = "testuser";
const NOREPLY_EMAIL: &str = "12345+testuser@users.noreply.github.com";
const CANONICAL_NAME: &str = "Test User";
const CANONICAL_EMAIL: &str = "testuser@example.com";
const MAILMAP_CONTENT: &str =
    "Test User <testuser@example.com> <12345+testuser@users.noreply.github.com>\n";

#[test]
fn test_bare_repo_reads_mailmap_from_head() {
    let temp_repo = Builder::new()
        .commit("Add mailmap")
        .file(".mailmap", MAILMAP_CONTENT.as_bytes())
        .build()
        .unwrap();

    // Add an empty commit by noreply email. This inherits the tree from the previous commit
    // (still has .mailmap), so it's an empty diff and H5 matches. HEAD points to this commit,
    // so `HEAD:.mailmap` is available for bare repos via open_mailmap().
    temp_repo
        .commit("Empty commit by noreply")
        .author(NOREPLY_NAME, NOREPLY_EMAIL)
        .create()
        .unwrap();

    let h = TestHarness::new();
    h.write_config(&Config::default().disable("all").enable("H5-empty-commit"));
    let mut cmd = h.command();
    cmd.arg("check").arg(temp_repo.path()).arg("HEAD");

    let output = cmd.captured_output();
    assert!(output.status.success(), "herostratus check failed");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains(&format!("user_name: \"{CANONICAL_NAME}\"")),
        "Expected canonical name in output, got: {stdout}"
    );
    assert!(
        stdout.contains(&format!("user_email: \"{CANONICAL_EMAIL}\"")),
        "Expected canonical email in output, got: {stdout}"
    );
    assert!(
        !stdout.contains(NOREPLY_EMAIL),
        "Output should not contain noreply email, got: {stdout}"
    );
}

#[test]
fn test_non_bare_repo_reads_mailmap_from_worktree() {
    let temp_repo = Builder::new().non_bare().build().unwrap();

    // Write .mailmap to the worktree (filesystem). For non-bare repos, open_mailmap() reads
    // .mailmap from the worktree rather than the object database.
    std::fs::write(temp_repo.path().join(".mailmap"), MAILMAP_CONTENT).unwrap();

    // Add an empty commit by noreply email. Empty tree -> empty tree means H5 matches.
    temp_repo
        .commit("Empty commit by noreply")
        .author(NOREPLY_NAME, NOREPLY_EMAIL)
        .create()
        .unwrap();

    let h = TestHarness::new();
    h.write_config(&Config::default().disable("all").enable("H5-empty-commit"));
    let mut cmd = h.command();
    cmd.arg("check").arg(temp_repo.path()).arg("HEAD");

    let output = cmd.captured_output();
    assert!(output.status.success(), "herostratus check failed");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains(&format!("user_name: \"{CANONICAL_NAME}\"")),
        "Expected canonical name in output, got: {stdout}"
    );
    assert!(
        stdout.contains(&format!("user_email: \"{CANONICAL_EMAIL}\"")),
        "Expected canonical email in output, got: {stdout}"
    );
    assert!(
        !stdout.contains(NOREPLY_EMAIL),
        "Output should not contain noreply email, got: {stdout}"
    );
}
