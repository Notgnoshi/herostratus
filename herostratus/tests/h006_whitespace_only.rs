use herostratus_tests::cmd::{CommandExt, exclude_all_rules_except, herostratus};
use predicates::prelude::*;
use predicates::str;

/// Run the CLI on a single commit (--depth=1 with the commit as the reference) and return whether
/// it granted an achievement for that commit.
fn granted_for(commit: &str) -> bool {
    let config = exclude_all_rules_except("H6-whitespace-only");
    let (mut cmd, _temp) = herostratus(None, Some(config));
    cmd.arg("check").arg(".").arg(commit).arg("--depth=1");

    let output = cmd.captured_output();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    stdout.contains(commit)
}

#[test]
fn h006_whitespace_only() {
    // Commits that should NOT grant H6
    assert!(
        !granted_for("37f5c446079eff62dcca0c3ada2c6b8786b94d16"),
        "Initial empty commit"
    );
    assert!(
        !granted_for("0eef8fb603dd80af0997b15832538347bd07c264"),
        "Add empty file"
    );
    assert!(
        !granted_for("5caf531c4722da09c1c0b6dba1ab12b0f9770813"),
        "Add line to empty file"
    );
    assert!(
        !granted_for("10d071c0b3900d8f04e95212ce8d2c4eeebd4b1d"),
        "Add second line to file"
    );
    assert!(
        !granted_for("5e21807f70160a0a4cf625a5c72f4f9ebc327c1b"),
        "Add non-empty file"
    );

    // Commits that SHOULD grant H6 (each is a whitespace-only modification)
    assert!(
        granted_for("8ff39de70dc7b67524d9db9c73192c9d09d9540e"),
        "Add trailing whitespace"
    );
    assert!(
        granted_for("22ac84cc185282be9077da83c0ad3bfd52c1df58"),
        "Add indentation"
    );
    assert!(
        granted_for("7e6c42efee5d236890b65c9dfaaf7b42c2b8754a"),
        "Remove trailing whitespace"
    );
    assert!(
        granted_for("862ad9c83d8285ebb2eb738ab02e1a7569a1f44b"),
        "Dedent line"
    );
}

/// When the full branch is checked, only one grant should appear (PerUser deduplication).
#[test]
fn h006_whitespace_only_dedup() {
    let config = exclude_all_rules_except("H6-whitespace-only");
    let (mut cmd, _temp) = herostratus(None, Some(config));
    cmd.arg("check").arg(".").arg("origin/test/whitespace-only");

    let output = cmd.captured_output();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);

    let whitespace_hashes = [
        "8ff39de70dc7b67524d9db9c73192c9d09d9540e",
        "22ac84cc185282be9077da83c0ad3bfd52c1df58",
        "7e6c42efee5d236890b65c9dfaaf7b42c2b8754a",
        "862ad9c83d8285ebb2eb738ab02e1a7569a1f44b",
    ];
    let matches: Vec<_> = whitespace_hashes
        .iter()
        .filter(|h| stdout.contains(*h))
        .collect();
    assert_eq!(
        matches.len(),
        1,
        "expected exactly 1 grant after PerUser dedup, got {}: {stdout:?}",
        matches.len()
    );

    // The newest commit in walk order is "Dedent line"
    let assertion = str::contains("862ad9c83d8285ebb2eb738ab02e1a7569a1f44b");
    assert!(
        assertion.eval(&stdout),
        "expected 'Dedent line' to be the granted commit: {stdout:?}"
    );
}
