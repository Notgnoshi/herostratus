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

pub fn find_local_repository2<P: AsRef<Path> + std::fmt::Debug>(
    path: P,
) -> eyre::Result<gix::Repository> {
    tracing::debug!("Searching local path {path:?} for a Git repository");
    let repo = gix::discover(path)?;
    tracing::debug!("Found local Git repository at {:?}", repo.path());
    Ok(repo)
}

// ssh://git@example.com/path.git           => path.git
// git@github.com:Notgnoshi/herostratus.git => Notgnoshi/herostratus.git
// https://example.com/foo                  => foo
// domain:path                              => path
pub fn parse_path_from_url(url: &str) -> eyre::Result<PathBuf> {
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

pub fn get_clone_path(data_dir: &Path, url: &str) -> eyre::Result<PathBuf> {
    let clone_path = parse_path_from_url(url).wrap_err("Failed to parse clone path from URL")?;
    let clone_path = data_dir.join("git").join(clone_path);
    Ok(clone_path)
}

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

pub fn fetch_remote(
    config: &crate::config::RepositoryConfig,
    repo: &git2::Repository,
) -> eyre::Result<usize> {
    let mut remote = repo.find_remote("origin")?;
    assert_eq!(
        remote.url().unwrap_or_default(),
        config.url.as_str(),
        "RepositoryConfig and remote 'origin' don't agree on the URL"
    );
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
    let after = reference.peel_to_commit()?;

    let mut new_commits: usize = 0;
    if before.is_some() && before.as_ref().unwrap().id() == after.id() {
        tracing::debug!("... done. No new commits");
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
        tracing::debug!("... done. {new_commits} new commits");
    }

    Ok(new_commits)
}

pub fn fetch_remote2(
    config: &crate::config::RepositoryConfig,
    repo: &gix::Repository,
) -> eyre::Result<usize> {
    let remote = repo.find_remote("origin")?;
    assert_eq!(
        remote
            .url(gix::remote::Direction::Fetch)
            .unwrap()
            .to_bstring(),
        config.url.as_str(),
        "RepositoryConfig and remote 'origin' don't agree on the URL"
    );
    let reference_name = config.branch.as_deref().unwrap_or("HEAD");
    // TODO: Fetch just the specified branch, not all of them
    // // If this is the first time this reference is being fetched, fetch it like
    // //     git fetch origin branch:branch
    // // which updates the local branch to match the remote
    // let fetch_reference_name = if reference_name != "HEAD" {
    //     format!("{reference_name}:{reference_name}")
    // } else {
    //     reference_name.to_string()
    // };

    let before = repo.rev_parse_single(reference_name).ok();

    // TODO: Need to figure out how to override HTTPS/SSH default details. Is this actually
    // important? In what scenarios is a user going to try to run Herostratus on a repository they
    // can't 'git clone'?
    let connection = remote.connect(gix::remote::Direction::Fetch)?;
    let options = gix::remote::ref_map::Options::default();
    let prepare = connection.prepare_fetch(gix::progress::Discard, options)?;
    let interrupt = std::sync::atomic::AtomicBool::new(false);
    let _outcome = prepare.receive(gix::progress::Discard, &interrupt)?;

    let after = repo.rev_parse_single(reference_name)?;

    let mut new_commits: usize = 0;
    if before.is_some() && before.as_ref().unwrap().detach() == after.detach() {
        tracing::debug!("... done. No new commits");
    } else {
        let commits = crate::git::rev::walk2(after.detach(), repo)?;
        for commit_id in commits {
            if let Some(before) = &before {
                if commit_id? == before.detach() {
                    break;
                }
            }
            new_commits += 1;
        }
        tracing::debug!("... done. {new_commits} new commits");
    }

    Ok(new_commits)
}

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
                fetch_remote(config, &existing_repo)?;

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
    #[cfg_attr(feature = "ci", ignore = "Requires .gitconfig not available in CI")]
    fn test_find_local_repository() {
        let temp_repo = fixtures::repository::simplest2().unwrap();

        let repo = find_local_repository2(temp_repo.tempdir.path()).unwrap();
        assert_eq!(repo.path(), temp_repo.repo.path());
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
    #[cfg_attr(not(feature = "ci"), ignore = "Slow; performs fetch")]
    fn required_fetch_remote() {
        // this is a workspace crate, so its tests are *not* run from the workspace root, rather
        // from the workspace member.
        let this = find_local_repository("..").unwrap();
        // NOTE: There's awkard duplication between the RepositoryConfig, and the repository
        // remotes. This is because the same RepositoryConfig is used to clone the repository as is
        // used to fetch.
        let remote = this.find_remote("origin").unwrap();
        let config = crate::config::RepositoryConfig {
            branch: Some("main".to_string()),
            url: remote.url().unwrap().to_string(),
            ..Default::default()
        };
        fetch_remote(&config, &this).unwrap(); // assert that fetching doesn't fail

        let this = find_local_repository2("..").unwrap();
        fetch_remote2(&config, &this).unwrap(); // assert that fetching doesn't fail
    }

    #[test]
    #[cfg_attr(not(feature = "ci"), ignore = "Slow; performs fetch")]
    fn test_fetch_remote_branch_doesnt_exist() {
        let this = find_local_repository("..").unwrap();
        let remote = this.find_remote("origin").unwrap();
        let config = crate::config::RepositoryConfig {
            branch: Some("THIS_BRANCH_DOESNT_EXIST".to_string()),
            url: remote.url().unwrap().to_string(),
            ..Default::default()
        };
        let result = fetch_remote(&config, &this);
        assert!(result.is_err());

        let this = find_local_repository2("..").unwrap();
        let result = fetch_remote2(&config, &this);
        assert!(result.is_err());
    }

    #[test]
    fn test_fast_fetch_single_ref() {
        let (upstream, downstream) = fixtures::repository::upstream_downstream().unwrap();
        // TODO
    }

    #[test]
    fn test_fetch_remote_branch_creates_or_updates_local_branch() {
        let (upstream, downstream) = fixtures::repository::upstream_downstream().unwrap();
        // TODO
    }

    #[test]
    fn test_fetch_remote_branch_that_doesnt_exist_locally() {
        let (upstream, downstream) = fixtures::repository::upstream_downstream().unwrap();
        let upstream_commit =
            fixtures::repository::add_empty_commit(&upstream.repo, "First upstream commit")
                .unwrap();
        fixtures::repository::add_empty_commit(&downstream.repo, "First downstream commit")
            .unwrap();

        let remote = downstream.repo.find_remote("origin").unwrap();
        let config = crate::config::RepositoryConfig {
            branch: Some("HEAD".to_string()),
            url: remote.url().unwrap().to_string(),
            ..Default::default()
        };
        let result = downstream.repo.find_commit(upstream_commit.id());
        assert!(result.is_err()); // can't find the upstream commit until you fetch
        fetch_remote(&config, &downstream.repo).unwrap();
        let result = downstream.repo.find_commit(upstream_commit.id());
        assert!(result.is_ok());
    }

    #[test]
    fn test_fetch_remote_branch_that_doesnt_exist_locally2() {
        let (upstream, downstream) = fixtures::repository::upstream_downstream2().unwrap();
        let upstream_commit =
            fixtures::repository::add_empty_commit2(&upstream.repo, "First upstream commit")
                .unwrap();
        fixtures::repository::add_empty_commit2(&downstream.repo, "First downstream commit")
            .unwrap();

        let remote = downstream.repo.find_remote("origin").unwrap();
        let config = crate::config::RepositoryConfig {
            branch: Some("HEAD".to_string()),
            url: remote
                .url(gix::remote::Direction::Fetch)
                .unwrap()
                .to_string(),
            ..Default::default()
        };
        let result = downstream.repo.find_commit(upstream_commit.id());
        assert!(result.is_err()); // can't find the upstream commit until you fetch
        fetch_remote2(&config, &downstream.repo).unwrap();
        let result = downstream.repo.find_commit(upstream_commit.id());
        assert!(result.is_ok());
    }
}
