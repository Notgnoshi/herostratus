use herostratus::config::Config;
use herostratus_tests::cmd::{CommandExt, TestHarness};
use predicates::prelude::*;
use predicates::str;

/// The second quine commit (0e0d4d0a) has "Hash: 0e0d4d0" in its message, and the fortune-teller
/// commit (0e0d4d0c) has a hash that starts with "0e0d4d0" -- matching the prediction. The
/// fortune-teller rule should grant an achievement to the author of the predicting commit.
#[test]
fn h013_fortune_teller() {
    let h = TestHarness::new();
    h.write_config(
        &Config::default()
            .disable("all")
            .enable("H13-fortune-teller"),
    );
    let mut cmd = h.command();
    cmd.arg("check").arg(".").arg("origin/test/quine");

    let output = cmd.captured_output();
    assert!(output.status.success());

    // The predicting commit (0e0d4d0a) should be granted the achievement
    let assertion = str::contains("0e0d4d0a3c8ae4d09761790162414bfc22010d7f");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        assertion.eval(&stdout),
        "Output did not contain predicting commit hash: {stdout:?}"
    );
}
