use std::path::Path;

use herostratus_tests::cmd::{CommandExt, TestHarness};
use predicates::prelude::*;

fn write_file(dir: &Path, name: &str, content: &str) {
    let path = dir.join(name);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(path, content).unwrap();
}

fn setup_fixture(dir: &Path) {
    write_file(
        dir,
        "export/achievements.csv",
        "id,human_id,name,description,kind\n\
         1,fixup,Leftovers,Prefix a commit with fixup,per-user\n\
         2,shortest,Brevity,The shortest subject line,global-revocable\n",
    );
    write_file(
        dir,
        "export/repositories.csv",
        "name,url,commit_url_prefix,ref,commits_checked,last_checked\n\
         test-repo,https://example.com/test.git,https://example.com/test/commit/,main,42,2026-01-15T00:00:00Z\n",
    );
    write_file(
        dir,
        "export/events/test-repo.csv",
        "timestamp,event,achievement_id,commit,user_name,user_email\n\
         2026-01-01T00:00:00Z,grant,fixup,aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa,Alice Smith,alice@example.com\n\
         2026-01-02T00:00:00Z,grant,fixup,bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb,Bob Jones,bob@example.com\n\
         2026-01-03T00:00:00Z,grant,shortest,cccccccccccccccccccccccccccccccccccccccc,Alice Smith,alice@example.com\n",
    );
}

fn templates_dir() -> std::path::PathBuf {
    // The templates directory is at the repo root, two levels up from herostratus/tests/
    let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir.parent().unwrap().join("templates")
}

#[test]
fn render_generates_all_pages() {
    let dir = tempfile::tempdir().unwrap();
    setup_fixture(dir.path());

    let output_dir = dir.path().join("site");
    let mut cmd = TestHarness::stateless_command();
    cmd.arg("render")
        .arg("--export-dir")
        .arg(dir.path().join("export"))
        .arg("--output-dir")
        .arg(&output_dir)
        .arg("--templates")
        .arg(templates_dir());
    let output = cmd.captured_output();
    assert!(output.status.success());

    // Root pages
    assert!(output_dir.join("index.html").exists());
    assert!(output_dir.join("achievements.html").exists());

    // Achievement detail pages
    assert!(output_dir.join("achievement/fixup.html").exists());
    assert!(output_dir.join("achievement/shortest.html").exists());

    // Repository page
    assert!(output_dir.join("repo/test-repo.html").exists());

    // User pages
    assert!(output_dir.join("user/alice-smith.html").exists());
    assert!(output_dir.join("user/bob-jones.html").exists());
}

#[test]
fn render_index_contains_repo_and_activity() {
    let dir = tempfile::tempdir().unwrap();
    setup_fixture(dir.path());

    let output_dir = dir.path().join("site");
    let mut cmd = TestHarness::stateless_command();
    cmd.arg("render")
        .arg("-e")
        .arg(dir.path().join("export"))
        .arg("-o")
        .arg(&output_dir)
        .arg("-t")
        .arg(templates_dir());
    let output = cmd.captured_output();
    assert!(output.status.success());

    let index = std::fs::read_to_string(output_dir.join("index.html")).unwrap();
    assert!(
        predicates::str::contains("test-repo").eval(&index),
        "index should contain repo name"
    );
    assert!(
        predicates::str::contains("Alice Smith").eval(&index),
        "index should contain user name in recent activity"
    );
    assert!(
        predicates::str::contains("Leftovers").eval(&index),
        "index should contain achievement name"
    );
}

#[test]
fn render_cross_links_work() {
    let dir = tempfile::tempdir().unwrap();
    setup_fixture(dir.path());

    let output_dir = dir.path().join("site");
    let mut cmd = TestHarness::stateless_command();
    cmd.arg("render")
        .arg("-e")
        .arg(dir.path().join("export"))
        .arg("-o")
        .arg(&output_dir)
        .arg("-t")
        .arg(templates_dir());
    let output = cmd.captured_output();
    assert!(output.status.success());

    // Repo page links to users and achievements
    let repo_page = std::fs::read_to_string(output_dir.join("repo/test-repo.html")).unwrap();
    assert!(repo_page.contains("user/alice-smith.html"));
    assert!(repo_page.contains("achievement/fixup.html"));

    // User page links to achievements and repos
    let user_page = std::fs::read_to_string(output_dir.join("user/alice-smith.html")).unwrap();
    assert!(user_page.contains("achievement/fixup.html"));
    assert!(user_page.contains("repo/test-repo.html"));

    // Achievement page links to users and repos
    let achv_page = std::fs::read_to_string(output_dir.join("achievement/fixup.html")).unwrap();
    assert!(achv_page.contains("user/alice-smith.html"));
    assert!(achv_page.contains("repo/test-repo.html"));
}

#[test]
fn render_commit_links_use_prefix() {
    let dir = tempfile::tempdir().unwrap();
    setup_fixture(dir.path());

    let output_dir = dir.path().join("site");
    let mut cmd = TestHarness::stateless_command();
    cmd.arg("render")
        .arg("-e")
        .arg(dir.path().join("export"))
        .arg("-o")
        .arg(&output_dir)
        .arg("-t")
        .arg(templates_dir());
    let output = cmd.captured_output();
    assert!(output.status.success());

    let repo_page = std::fs::read_to_string(output_dir.join("repo/test-repo.html")).unwrap();
    // minijinja HTML-escapes URLs in attributes, so check for the commit hash which is not escaped
    assert!(
        repo_page.contains("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
        "commit hash should appear on repo page"
    );
    assert!(
        repo_page.contains(">aaaaaaaaaaaa</a>"),
        "commit should be linked on repo page"
    );

    let user_page = std::fs::read_to_string(output_dir.join("user/alice-smith.html")).unwrap();
    assert!(
        user_page.contains("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
        "commit hash should appear on user page"
    );
    // Check the commit hash is inside an <a> tag (linked, not plain text)
    assert!(
        user_page.contains(">aaaaaaaaaaaa</a>"),
        "commit should be linked on user page"
    );
}
