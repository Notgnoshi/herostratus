use herostratus::config::Config;
use herostratus::rules::H012Config;
use herostratus_tests::cmd::{CommandExt, herostratus};
use predicates::prelude::*;
use predicates::str;

#[test]
fn h012_quine_commit() {
    // Set min_matched_chars = 10 so only the original quine commit (588b41b6e9, 10 chars)
    // triggers, not the newer 7-char quine added for fortune-teller testing.
    let mut config = Config::default().disable("all").enable("H12-quine-commit");
    config.rules.as_mut().unwrap().h12_quine_commit = Some(H012Config {
        min_matched_chars: 10,
    });
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
