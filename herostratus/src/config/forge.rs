/// Infer a commit URL prefix from a Git clone URL by detecting the forge type.
///
/// Returns a URL prefix that, when concatenated with a commit hash, produces a valid link to the
/// commit on the forge's web UI. For example, `https://github.com/owner/repo/commit/` concatenated
/// with `abc123` gives the full commit URL.
///
/// Returns None if the forge cannot be identified or the URL cannot be parsed (e.g., `file://` URLs
/// or local paths).
pub fn infer_commit_url_prefix(url: &str) -> Option<String> {
    let parsed = parse_clone_url(url)?;
    let forge = detect_forge(&parsed.host)?;
    let web_host = web_host(forge, &parsed.host);
    let commit_path = commit_url_path(forge, &parsed.path)?;
    Some(format!("https://{web_host}/{commit_path}"))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Forge {
    GitHub,
    GitLab,
    Bitbucket,
    Forgejo,
    AzureDevOps,
    SourceHut,
}

struct ParsedUrl {
    host: String,
    /// The path component, without a leading `/` or trailing `.git`
    path: String,
}

/// Parse a Git clone URL into its host and path components.
///
/// Handles SSH shorthand (`git@host:path`), SSH URLs (`ssh://...`), and HTTP(S) URLs.
/// Returns None for `file://` URLs and local paths.
fn parse_clone_url(url: &str) -> Option<ParsedUrl> {
    // file:// URLs and bare paths have no forge
    if url.starts_with("file://") || url.starts_with('/') || url.starts_with('.') {
        return None;
    }

    // SSH shorthand: user@host:path (no :// present)
    if !url.contains("://") {
        if let Some((user_host, path)) = url.split_once(':')
            && user_host.contains('@')
        {
            let host = user_host.rsplit_once('@').map(|(_, h)| h)?;
            let path = path.trim_end_matches(".git");
            return Some(ParsedUrl {
                host: host.to_string(),
                path: path.to_string(),
            });
        }
        return None;
    }

    // URL with scheme: ssh://, https://, http://
    let without_scheme = url
        .strip_prefix("ssh://")
        .or_else(|| url.strip_prefix("https://"))
        .or_else(|| url.strip_prefix("http://"))?;

    let (host_part, path) = without_scheme.split_once('/')?;

    // Remove user@ prefix if present
    let host_part = host_part
        .rsplit_once('@')
        .map(|(_, h)| h)
        .unwrap_or(host_part);

    // Remove :port suffix if present
    let host = host_part.split(':').next()?;

    let path = path.trim_end_matches(".git").trim_end_matches('/');

    Some(ParsedUrl {
        host: host.to_string(),
        path: path.to_string(),
    })
}

/// Detect the forge from a hostname, using exact matches for well-known hosts and substring matches
/// for self-hosted instances.
fn detect_forge(host: &str) -> Option<Forge> {
    let host_lower = host.to_lowercase();

    // Exact matches for well-known hosts
    match host_lower.as_str() {
        "github.com" => return Some(Forge::GitHub),
        "gitlab.com" => return Some(Forge::GitLab),
        "bitbucket.org" => return Some(Forge::Bitbucket),
        "codeberg.org" => return Some(Forge::Forgejo),
        "dev.azure.com" | "ssh.dev.azure.com" => return Some(Forge::AzureDevOps),
        "git.sr.ht" => return Some(Forge::SourceHut),
        _ => {}
    }

    // Substring matches for self-hosted instances. Order matters: check more specific patterns
    // first to avoid ambiguity.
    if host_lower.contains("github") {
        Some(Forge::GitHub)
    } else if host_lower.contains("gitlab") {
        Some(Forge::GitLab)
    } else if host_lower.contains("bitbucket") {
        Some(Forge::Bitbucket)
    } else if host_lower.contains("gitea") || host_lower.contains("forgejo") {
        Some(Forge::Forgejo)
    } else if host_lower.contains("azure")
        || host_lower.contains("visualstudio")
        || host_lower.contains("vs-ssh")
    {
        Some(Forge::AzureDevOps)
    } else if host_lower.contains("sr.ht") || host_lower.contains("sourcehut") {
        Some(Forge::SourceHut)
    } else {
        None
    }
}

/// Map an SSH host to its web UI host when they differ.
fn web_host(forge: Forge, host: &str) -> String {
    match forge {
        Forge::AzureDevOps if host.eq_ignore_ascii_case("ssh.dev.azure.com") => {
            "dev.azure.com".to_string()
        }
        Forge::AzureDevOps if host.contains("vs-ssh") => {
            // vs-ssh.visualstudio.com -> dev.azure.com
            "dev.azure.com".to_string()
        }
        _ => host.to_string(),
    }
}

/// Construct the commit URL path component for the given forge.
///
/// The returned path does NOT have a leading `/`, but DOES have a trailing `/` so that a commit
/// hash can be directly appended.
fn commit_url_path(forge: Forge, path: &str) -> Option<String> {
    match forge {
        Forge::GitHub | Forge::Forgejo | Forge::SourceHut => Some(format!("{path}/commit/")),
        Forge::GitLab => Some(format!("{path}/-/commit/")),
        Forge::Bitbucket => Some(format!("{path}/commits/")),
        Forge::AzureDevOps => {
            // SSH uses v3/org/project/repo; transform to org/project/_git/repo
            if let Some(rest) = path.strip_prefix("v3/") {
                let parts: Vec<&str> = rest.splitn(3, '/').collect();
                if parts.len() == 3 {
                    Some(format!(
                        "{}/_git/{}/commit/",
                        parts[..2].join("/"),
                        parts[2]
                    ))
                } else {
                    None
                }
            } else {
                // HTTPS path already has _git/ in it
                Some(format!("{path}/commit/"))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn github_https() {
        let url = "https://github.com/Notgnoshi/herostratus.git";
        assert_eq!(
            infer_commit_url_prefix(url).unwrap(),
            "https://github.com/Notgnoshi/herostratus/commit/"
        );
    }

    #[test]
    fn github_ssh() {
        let url = "git@github.com:Notgnoshi/herostratus.git";
        assert_eq!(
            infer_commit_url_prefix(url).unwrap(),
            "https://github.com/Notgnoshi/herostratus/commit/"
        );
    }

    #[test]
    fn github_ssh_url() {
        let url = "ssh://git@github.com/Notgnoshi/herostratus.git";
        assert_eq!(
            infer_commit_url_prefix(url).unwrap(),
            "https://github.com/Notgnoshi/herostratus/commit/"
        );
    }

    #[test]
    fn github_no_dotgit_suffix() {
        let url = "https://github.com/Notgnoshi/herostratus";
        assert_eq!(
            infer_commit_url_prefix(url).unwrap(),
            "https://github.com/Notgnoshi/herostratus/commit/"
        );
    }

    #[test]
    fn gitlab_https() {
        let url = "https://gitlab.com/user/project.git";
        assert_eq!(
            infer_commit_url_prefix(url).unwrap(),
            "https://gitlab.com/user/project/-/commit/"
        );
    }

    #[test]
    fn gitlab_nested_group() {
        let url = "https://gitlab.com/group/subgroup/project.git";
        assert_eq!(
            infer_commit_url_prefix(url).unwrap(),
            "https://gitlab.com/group/subgroup/project/-/commit/"
        );
    }

    #[test]
    fn gitlab_ssh() {
        let url = "git@gitlab.com:user/project.git";
        assert_eq!(
            infer_commit_url_prefix(url).unwrap(),
            "https://gitlab.com/user/project/-/commit/"
        );
    }

    #[test]
    fn bitbucket_https() {
        let url = "https://bitbucket.org/owner/repo.git";
        assert_eq!(
            infer_commit_url_prefix(url).unwrap(),
            "https://bitbucket.org/owner/repo/commits/"
        );
    }

    #[test]
    fn bitbucket_ssh() {
        let url = "git@bitbucket.org:owner/repo.git";
        assert_eq!(
            infer_commit_url_prefix(url).unwrap(),
            "https://bitbucket.org/owner/repo/commits/"
        );
    }

    #[test]
    fn forgejo_codeberg() {
        let url = "https://codeberg.org/owner/repo.git";
        assert_eq!(
            infer_commit_url_prefix(url).unwrap(),
            "https://codeberg.org/owner/repo/commit/"
        );
    }

    #[test]
    fn forgejo_codeberg_ssh() {
        let url = "git@codeberg.org:owner/repo.git";
        assert_eq!(
            infer_commit_url_prefix(url).unwrap(),
            "https://codeberg.org/owner/repo/commit/"
        );
    }

    #[test]
    fn azure_devops_https() {
        let url = "https://dev.azure.com/org/project/_git/repo";
        assert_eq!(
            infer_commit_url_prefix(url).unwrap(),
            "https://dev.azure.com/org/project/_git/repo/commit/"
        );
    }

    #[test]
    fn azure_devops_ssh() {
        let url = "git@ssh.dev.azure.com:v3/org/project/repo";
        assert_eq!(
            infer_commit_url_prefix(url).unwrap(),
            "https://dev.azure.com/org/project/_git/repo/commit/"
        );
    }

    #[test]
    fn sourcehut_https() {
        let url = "https://git.sr.ht/~owner/repo";
        assert_eq!(
            infer_commit_url_prefix(url).unwrap(),
            "https://git.sr.ht/~owner/repo/commit/"
        );
    }

    #[test]
    fn sourcehut_ssh() {
        let url = "git@git.sr.ht:~owner/repo";
        assert_eq!(
            infer_commit_url_prefix(url).unwrap(),
            "https://git.sr.ht/~owner/repo/commit/"
        );
    }

    #[test]
    fn self_hosted_github() {
        let url = "https://github.example.com/owner/repo.git";
        assert_eq!(
            infer_commit_url_prefix(url).unwrap(),
            "https://github.example.com/owner/repo/commit/"
        );
    }

    #[test]
    fn self_hosted_gitlab() {
        let url = "https://gitlab.mycompany.com/team/project.git";
        assert_eq!(
            infer_commit_url_prefix(url).unwrap(),
            "https://gitlab.mycompany.com/team/project/-/commit/"
        );
    }

    #[test]
    fn self_hosted_gitea() {
        let url = "https://gitea.internal.dev/owner/repo.git";
        assert_eq!(
            infer_commit_url_prefix(url).unwrap(),
            "https://gitea.internal.dev/owner/repo/commit/"
        );
    }

    #[test]
    fn self_hosted_forgejo() {
        let url = "https://forgejo.example.org/owner/repo.git";
        assert_eq!(
            infer_commit_url_prefix(url).unwrap(),
            "https://forgejo.example.org/owner/repo/commit/"
        );
    }

    #[test]
    fn self_hosted_bitbucket() {
        let url = "https://bitbucket.mycompany.com/scm/project/repo.git";
        assert_eq!(
            infer_commit_url_prefix(url).unwrap(),
            "https://bitbucket.mycompany.com/scm/project/repo/commits/"
        );
    }

    #[test]
    fn self_hosted_sourcehut() {
        let url = "https://git.sr.ht.example.com/~owner/repo";
        assert_eq!(
            infer_commit_url_prefix(url).unwrap(),
            "https://git.sr.ht.example.com/~owner/repo/commit/"
        );
    }

    #[test]
    fn file_url_returns_none() {
        let url = "file:///home/user/repo.git";
        assert!(infer_commit_url_prefix(url).is_none());
    }

    #[test]
    fn local_path_returns_none() {
        let url = "/home/user/repo.git";
        assert!(infer_commit_url_prefix(url).is_none());
    }

    #[test]
    fn relative_path_returns_none() {
        let url = "./repo";
        assert!(infer_commit_url_prefix(url).is_none());
    }

    #[test]
    fn unknown_host_returns_none() {
        let url = "https://example.com/owner/repo.git";
        assert!(infer_commit_url_prefix(url).is_none());
    }

    #[test]
    fn http_url() {
        let url = "http://github.com/owner/repo.git";
        assert_eq!(
            infer_commit_url_prefix(url).unwrap(),
            "https://github.com/owner/repo/commit/"
        );
    }

    #[test]
    fn ssh_url_with_port() {
        let url = "ssh://git@gitlab.example.com:2222/owner/repo.git";
        assert_eq!(
            infer_commit_url_prefix(url).unwrap(),
            "https://gitlab.example.com/owner/repo/-/commit/"
        );
    }
}
