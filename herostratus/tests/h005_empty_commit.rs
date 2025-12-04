use herostratus_tests::cmd::{CommandExt, herostratus};
use predicates::prelude::*;
use predicates::str;

#[test]
fn h005_empty_commit() {
    let (mut cmd, _temp) = herostratus(None, None);
    // TODO: Support checking tags, and point to an early on tag in the branch?
    cmd.arg("check").arg(".").arg("origin/main");

    let output = cmd.captured_output();
    assert!(output.status.success());

    let assertion = str::contains("2dcecd66c21932043cf127b31218cb67c2b0f0a4");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        assertion.eval(&stdout),
        "Output did not contain hash: {stdout:?}"
    );
}
