use std::path::PathBuf;

use crate::git::clone::parse_path_from_url;

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
