use std::collections::BTreeMap;
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

/// Decode HTML entities in a string
fn decode_html_entities(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '&' {
            let mut entity = String::new();
            for ec in chars.by_ref() {
                if ec == ';' {
                    break;
                }
                entity.push(ec);
            }
            match entity.as_str() {
                "amp" => result.push('&'),
                "lt" => result.push('<'),
                "gt" => result.push('>'),
                "quot" => result.push('"'),
                s if s.starts_with("#x") || s.starts_with("#X") => {
                    if let Ok(n) = u32::from_str_radix(&s[2..], 16)
                        && let Some(decoded) = char::from_u32(n)
                    {
                        result.push(decoded);
                        continue;
                    }
                    result.push('&');
                    result.push_str(&entity);
                    result.push(';');
                }
                _ => {
                    result.push('&');
                    result.push_str(&entity);
                    result.push(';');
                }
            }
        } else {
            result.push(c);
        }
    }
    result
}

/// Extract all href attribute values from an HTML string, with HTML entities decoded
fn extract_hrefs(html: &str) -> Vec<String> {
    let mut hrefs = Vec::new();
    let pattern = "href=\"";
    let mut search_from = 0;
    while let Some(start) = html[search_from..].find(pattern) {
        let value_start = search_from + start + pattern.len();
        if let Some(end) = html[value_start..].find('"') {
            let raw = &html[value_start..value_start + end];
            hrefs.push(decode_html_entities(raw));
            search_from = value_start + end + 1;
        } else {
            break;
        }
    }
    hrefs
}

/// Collect all .html files under a directory recursively
fn collect_html_files(dir: &Path) -> Vec<std::path::PathBuf> {
    let mut files = Vec::new();
    for entry in std::fs::read_dir(dir).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.is_dir() {
            files.extend(collect_html_files(&path));
        } else if path.extension().is_some_and(|e| e == "html") {
            files.push(path);
        }
    }
    files.sort();
    files
}

#[test]
fn render_all_internal_links_resolve() {
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

    let html_files = collect_html_files(&output_dir);
    assert!(!html_files.is_empty(), "render should produce HTML files");

    // Map from (source file, href) -> resolved path, for broken links
    let mut broken: BTreeMap<(String, String), std::path::PathBuf> = BTreeMap::new();
    let mut checked = 0;

    for file in &html_files {
        let content = std::fs::read_to_string(file).unwrap();
        let parent = file.parent().unwrap();

        for href in extract_hrefs(&content) {
            // Skip external links
            if href.starts_with("http://")
                || href.starts_with("https://")
                || href.starts_with("//")
                || href.starts_with("mailto:")
            {
                continue;
            }

            // Strip fragment
            let path_part = match href.find('#') {
                Some(i) => &href[..i],
                None => href.as_str(),
            };

            // Skip empty (pure fragment links like "#section")
            if path_part.is_empty() {
                continue;
            }

            let resolved = parent.join(path_part);
            checked += 1;

            if !resolved.exists() {
                let relative_source = file.strip_prefix(&output_dir).unwrap();
                broken.insert(
                    (relative_source.display().to_string(), href.clone()),
                    resolved,
                );
            }
        }
    }

    assert!(
        checked > 0,
        "should have checked at least one internal link"
    );

    if !broken.is_empty() {
        let mut msg = format!("{} broken internal link(s):\n", broken.len());
        for ((source, href), resolved) in &broken {
            msg.push_str(&format!(
                "  {} -> href=\"{}\" (resolved to {})\n",
                source,
                href,
                resolved.display()
            ));
        }
        panic!("{msg}");
    }
}
