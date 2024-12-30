use std::path::{Path, PathBuf};

use eyre::WrapErr;

pub fn find_local_repository<P: AsRef<Path> + std::fmt::Debug>(
    path: P,
) -> eyre::Result<git2::Repository> {
    tracing::debug!("Searching local path {path:?} for a Git repository");
    let repo = git2::Repository::discover(path)?;
    tracing::debug!("Found local git repository at {:?}", repo.path());
    Ok(repo)
}

pub fn find_local_repository_gix<P: AsRef<Path> + std::fmt::Debug>(
    path: P,
) -> eyre::Result<gix::Repository> {
    tracing::debug!("Searching local path {path:?} for a Git repository");
    let repo = gix::discover(path)?;
    tracing::debug!("Found local git repository at {:?}", repo.path());
    Ok(repo)
}

/// Parse a mostly-unique filesystem path from a clone URL
///
/// ```text
/// ssh://git@example.com/path.git           => path.git
/// git@github.com:Notgnoshi/herostratus.git => Notgnoshi/herostratus.git
/// https://example.com/foo                  => foo
/// domain:path                              => path
/// ```
fn parse_path_from_url(url: &str) -> eyre::Result<PathBuf> {
    let known_remote_protocols = [
        "ssh://", "git://", "http://", "https://", "ftp://", "ftps://", "file://",
    ];
    for protocol in known_remote_protocols {
        if let Some(url) = url.strip_prefix(protocol) {
            let Some(idx) = url.find('/') else {
                eyre::bail!("Failed to find '/' separator in {url:?}");
            };
            return Ok(PathBuf::from(&url[idx + 1..]));
        }
    }

    if url.contains("://") {
        eyre::bail!("Found unsupported protocol from URL {url:?}");
    }

    // If the URL doesn't begin with a protocol, it's likely the alternative SSH syntax, where the
    // path begins after the ':'
    //
    // TODO(#26): Support ~username expansion.

    let Some(idx) = url.find(':') else {
        eyre::bail!("Failed to find ':' separator in {url:?}")
    };
    Ok(PathBuf::from(&url[idx + 1..]))
}

/// Get the path to clone the given URL into in the application data directory
pub fn get_clone_path(data_dir: &Path, url: &str) -> eyre::Result<PathBuf> {
    let clone_path = parse_path_from_url(url).wrap_err("Failed to parse clone path from URL")?;
    let clone_path = data_dir.join("git").join(clone_path);
    Ok(clone_path)
}

/// Remove the contents of the given directory, though not the directory itself
fn remove_dir_contents<P: AsRef<Path>>(path: P) -> std::io::Result<()> {
    for entry in std::fs::read_dir(path)? {
        let entry = entry?;
        let path = entry.path();

        if entry.file_type()?.is_dir() {
            remove_dir_contents(&path)?;
            std::fs::remove_dir(path)?;
        } else {
            std::fs::remove_file(path)?;
        }
    }
    Ok(())
}

fn clone_credentials(
    config: &crate::config::RepositoryConfig,
    parsed_username: Option<&str>,
) -> eyre::Result<git2::Cred> {
    let username = config
        .remote_username
        .as_deref()
        .unwrap_or(parsed_username.unwrap_or("git"));

    if config.url.starts_with("https://") {
        if let Some(password) = &config.https_password {
            Ok(git2::Cred::userpass_plaintext(username, password)?)
        } else {
            let git_config = git2::Config::open_default()?;
            Ok(git2::Cred::credential_helper(
                &git_config,
                &config.url,
                Some(username),
            )?)
        }
    } else if config.url.starts_with("ssh://") || config.url.contains('@') {
        if let Some(priv_key) = &config.ssh_private_key {
            Ok(git2::Cred::ssh_key(
                username,
                config.ssh_public_key.as_deref(),
                priv_key,
                config.ssh_passphrase.as_deref(),
            )?)
        } else {
            Ok(git2::Cred::ssh_key_from_agent(username)?)
        }
    } else {
        Ok(git2::Cred::default()?)
    }
}

fn fetch_options(config: &crate::config::RepositoryConfig) -> git2::FetchOptions {
    let mut callbacks = git2::RemoteCallbacks::new();
    callbacks.credentials(|_url, username_from_url, _allowed_typed| {
        match clone_credentials(config, username_from_url) {
            Ok(creds) => Ok(creds),
            Err(e) => Err(git2::Error::new(
                git2::ErrorCode::NotFound,
                git2::ErrorClass::Config,
                format!("Failed to determine appropriate clone credentials: {e:?}"),
            )),
        }
    });
    let mut options = git2::FetchOptions::new();
    options.remote_callbacks(callbacks);
    options
}

/// Update the local branch to match the remote's
///
/// Returns the number of new commits fetched. Creates the local branch if it does not exist. If a
/// non-HEAD reference is given, do a "fast" fetch of *just* the specified reference, as opposed to
/// all references from the remote.
///
/// TODO: Probably doesn't work on tags?
pub fn pull_branch(
    config: &crate::config::RepositoryConfig,
    repo: &git2::Repository,
) -> eyre::Result<usize> {
    let mut remote = repo.find_remote("origin")?;
    let reference_name = config.branch.as_deref().unwrap_or("HEAD");
    // If this is the first time this reference is being fetched, fetch it like
    //     git fetch origin branch:branch
    // which updates the local branch to match the remote
    let fetch_reference_name = if reference_name != "HEAD" {
        format!("{reference_name}:{reference_name}")
    } else {
        reference_name.to_string()
    };

    let refspecs = [&fetch_reference_name];
    let mut options = fetch_options(config);

    tracing::info!("Fetching from {:?} ...", remote.url().unwrap_or_default());
    let before = if let Ok(reference) = repo.resolve_reference_from_short_name(reference_name) {
        reference.peel_to_commit().ok()
    } else {
        None
    };
    remote.fetch(&refspecs, Some(&mut options), None)?;
    // If the fetch was successful, resolving the reference should succeed, even if this was the
    // first fetch ever for this reference.
    let reference = repo.resolve_reference_from_short_name(reference_name)?;
    // BUG: The fetch doesn't update the local branch (unless it creates the branch), so this
    // 'after' remains unchanged after fetching. Can use Reference::set_target to update the commit
    // the reference points to, but to do that, we need to find the target of the remote reference.
    //
    // TODO: This doesn't work, possibly because fetch is unfinished? (TODO: Read about FETCH_HEAD;
    // the .git/refs/ only has heads/ and not remotes/ - there's nowhere else where the remote
    // upstream target of the reference is tracked).
    //
    // let remote_reference = repo.find_reference(&format!("origin/{reference_name}"))?;
    let after = reference.peel_to_commit()?;

    let mut new_commits: usize = 0;
    if before.is_some() && before.as_ref().unwrap().id() == after.id() {
        tracing::debug!(
            "... done. {:?} -> {:?} No new commits",
            before.as_ref().unwrap().id(),
            after.id()
        );
    } else {
        let commits = crate::git::rev::walk(after.id(), repo)?;
        for commit_id in commits {
            if let Some(before) = &before {
                if commit_id? == before.id() {
                    break;
                }
            }
            new_commits += 1;
        }
        tracing::debug!(
            "... done. {:?} -> {:?} {new_commits} new commits",
            before,
            after.id()
        );
    }

    Ok(new_commits)
}

/// Clone the given repository
///
/// If the repository already exists on-disk, then if
/// * `force == false`, rather than cloning, update the reference from the given config
/// * `force == true`, delete the existing repository and re-clone
///
/// If there's an existing repository on-disk with a different clone URL (even if it's just HTTPS
/// vs SSH) then fail.
///
/// If a branch has been specified, then clone *just* that branch.
pub fn clone_repository(
    config: &crate::config::RepositoryConfig,
    force: bool,
) -> eyre::Result<git2::Repository> {
    let start = std::time::Instant::now();
    tracing::info!(
        "Cloning {:?} (ref={}) to {} ...",
        config.url,
        config.branch.as_deref().unwrap_or("HEAD"),
        config.path.display()
    );

    if config.path.exists() {
        tracing::warn!("{} already exists ...", config.path.display());
        if force {
            tracing::info!("Deleting {} ...", config.path.display());
            remove_dir_contents(&config.path).wrap_err(format!(
                "Failed to force clone {:?} to {}",
                config.url,
                config.path.display()
            ))?;
        } else {
            let existing_repo = git2::Repository::discover(&config.path)?;
            let remote = existing_repo.find_remote("origin")?;
            let existing_url = remote.url().unwrap_or("THIS_STRING_WONT_MATCH");
            if existing_url == config.url {
                tracing::info!("... URLs match. Using existing repository and fetching");
                pull_branch(config, &existing_repo)?;

                drop(remote);
                return Ok(existing_repo);
            }

            eyre::bail!(
                "{} already exists with a different clone URL: {existing_url:?}",
                config.path.display()
            );
        }
    }

    let mut builder = git2::build::RepoBuilder::new();
    builder.bare(true);
    let options = fetch_options(config);
    builder.fetch_options(options);

    if let Some(branch) = &config.branch {
        tracing::debug!("Cloning just the '{branch}' branch ...");
        builder.branch(branch);
    }

    let repo = builder
        .clone(&config.url, &config.path)
        .wrap_err("Failed to clone repository")?;
    tracing::info!(
        "Finished cloning {:?} after {:.2?}",
        config.url,
        start.elapsed()
    );
    Ok(repo)
}

#[cfg(test)]
mod tests {
    use herostratus_tests::fixtures;

    use super::*;

    #[test]
    fn test_find_local_repository() {
        let temp_repo = fixtures::repository::simplest().unwrap();
        let repo = find_local_repository(temp_repo.tempdir.path()).unwrap();
        assert_eq!(repo.path(), temp_repo.tempdir.path().join(".git"));

        let repo = find_local_repository_gix(temp_repo.tempdir.path()).unwrap();
        assert_eq!(repo.path(), temp_repo.tempdir.path().join(".git"));
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

    #[test]
    fn test_pull_herostratus_own_remote() {
        // This is a Cargo workspace project, so the tests aren't run from the repository root,
        // they're run from the workspace root.
        let this_repo = find_local_repository("..").unwrap();
        // Fetching requires a RepositoryConfig, so populate it with whatever the developer has
        // cloned Herostratus with
        let remote = this_repo.find_remote("origin").unwrap();
        let config = crate::config::RepositoryConfig {
            branch: Some("main".to_string()),
            url: remote.url().unwrap().to_string(),
            ..Default::default()
        };

        // Assert that fetching doesn't fail. That's about all we can do against a GitHub remote
        pull_branch(&config, &this_repo).unwrap();
    }

    #[test]
    fn test_pull_branch_that_doesnt_exist_on_the_remote() {
        let this_repo = find_local_repository("..").unwrap();
        let remote = this_repo.find_remote("origin").unwrap();
        let config = crate::config::RepositoryConfig {
            branch: Some("THIS_BRANCH_DOESNT_EXIST".to_string()),
            url: remote.url().unwrap().to_string(),
            ..Default::default()
        };
        let result = pull_branch(&config, &this_repo);
        assert!(result.is_err());
    }

    #[test]
    fn test_pull_remote_branch_that_doesnt_exist_locally() {
        let (upstream, downstream) = fixtures::repository::upstream_downstream_empty().unwrap();
        let upstream_commit =
            fixtures::repository::add_empty_commit(&upstream.repo, "First upstream commit")
                .unwrap();
        let _downstream_commit =
            fixtures::repository::add_empty_commit(&downstream.repo, "First downstream commit")
                .unwrap();

        let remote = downstream.repo.find_remote("origin").unwrap();
        let config = crate::config::RepositoryConfig {
            branch: None, // HEAD
            url: remote.url().unwrap().to_string(),
            ..Default::default()
        };

        // Try to find the upstream commit in the downstream repository. Can't find it, because the
        // remote hasn't been fetched from yet.
        let result = downstream.repo.find_commit(upstream_commit.id());
        assert!(result.is_err());

        // Now after fetching, it can be found
        pull_branch(&config, &downstream.repo).unwrap();
        let result = downstream.repo.find_commit(upstream_commit.id());
        assert!(result.is_ok());
    }

    #[test]
    fn test_pulling_creates_a_local_branch() {
        let (upstream, downstream) = fixtures::repository::upstream_downstream().unwrap();
        fixtures::repository::switch_branch(&upstream.repo, "branch1").unwrap();
        fixtures::repository::add_empty_commit(&upstream.repo, "commit on branch1").unwrap();

        let remote = downstream.repo.find_remote("origin").unwrap();
        let config = crate::config::RepositoryConfig {
            branch: Some("branch1".to_string()),
            url: remote.url().unwrap().to_string(),
            ..Default::default()
        };

        let result = pull_branch(&config, &downstream.repo);
        assert!(result.is_ok());

        // branch1 exists as a local branch, not just a remote one
        let result = downstream
            .repo
            .find_branch("branch1", git2::BranchType::Local);
        assert!(result.is_ok());
    }

    #[test]
    fn test_fast_fetch_single_reference() {
        let (upstream, downstream) = fixtures::repository::upstream_downstream().unwrap();
        fixtures::repository::switch_branch(&upstream.repo, "branch1").unwrap();
        fixtures::repository::switch_branch(&upstream.repo, "branch2").unwrap();

        fixtures::repository::switch_branch(&upstream.repo, "branch1").unwrap();
        fixtures::repository::add_empty_commit(&upstream.repo, "commit on branch1").unwrap();

        let remote = downstream.repo.find_remote("origin").unwrap();
        let config = crate::config::RepositoryConfig {
            branch: Some("branch1".to_string()),
            url: remote.url().unwrap().to_string(),
            ..Default::default()
        };

        let result = pull_branch(&config, &downstream.repo);
        assert!(result.is_ok());

        // We find the branch1 that was fetched
        let result = downstream
            .repo
            .find_branch("branch1", git2::BranchType::Local);
        assert!(result.is_ok());

        // But not the branch2 that wasn't fetched
        let result = downstream
            .repo
            .find_branch("branch2", git2::BranchType::Local);
        assert!(result.is_err());
        let result = downstream
            .repo
            .find_branch("branch2", git2::BranchType::Remote);
        assert!(result.is_err());
    }

    #[test]
    #[ignore = "XFAIL: Reproduces pull_branch bug"]
    fn test_number_of_fetched_commits_update_existing() {
        let (upstream, downstream) = fixtures::repository::upstream_downstream().unwrap();
        fixtures::repository::switch_branch(&upstream.repo, "branch1").unwrap();
        fixtures::repository::switch_branch(&downstream.repo, "branch1").unwrap();

        fixtures::repository::add_empty_commit(&upstream.repo, "commit 1 on branch1").unwrap();
        fixtures::repository::add_empty_commit(&upstream.repo, "commit 2 on branch1").unwrap();
        fixtures::repository::add_empty_commit(&upstream.repo, "commit 3 on branch1").unwrap();
        fixtures::repository::add_empty_commit(&upstream.repo, "commit 4 on branch1").unwrap();

        let downstream = downstream.forget();
        let remote = downstream.find_remote("origin").unwrap();
        let config = crate::config::RepositoryConfig {
            branch: Some("branch1".to_string()),
            url: remote.url().unwrap().to_string(),
            ..Default::default()
        };

        let fetched_commits = pull_branch(&config, &downstream).unwrap();
        assert_eq!(fetched_commits, 4);
    }

    #[test]
    fn test_number_of_fetched_commits_create_new() {
        let (upstream, downstream) = fixtures::repository::upstream_downstream().unwrap();
        // This branch doesn't exist in the downstream repo
        fixtures::repository::switch_branch(&upstream.repo, "branch1").unwrap();

        fixtures::repository::add_empty_commit(&upstream.repo, "commit 1 on branch1").unwrap();
        fixtures::repository::add_empty_commit(&upstream.repo, "commit 2 on branch1").unwrap();
        fixtures::repository::add_empty_commit(&upstream.repo, "commit 3 on branch1").unwrap();
        fixtures::repository::add_empty_commit(&upstream.repo, "commit 4 on branch1").unwrap();

        let remote = downstream.repo.find_remote("origin").unwrap();
        let config = crate::config::RepositoryConfig {
            branch: Some("branch1".to_string()),
            url: remote.url().unwrap().to_string(),
            ..Default::default()
        };

        let fetched_commits = pull_branch(&config, &downstream.repo).unwrap();
        // The 4 new commits on the branch1 branch, as well as the single commit on the master
        // branch
        assert_eq!(fetched_commits, 5);
    }

    #[test]
    fn test_force_clone() {
        let upstream = fixtures::repository::simplest().unwrap();
        let tempdir = tempfile::tempdir().unwrap();
        let downstream_dir = tempdir.path().join("downstream");
        std::fs::create_dir_all(&downstream_dir).unwrap();
        // Something's already using the clone directory
        let sentinel = downstream_dir.join("sentinel.txt");
        std::fs::File::create(&sentinel).unwrap();
        assert!(sentinel.exists());

        let config = crate::config::RepositoryConfig {
            branch: None, // HEAD
            url: format!("file://{}", upstream.tempdir.path().display()),
            path: downstream_dir,
            ..Default::default()
        };
        let force = false;
        let result = clone_repository(&config, force);
        assert!(result.is_err());
        assert!(sentinel.exists());

        let force = true;
        let result = clone_repository(&config, force);
        assert!(result.is_ok());
        assert!(!sentinel.exists());
    }
}
