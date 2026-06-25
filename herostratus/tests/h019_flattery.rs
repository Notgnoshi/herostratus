use herostratus::config::Config;
use herostratus_tests::cmd::{CommandExt, TestHarness, assert_grants};
use herostratus_tests::fixtures::repository::Builder;

#[test]
fn h019_flattery() {
    // history (oldest -> newest):
    //   1000  "wip"             (Alice)  -- original, earns nothing
    //   2000  "a real change"   (Alice)  -- unique, earns nothing
    //   3000  "WIP"             (Bob)    -- copy of "wip" (normalized), earns flattery
    let temp = Builder::new()
        .commit("wip")
        .author("Alice", "alice@example.com")
        .time(1_000)
        .build()
        .unwrap();
    let original = temp.repo.head_id().unwrap().detach();

    let unique = temp
        .commit("a real change")
        .author("Alice", "alice@example.com")
        .time(2_000)
        .create()
        .unwrap()
        .detach();

    let copier = temp
        .commit("WIP")
        .author("Bob", "bob@example.com")
        .time(3_000)
        .create()
        .unwrap()
        .detach();

    let h = TestHarness::new();
    h.write_config(&Config::default().disable("all").enable("H19-flattery"));

    let mut cmd = h.command();
    cmd.arg("check").arg(temp.tempdir.path()).arg("HEAD");
    let output = cmd.captured_output();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "command failed:\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );

    let title = "Imitation Is the Sincerest Form of Flattery";
    // Bob's "WIP" copies Alice's older "wip" and earns the achievement.
    assert_grants(&stdout, copier, title);
    // The original and the unique subject earn nothing.
    let original = original.to_string();
    let unique = unique.to_string();
    for line in stdout.lines().filter(|l| l.contains(title)) {
        assert!(
            !line.contains(&original),
            "original commit should not earn flattery:\n{line}"
        );
        assert!(
            !line.contains(&unique),
            "unique-subject commit should not earn flattery:\n{line}"
        );
    }
}
