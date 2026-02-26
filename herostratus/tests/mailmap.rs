use herostratus_tests::cmd::{CommandExt, exclude_all_rules_except, herostratus};
use herostratus_tests::fixtures;

const NOREPLY_NAME: &str = "testuser";
const NOREPLY_EMAIL: &str = "12345+testuser@users.noreply.github.com";
const CANONICAL_NAME: &str = "Test User";
const CANONICAL_EMAIL: &str = "testuser@example.com";
const MAILMAP_CONTENT: &str =
    "Test User <testuser@example.com> <12345+testuser@users.noreply.github.com>\n";

#[test]
fn test_bare_repo_reads_mailmap_from_head() {
    let temp_repo = fixtures::repository::bare().unwrap();

    // Add a commit with .mailmap in the tree (authored by default Herostratus identity).
    // This commit has a diff (adds .mailmap) so H5 does NOT match -- that's fine.
    fixtures::repository::add_commit_with_file(
        &temp_repo.repo,
        "Add mailmap",
        ".mailmap",
        MAILMAP_CONTENT.as_bytes(),
    )
    .unwrap();

    // Add an empty commit by noreply email. This inherits the tree from the previous commit
    // (still has .mailmap), so it's an empty diff and H5 matches. HEAD points to this commit,
    // so `HEAD:.mailmap` is available for bare repos via open_mailmap().
    fixtures::repository::add_empty_commit_as(
        &temp_repo.repo,
        "Empty commit by noreply",
        NOREPLY_NAME,
        NOREPLY_EMAIL,
    )
    .unwrap();

    let config = exclude_all_rules_except("H5-empty-commit");
    let (mut cmd, _temp) = herostratus(None, Some(config));
    cmd.arg("check").arg(temp_repo.path()).arg("HEAD");

    let output = cmd.captured_output();
    assert!(output.status.success(), "herostratus check failed");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains(&format!("author_name: \"{CANONICAL_NAME}\"")),
        "Expected canonical name in output, got: {stdout}"
    );
    assert!(
        stdout.contains(&format!("author_email: \"{CANONICAL_EMAIL}\"")),
        "Expected canonical email in output, got: {stdout}"
    );
    assert!(
        !stdout.contains(NOREPLY_EMAIL),
        "Output should not contain noreply email, got: {stdout}"
    );
}

#[test]
fn test_non_bare_repo_reads_mailmap_from_worktree() {
    let temp_repo = fixtures::repository::non_bare().unwrap();

    // Write .mailmap to the worktree (filesystem). For non-bare repos, open_mailmap() reads
    // .mailmap from the worktree rather than the object database.
    std::fs::write(temp_repo.path().join(".mailmap"), MAILMAP_CONTENT).unwrap();

    // Add an empty commit by noreply email. Empty tree -> empty tree means H5 matches.
    fixtures::repository::add_empty_commit_as(
        &temp_repo.repo,
        "Empty commit by noreply",
        NOREPLY_NAME,
        NOREPLY_EMAIL,
    )
    .unwrap();

    let config = exclude_all_rules_except("H5-empty-commit");
    let (mut cmd, _temp) = herostratus(None, Some(config));
    cmd.arg("check").arg(temp_repo.path()).arg("HEAD");

    let output = cmd.captured_output();
    assert!(output.status.success(), "herostratus check failed");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains(&format!("author_name: \"{CANONICAL_NAME}\"")),
        "Expected canonical name in output, got: {stdout}"
    );
    assert!(
        stdout.contains(&format!("author_email: \"{CANONICAL_EMAIL}\"")),
        "Expected canonical email in output, got: {stdout}"
    );
    assert!(
        !stdout.contains(NOREPLY_EMAIL),
        "Output should not contain noreply email, got: {stdout}"
    );
}
