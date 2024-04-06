use std::path::PathBuf;

use crate::git::clone::{local_or_remote, parse_path_from_url, RepoType};

#[test]
fn local_path_no_exist() {
    let path = "this-path-does-not-exist";
    let repo_type = local_or_remote(path);
    assert!(repo_type.is_err(), "Local path required to exist");

    // Parsed as local path because there's a slash before the colon
    let path = "this/path:also/does/not/exist";
    let repo_type = local_or_remote(path);
    assert!(repo_type.is_err(), "Local path required to exist");
}

#[test]
fn local_path_exist() {
    let relative_path = ".";
    let repo_type = local_or_remote(relative_path).unwrap();
    assert!(matches!(repo_type, RepoType::LocalFilePath(_)));

    let absolute_path = "/";
    let repo_type = local_or_remote(absolute_path).unwrap();
    assert!(matches!(repo_type, RepoType::LocalFilePath(_)));
}

#[test]
fn remote_protocols() {
    let urls = [
        "ssh://git@example.com/path.git",
        "git@github.com:Notgnoshi/herostratus.git",
        "https://example.com/foo",
        "domain:path",
    ];
    for url in urls {
        let repo_type = local_or_remote(url).unwrap();
        assert!(matches!(repo_type, RepoType::RemoteCloneUrl(_)));
    }
}

#[test]
fn test_parse_path_from_url() {
    let url_paths = [
        (
            "git@github.com:Notgnoshi/herostratus.git",
            "Notgnoshi/herostratus.git",
        ),
        ("domain:path", "path"),
        ("ssh://git@example.com:2222/path.git", "path.git"),
        ("ssh://git@example.com/path.git", "path.git"),
        ("https://example.com/path", "path"),
        ("file:///tmp/foo", "tmp/foo"),
    ];

    for (url, expected) in url_paths {
        let expected = PathBuf::from(expected);
        let actual = parse_path_from_url(url).unwrap();
        assert_eq!(expected, actual);
    }
}
