use herostratus_tests::cmd::{CommandExt, TestHarness};

const PRIVATE_REPO_HTTPS_URL: &str = "https://github.com/Notgnoshi/herostratus-private-test.git";

#[test]
#[cfg_attr(not(feature = "ci"), ignore = "Requires CI secrets")]
fn clone_private_repo_https() {
    let pat = std::env::var("HEROSTRATUS_PRIVATE_TEST_PAT")
        .expect("HEROSTRATUS_PRIVATE_TEST_PAT must be set in CI");

    let h = TestHarness::new();
    let clone_dir = h
        .path()
        .join("git")
        .join("Notgnoshi")
        .join("herostratus-private-test.git");

    let mut cmd = h.command();
    cmd.arg("add")
        .arg(PRIVATE_REPO_HTTPS_URL)
        .arg("--remote-username")
        .arg("x-access-token")
        .arg("--https-password")
        .arg(&pat);

    assert!(!clone_dir.exists());

    let output = cmd.captured_output();
    assert!(output.status.success());
    assert!(clone_dir.exists());

    // The password should not appear in the log output
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stderr.contains(&pat),
        "HTTPS password was leaked in stderr"
    );
}
