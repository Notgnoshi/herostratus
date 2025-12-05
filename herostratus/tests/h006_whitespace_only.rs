use herostratus_tests::cmd::{CommandExt, herostratus};
use predicates::prelude::*;
use predicates::str;

#[test]
fn h006_whitespace_only() {
    let (mut cmd, _temp) = herostratus(None, None);
    cmd.arg("check").arg(".").arg("origin/test/whitespace-only");

    let output = cmd.captured_output();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Initial empty commit
    //
    // TODO: The test says this commit is awarded an achievement, but it's the H5-empty-commit
    // achievement, not the H6-whitespace-only achievement.
    let assertion = str::contains("37f5c446079eff62dcca0c3ada2c6b8786b94d16");
    assert!(
        assertion.eval(&stdout),
        "Output did not contain hash: {stdout:?}"
    );

    // Add empty file
    let assertion = str::contains("0eef8fb603dd80af0997b15832538347bd07c264").not();
    assert!(assertion.eval(&stdout), "Output contained hash: {stdout:?}");

    // Add line to empty file
    let assertion = str::contains("5caf531c4722da09c1c0b6dba1ab12b0f9770813").not();
    assert!(assertion.eval(&stdout), "Output contained hash: {stdout:?}");

    // Add second line to file
    let assertion = str::contains("10d071c0b3900d8f04e95212ce8d2c4eeebd4b1d").not();
    assert!(assertion.eval(&stdout), "Output contained hash: {stdout:?}");

    // Add trailing whitespace
    let assertion = str::contains("8ff39de70dc7b67524d9db9c73192c9d09d9540e");
    assert!(
        assertion.eval(&stdout),
        "Output did not contain hash: {stdout:?}"
    );

    // Add indentation
    let assertion = str::contains("22ac84cc185282be9077da83c0ad3bfd52c1df58");
    assert!(
        assertion.eval(&stdout),
        "Output did not contain hash: {stdout:?}"
    );

    // Remove trailing whitespace
    let assertion = str::contains("7e6c42efee5d236890b65c9dfaaf7b42c2b8754a");
    assert!(
        assertion.eval(&stdout),
        "Output did not contain hash: {stdout:?}"
    );

    // Dedent line
    let assertion = str::contains("862ad9c83d8285ebb2eb738ab02e1a7569a1f44b");
    assert!(
        assertion.eval(&stdout),
        "Output did not contain hash: {stdout:?}"
    );

    // Add non-empty file
    let assertion = str::contains("5e21807f70160a0a4cf625a5c72f4f9ebc327c1b").not();
    assert!(assertion.eval(&stdout), "Output contained hash: {stdout:?}");
}
