use herostratus_tests::cmd::{CommandExt, exclude_all_rules_except, herostratus};
use predicates::prelude::*;
use predicates::str;

#[test]
fn h005_empty_commit() {
    let config = exclude_all_rules_except("H5-empty-commit");
    let (mut cmd, _temp) = herostratus(None, Some(config));
    // This test serves two purposes:
    // 1. Use an early tag so this test doesn't have to parse a variable number of commits as the
    //    project grows
    // 2. Ensure we are able to run on tags, branches, and HEAD alike
    cmd.arg("check").arg(".").arg("v0.1.0-rc1");

    let output = cmd.captured_output();
    assert!(output.status.success());

    let assertion = str::contains("2dcecd66c21932043cf127b31218cb67c2b0f0a4");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        assertion.eval(&stdout),
        "Output did not contain hash: {stdout:?}"
    );
}
