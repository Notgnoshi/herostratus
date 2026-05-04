use herostratus::config::Config;
use herostratus_tests::cmd::{CommandExt, TestHarness, assert_grants};

#[test]
fn h004_non_unicode() {
    let h = TestHarness::new();
    h.write_config(&Config::default().disable("all").enable("H4-non-unicode"));
    let mut cmd = h.command();
    cmd.arg("check").arg(".").arg("origin/test/non-unicode");

    let output = cmd.captured_output();
    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_grants(
        &stdout,
        "0f64af5fd5f51a45943dcd3f8c0fb53b88974aec",
        "But ... How?!",
    );
}
