use herostratus_tests::cmd::{CommandExt, exclude_all_rules_except, herostratus};
use predicates::prelude::*;
use predicates::str;

#[test]
fn h004_non_unicode() {
    let config = exclude_all_rules_except("H4-non-unicode");
    let (mut cmd, _temp) = herostratus(None, Some(config));
    cmd.arg("check").arg(".").arg("origin/test/non-unicode");

    let output = cmd.captured_output();
    assert!(output.status.success());

    let assertion = str::contains("0f64af5fd5f51a45943dcd3f8c0fb53b88974aec");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        assertion.eval(&stdout),
        "Output did not contain hash: {stdout:?}"
    );
}
