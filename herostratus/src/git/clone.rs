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

fn fetch_options(config: &crate::config::RepositoryConfig) -> git2::FetchOptions<'_> {
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
    debug_assert!(repo.is_bare());
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
            if let Some(before) = &before
                && commit_id? == before.id()
            {
                break;
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

/// After fetching, update the local repository to match the remote for the fetched references.
fn update_local_repo(
    repo: &gix::Repository,
    local_ref_name: &str,
    remote_ref_map: &gix::remote::fetch::RefMap,
) -> eyre::Result<()> {
    for remote_ref in &remote_ref_map.remote_refs {
        sync_remote_ref(repo, local_ref_name, remote_ref)?;
    }

    Ok(())
}

fn sync_remote_symref(
    repo: &gix::Repository,
    full_ref_name: &gix::bstr::BStr,
    target: &gix::bstr::BStr,
    object: gix::ObjectId,
) -> eyre::Result<()> {
    tracing::debug!("Syncing symref {full_ref_name:?} -> {target:?} -> {object:?}");

    // If the target does exist, update it
    if let Some(mut local_ref) = repo.try_find_reference(target)? {
        tracing::debug!("Updating {target:?} -> {object:?}");
        local_ref.set_target_id(
            object,
            format!("Herostratus: Updating {target:?} -> {object:?}"),
        )?;
    }
    // If the target doesn't exist, create it
    else {
        tracing::debug!("Creating {target:?} -> {object:?}");
        repo.reference(
            target,
            object,
            gix::refs::transaction::PreviousValue::Any,
            format!("Herostratus: Creating {target:?} -> {object:?}"),
        )?;
    }

    // Now update the symbolic ref itself to point to the target
    let local_head = gix::refs::FullName::try_from(full_ref_name)?;
    let new_target = gix::refs::FullName::try_from(target)?;

    let change = gix::refs::transaction::Change::Update {
        log: gix::refs::transaction::LogChange::default(),
        expected: gix::refs::transaction::PreviousValue::Any,
        new: gix::refs::Target::Symbolic(new_target),
    };

    let edit = gix::refs::transaction::RefEdit {
        change,
        name: local_head,
        deref: false,
    };

    repo.edit_reference(edit)?;
    Ok(())
}

fn sync_remote_dirref(
    repo: &gix::Repository,
    full_ref_name: &gix::bstr::BStr,
    object: gix::ObjectId,
) -> eyre::Result<()> {
    tracing::debug!("Syncing ref {full_ref_name:?} -> {object:?}");
    let local_head = gix::refs::FullName::try_from(full_ref_name)?;

    let change = gix::refs::transaction::Change::Update {
        log: gix::refs::transaction::LogChange::default(),
        expected: gix::refs::transaction::PreviousValue::Any,
        new: gix::refs::Target::Object(object),
    };

    let edit = gix::refs::transaction::RefEdit {
        change,
        name: local_head,
        deref: false,
    };

    repo.edit_reference(edit)?;
    Ok(())
}

/// Update the local HEAD to match the remote HEAD
fn sync_remote_ref(
    repo: &gix::Repository,
    local_ref_name: &str,
    remote_ref: &gix::protocol::handshake::Ref,
) -> eyre::Result<()> {
    match remote_ref {
        gix::protocol::handshake::Ref::Symbolic {
            full_ref_name,
            target,
            object,
            ..
        } => {
            if full_ref_name.ends_with(local_ref_name.as_bytes()) || full_ref_name == "HEAD" {
                sync_remote_symref(repo, full_ref_name.as_ref(), target.as_ref(), *object)?;
            }
        }
        gix::protocol::handshake::Ref::Peeled {
            full_ref_name,
            object,
            ..
        }
        | gix::protocol::handshake::Ref::Direct {
            full_ref_name,
            object,
        } => {
            if full_ref_name.ends_with(local_ref_name.as_bytes()) {
                sync_remote_dirref(repo, full_ref_name.as_ref(), *object)?;
            }
        }
        gix::protocol::handshake::Ref::Unborn {
            full_ref_name,
            target,
        } => {
            tracing::error!("Refusing to sync unborn ref {full_ref_name:?} -> {target:?}");
        }
    }
    Ok(())
}

/// Count number of commits between two revs
fn count_commits_between(
    repo: &gix::Repository,
    base: Option<gix::Id>,
    head: gix::Id,
) -> eyre::Result<usize> {
    let mut num_fetched_commits = 0;
    if base == Some(head) {
        tracing::debug!("No new commits");
    } else {
        let commits = crate::git::rev::walk_gix(head.object()?.id, repo)?;
        for commit_id in commits {
            if let Some(before) = &base
                && commit_id? == before.object()?.id
            {
                break;
            }
            num_fetched_commits += 1;
        }
        tracing::debug!("{num_fetched_commits} new commits");
    }

    Ok(num_fetched_commits)
}

/// Given a bare repository, pull the given branch from the remote.
///
/// If the branch is not specified, pull the remote's default branch (HEAD).
///
/// Because this function only runs against repositories managed by Herostratus, we can know that
/// the local branch will always be strictly behind the remote branch, so we don't need to merge or
/// rebase to manage merge conflicts. Additionally, because it's a bare repository, there's no work
/// tree to consider either.
///
/// Returns the number of commits pulled.
#[tracing::instrument(level = "debug", skip_all, fields(url = %config.url))]
pub fn pull_branch_gix(
    config: &crate::config::RepositoryConfig,
    repo: &gix::Repository,
) -> eyre::Result<usize> {
    debug_assert!(repo.is_bare());
    // TODO: Handle non-origin remotes (#71)
    let remote = repo.find_remote("origin")?;
    // Can't be a Vec<String>; has to be a Vec<&str> ...
    let mut refspecs = vec!["+HEAD:refs/remotes/origin/HEAD"];
    let branch_refspec;
    if let Some(branch) = &config.branch {
        branch_refspec = format!("+refs/heads/{branch}:refs/heads/{branch}");
        refspecs.push(&branch_refspec);
    }
    let ref_name = config.branch.as_deref().unwrap_or("HEAD");
    let remote = remote.with_refspecs(&refspecs, gix::remote::Direction::Fetch)?;
    tracing::info!("Pulling {ref_name:?} from remote {:?}", config.url);
    tracing::debug!("refspecs: {refspecs:?}");
    // If this is None, then the reference doesn't exist locally (yet), and this is the first time
    // we're pulling it.
    let before = repo.rev_parse_single(ref_name).ok();

    // TODO: HTTPS/SSH auth
    let connection = remote.connect(gix::remote::Direction::Fetch)?;
    let options = gix::remote::ref_map::Options::default();
    // TODO: Handle fetch progress nicely?
    let prepare = connection.prepare_fetch(gix::progress::Discard, options)?;
    let interrupt = std::sync::atomic::AtomicBool::new(false);
    let outcome = prepare.receive(gix::progress::Discard, &interrupt)?;

    update_local_repo(repo, ref_name, &outcome.ref_map)?;
    let after = repo.rev_parse_single(ref_name)?;

    let num_fetched_commits = count_commits_between(repo, before, after)?;
    tracing::info!("Pulled {num_fetched_commits} new commits for {ref_name:?}");
    Ok(num_fetched_commits)
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
        assert_eq!(repo.path(), temp_repo.tempdir.path());

        let repo = find_local_repository_gix(temp_repo.tempdir.path()).unwrap();
        assert_eq!(repo.path(), temp_repo.tempdir.path());
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
    fn test_pull_default_branch_from_empty() {
        let (upstream, downstream) = fixtures::repository::upstream_downstream_empty().unwrap();
        let commit1 = fixtures::repository::add_empty_commit(&upstream.repo, "commit1").unwrap();
        let commit2 = fixtures::repository::add_empty_commit(&upstream.repo, "commit2").unwrap();

        let remote = downstream.repo.find_remote("origin").unwrap();
        let config = crate::config::RepositoryConfig {
            branch: None, // HEAD
            url: remote.url().unwrap().to_string(),
            ..Default::default()
        };
        let repo = downstream.gix();

        // Try to find the upstream commit in the downstream repository. Can't find it, because the
        // remote hasn't been fetched from yet.
        let result = downstream.repo.find_commit(commit1.id());
        assert!(result.is_err());
        let result = downstream.repo.find_commit(commit2.id());
        assert!(result.is_err());

        let fetched_commits = pull_branch_gix(&config, &repo).unwrap();
        assert_eq!(fetched_commits, 2);

        // Now that we pulled, we can find the commits
        let result = downstream.repo.find_commit(commit1.id());
        assert!(result.is_ok());
        let result = downstream.repo.find_commit(commit2.id());
        assert!(result.is_ok());

        let downstream_head = downstream.repo.head().unwrap();
        let upstream_head = upstream.repo.head().unwrap();
        assert_eq!(
            downstream_head.name().unwrap(),
            upstream_head.name().unwrap()
        );
        assert_eq!(downstream_head.target().unwrap(), commit2.id());

        let commit3 = fixtures::repository::add_empty_commit(&upstream.repo, "commit3").unwrap();
        let fetched_commits = pull_branch_gix(&config, &repo).unwrap();
        assert_eq!(fetched_commits, 1);

        let result = downstream.repo.find_commit(commit3.id());
        assert!(result.is_ok());

        let downstream_head = downstream.repo.head().unwrap();
        let upstream_head = upstream.repo.head().unwrap();
        assert_eq!(
            downstream_head.name().unwrap(),
            upstream_head.name().unwrap()
        );
        assert_eq!(downstream_head.target().unwrap(), commit3.id());
    }

    #[test]
    fn test_pull_custom_default_branch_name() {
        let (upstream, downstream) = fixtures::repository::upstream_downstream_empty().unwrap();
        fixtures::repository::switch_branch(&upstream.repo, "trunk").unwrap();

        let commit1 = fixtures::repository::add_empty_commit(&upstream.repo, "commit1").unwrap();
        let commit2 = fixtures::repository::add_empty_commit(&upstream.repo, "commit2").unwrap();

        let remote = downstream.repo.find_remote("origin").unwrap();
        let config = crate::config::RepositoryConfig {
            branch: None, // HEAD
            url: remote.url().unwrap().to_string(),
            ..Default::default()
        };
        let repo = downstream.gix();

        let fetched_commits = pull_branch_gix(&config, &repo).unwrap();
        assert_eq!(fetched_commits, 2);

        let result = downstream.repo.find_commit(commit1.id());
        assert!(result.is_ok());
        let result = downstream.repo.find_commit(commit2.id());
        assert!(result.is_ok());

        let downstream_head = downstream.repo.head().unwrap();
        let upstream_head = upstream.repo.head().unwrap();
        assert_eq!(
            downstream_head.name().unwrap(),
            upstream_head.name().unwrap()
        );
        assert_eq!(downstream_head.target().unwrap(), commit2.id());

        let commit3 = fixtures::repository::add_empty_commit(&upstream.repo, "commit3").unwrap();
        let fetched_commits = pull_branch_gix(&config, &repo).unwrap();
        assert_eq!(fetched_commits, 1);

        let result = downstream.repo.find_commit(commit3.id());
        assert!(result.is_ok());

        let downstream_head = downstream.repo.head().unwrap();
        let upstream_head = upstream.repo.head().unwrap();
        assert_eq!(
            downstream_head.name().unwrap(),
            upstream_head.name().unwrap()
        );
        assert_eq!(downstream_head.target().unwrap(), commit3.id());
    }

    #[test]
    fn test_pull_specific_branch() {
        let (upstream, downstream) = fixtures::repository::upstream_downstream_empty().unwrap();
        fixtures::repository::switch_branch(&upstream.repo, "dev").unwrap();

        let commit1 = fixtures::repository::add_empty_commit(&upstream.repo, "commit1").unwrap();
        let commit2 = fixtures::repository::add_empty_commit(&upstream.repo, "commit2").unwrap();

        let remote = downstream.repo.find_remote("origin").unwrap();
        let config = crate::config::RepositoryConfig {
            branch: Some("dev".to_string()),
            url: remote.url().unwrap().to_string(),
            ..Default::default()
        };
        let repo = downstream.gix();

        let fetched_commits = pull_branch_gix(&config, &repo).unwrap();
        assert_eq!(fetched_commits, 2);

        let result = downstream.repo.find_commit(commit1.id());
        assert!(result.is_ok());
        let result = downstream.repo.find_commit(commit2.id());
        assert!(result.is_ok());

        let downstream_head = downstream.repo.head().unwrap();
        let upstream_head = upstream.repo.head().unwrap();
        assert_eq!(
            downstream_head.name().unwrap(),
            upstream_head.name().unwrap()
        );
        assert_eq!(downstream_head.target().unwrap(), commit2.id());

        let commit3 = fixtures::repository::add_empty_commit(&upstream.repo, "commit3").unwrap();
        let fetched_commits = pull_branch_gix(&config, &repo).unwrap();
        assert_eq!(fetched_commits, 1);

        let result = downstream.repo.find_commit(commit3.id());
        assert!(result.is_ok());

        let downstream_head = downstream.repo.head().unwrap();
        let upstream_head = upstream.repo.head().unwrap();
        assert_eq!(
            downstream_head.name().unwrap(),
            upstream_head.name().unwrap()
        );
        assert_eq!(downstream_head.target().unwrap(), commit3.id());
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
        let repo = downstream.gix();

        let result = pull_branch_gix(&config, &repo);
        assert!(result.is_ok());

        let result = repo.find_reference("branch1");
        assert!(result.is_ok());
    }

    #[test]
    fn test_fast_fetch_single_reference() {
        let (upstream, downstream) = fixtures::repository::upstream_downstream().unwrap();
        fixtures::repository::switch_branch(&upstream.repo, "branch1").unwrap();
        fixtures::repository::switch_branch(&upstream.repo, "branch2").unwrap();
        let commit2 =
            fixtures::repository::add_empty_commit(&upstream.repo, "commit on branch2").unwrap();

        fixtures::repository::switch_branch(&upstream.repo, "branch1").unwrap();
        fixtures::repository::add_empty_commit(&upstream.repo, "commit on branch1").unwrap();

        let upstream = upstream.gix();
        let result = upstream.find_reference("branch1");
        assert!(result.is_ok());
        let result = upstream.find_reference("branch2");
        assert!(result.is_ok());

        let remote = downstream.repo.find_remote("origin").unwrap();
        let config = crate::config::RepositoryConfig {
            branch: Some("branch1".to_string()),
            url: remote.url().unwrap().to_string(),
            ..Default::default()
        };

        let repo = downstream.gix();

        let result = pull_branch_gix(&config, &repo);
        assert!(result.is_ok());

        // We find the branch1 that was fetched
        let result = repo.find_reference("branch1");
        assert!(result.is_ok());

        // But branch2 wasn't fetched
        let result = repo.find_reference("branch2");
        assert!(result.is_err());

        // And the commit on branch2 wasn't fetched
        let result = downstream.repo.find_commit(commit2.id());
        assert!(result.is_err());
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
