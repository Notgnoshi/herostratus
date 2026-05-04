use herostratus::config::Config;
use herostratus_tests::cmd::{CommandExt, TestHarness, assert_grants};
use herostratus_tests::fixtures::repository::Builder;

/// Build a repository with three empty root commits and an octopus merge that joins them all,
/// then run `herostratus check` and verify that the topology rules fire as expected.
///
/// ```text
///   *-.   octopus merge (3 parents)
///   |\ \
///   | | *  Third root  (orphan2)
///   | *    Second root (orphan1)
///   *      Initial commit (main)
/// ```
#[test]
fn three_empty_roots_with_octopus_merge() {
    let temp = Builder::new()
        .commit("Initial commit")
        .time(1_000)
        .build()
        .unwrap();
    let initial = temp.repo.head_id().unwrap().detach();

    temp.create_orphan_branch("orphan1").unwrap();
    let second_root = temp
        .commit("Second root")
        .time(2_000)
        .create()
        .unwrap()
        .detach();

    temp.create_orphan_branch("orphan2").unwrap();
    let third_root = temp
        .commit("Third root")
        .time(3_000)
        .create()
        .unwrap()
        .detach();

    temp.set_branch("main").unwrap();
    let merge = temp
        .merge("orphan1", "octopus")
        .with_extra_parent("orphan2")
        .time(4_000)
        .create()
        .unwrap()
        .detach();

    let h = TestHarness::new();
    h.write_config(
        &Config::default()
            .disable("all")
            .enable("H15-octopus")
            .enable("H17-ex-nihilo")
            .enable("H18-second-chance"),
    );

    let mut cmd = h.command();
    cmd.arg("check").arg(temp.tempdir.path()).arg("main");
    let output = cmd.captured_output();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "command failed:\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );

    // H17-ex-nihilo: each of the three empty root commits grants once.
    assert_grants(&stdout, initial, "Ex Nihilo");
    assert_grants(&stdout, second_root, "Ex Nihilo");
    assert_grants(&stdout, third_root, "Ex Nihilo");

    // H18-second-chance: the chronologically oldest root ("Initial commit") is the original and
    // grants nothing. The 2nd-oldest ("Second root") gets "Second Chance"; the 3rd ("Third root")
    // gets "Third Time's the Charm".
    assert_grants(&stdout, second_root, "Second Chance");
    assert_grants(&stdout, third_root, "Third Time's the Charm");

    // H15-octopus: the merge has three parents, within the default 3..8 octopus window.
    assert_grants(&stdout, merge, "So You Have a Thing for Tentacles?");
}
