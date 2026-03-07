use herostratus_tests::cmd::{CommandExt, exclude_all_rules_except, herostratus};

/// The quine commit's message contains its own hash prefix, which should NOT trigger
/// fortune-teller (it's a self-match, not a prediction of a future commit).
#[test]
fn h013_fortune_teller_no_self_match() {
    let config = exclude_all_rules_except("H13-fortune-teller");
    let (mut cmd, _temp) = herostratus(None, Some(config));
    cmd.arg("check").arg(".").arg("origin/test/quine");

    let output = cmd.captured_output();
    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stdout.contains("fortune-teller"),
        "Quine commit should not trigger fortune-teller (self-match excluded): {stdout:?}"
    );
}
