use herostratus_tests::cmd::{CommandExt, exclude_all_rules_except, herostratus};
use predicates::prelude::*;
use predicates::str;

#[test]
fn h012_quine_commit() {
    let config = exclude_all_rules_except("H12-quine-commit");
    let (mut cmd, _temp) = herostratus(None, Some(config));
    cmd.arg("check").arg(".").arg("origin/test/quine");

    let output = cmd.captured_output();
    assert!(output.status.success());

    // The quine commit has hash 588b41b6e983... and its message contains "588b41b6e9" (10 chars)
    let assertion = str::contains("588b41b6e983c393df17689d7659145fbce16fa9");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        assertion.eval(&stdout),
        "Output did not contain quine commit hash: {stdout:?}"
    );
}
