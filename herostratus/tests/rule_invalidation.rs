use std::fs;

use herostratus_tests::cmd::{CommandExt, TestHarness};
use herostratus_tests::fixtures::repository::Builder;

/// Locate the per-repo cache subdirectory inside the harness's data directory.
fn find_repo_cache_dir(data_dir: &std::path::Path) -> std::path::PathBuf {
    for entry in fs::read_dir(data_dir.join("cache")).unwrap() {
        let entry = entry.unwrap();
        if entry.file_type().unwrap().is_dir() {
            return entry.path();
        }
    }
    panic!("no per-repo cache directory created under {data_dir:?}");
}

/// Locate the per-repo events CSV inside the harness's data directory.
fn find_events_csv(data_dir: &std::path::Path) -> std::path::PathBuf {
    for entry in fs::read_dir(data_dir.join("export").join("events")).unwrap() {
        let entry = entry.unwrap();
        if entry.path().extension().and_then(|s| s.to_str()) == Some("csv") {
            return entry.path();
        }
    }
    panic!("no events CSV under {data_dir:?}");
}

/// Simulate a version bump by writing version 0 into the checkpoint for rule 2 (shortest-subject).
/// On the next run: rule 2's cache file is removed, its grants are pruned from the events CSV,
/// rule 1's grants are preserved, and a WARN message is emitted. The cache file is then re-created
/// when rule 2 finalizes.
#[test]
fn version_bump_invalidates_cache_and_prunes_events() {
    let upstream = Builder::new()
        .commit("fixup! initial")
        .author("Alice", "alice@example.com")
        .commit("Hi")
        .author("Alice", "alice@example.com")
        .build()
        .unwrap();
    let url = format!("file://{}", upstream.tempdir.path().display());

    let h = TestHarness::new();

    let mut cmd = h.command();
    cmd.arg("add").arg(url);
    assert!(cmd.captured_output().status.success());

    // Run 1: produces H001 (fixup, no cache) and H002 (shortest-subject, has cache).
    let mut cmd = h.command();
    cmd.arg("check-all");
    assert!(cmd.captured_output().status.success());

    let cache_dir = find_repo_cache_dir(h.path());
    let checkpoint_path = cache_dir.join("checkpoint.json");
    let h002_cache_path = cache_dir.join("rule_shortest-subject-line.json");
    assert!(
        h002_cache_path.exists(),
        "H002 cache should exist after run 1"
    );

    // Force-seed the checkpoint so rule 2 appears at version 0.
    let contents = fs::read_to_string(&checkpoint_path).unwrap();
    let mut value: serde_json::Value = serde_json::from_str(&contents).unwrap();
    let rules = value["rules"].as_array_mut().unwrap();
    for pair in rules.iter_mut() {
        if pair[0].as_u64().unwrap() == 2 {
            pair[1] = serde_json::json!(0u32);
        }
    }
    fs::write(&checkpoint_path, value.to_string()).unwrap();

    let events_path = find_events_csv(h.path());
    let pre_csv = fs::read_to_string(&events_path).unwrap();
    assert!(
        pre_csv.contains("shortest-subject-line"),
        "H002 grant expected pre-invalidation: {pre_csv}"
    );
    assert!(
        pre_csv.contains("fixup"),
        "H001 grant expected pre-invalidation: {pre_csv}"
    );

    // Run 2: invalidation fires for rule 2.
    let mut cmd = h.command();
    cmd.arg("check-all");
    let out = cmd.captured_output();
    assert!(out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("Rule shortest-subject-line version changed"),
        "expected invalidation WARN for H002: {stderr}"
    );

    // H002's cache file is replaced (re-saved with the recomputed cache).
    assert!(
        h002_cache_path.exists(),
        "H002 cache should be re-created after invalidation"
    );

    // Events CSV still has H001 grants and now has fresh H002 grants.
    let post_csv = fs::read_to_string(&events_path).unwrap();
    assert!(post_csv.contains("fixup"), "H001 grants should persist");
    assert!(
        post_csv.contains("shortest-subject-line"),
        "H002 grants should be re-emitted"
    );
}
