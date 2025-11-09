use herostratus_tests::cmd::{CommandExt, herostratus};

#[test]
fn h004_non_unicode() {
    let (mut cmd, _temp) = herostratus(None);
    cmd.arg("check").arg(".").arg("origin/test/non-unicode");

    let output = cmd.captured_output();
    assert!(output.status.success());

    // TODO: This will be a fragile test, but it *feels* like the right way to assert that the
    // expected achievement was generated?
    //
    // Maybe when I get to the JSON output spec in #40, I can define a parser and do smarter tests
    // with the parsed achievements?
    let expected = "Achievement { name: \"But ... How?!\", commit: 0f64af5fd5f51a45943dcd3f8c0fb53b88974aec }\n";
    assert_eq!(&output.stdout, expected.as_bytes());
}
