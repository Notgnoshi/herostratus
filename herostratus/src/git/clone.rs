use std::path::{Path, PathBuf};

use eyre::WrapErr;

use crate::bstr::{BStr, BString};

/// Default number of commits to fetch when performing a shallow clone.
///
/// Also used as the batch size when deepening a shallow repository.
pub const DEFAULT_SHALLOW_DEPTH: usize = 50;

pub fn find_local_repository<P: AsRef<Path> + std::fmt::Debug>(
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
    full_ref_name: &BStr,
    target: &BStr,
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
    full_ref_name: &BStr,
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
) -> eyre::Result<u64> {
    let mut num_fetched_commits = 0;
    if base == Some(head) {
        tracing::debug!("No new commits");
    } else {
        let commits = crate::git::rev::walk(head.object()?.id, repo)?;
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

/// Build a `core.sshCommand` value that uses the given private key.
fn ssh_command_for_key(key_path: &Path) -> String {
    format!("ssh -i {} -o IdentitiesOnly=yes", key_path.display())
}

/// Apply HTTPS credentials from the config to a connection, if configured.
///
/// If [https_password](crate::config::RepositoryConfig::https_password) is set, the connection will
/// use it (along with [remote_username](crate::config::RepositoryConfig::remote_username)) instead
/// of the default credential helper cascade.
fn apply_https_credentials<'a, 'repo, T>(
    config: &crate::config::RepositoryConfig,
    connection: gix::remote::Connection<'a, 'repo, T>,
) -> eyre::Result<gix::remote::Connection<'a, 'repo, T>>
where
    T: gix::protocol::transport::client::blocking_io::Transport,
{
    let Some(password) = &config.https_password else {
        return Ok(connection);
    };
    let username = config.remote_username.as_deref().ok_or_else(|| {
        eyre::eyre!(
            "https_password is set but remote_username is not; \
             both are required for HTTPS authentication"
        )
    })?;
    let username = username.to_string();
    let password = password.clone();
    tracing::debug!("Using configured HTTPS credentials (username={username:?})");
    #[allow(clippy::result_large_err)] // Err type is defined by gix, not us
    Ok(connection.with_credentials(move |action| match action {
        gix::credentials::helper::Action::Get(ctx) => {
            Ok(Some(gix::credentials::protocol::Outcome {
                identity: gix::sec::identity::Account {
                    username: username.clone(),
                    password: password.clone(),
                    oauth_refresh_token: None,
                },
                next: gix::credentials::helper::NextAction::from(ctx),
            }))
        }
        _ => Ok(None),
    }))
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
pub fn pull_branch(
    config: &crate::config::RepositoryConfig,
    repo: &mut gix::Repository,
) -> eyre::Result<u64> {
    debug_assert!(repo.is_bare());

    // Configure SSH key if provided, before connecting.
    if let Some(key_path) = &config.ssh_private_key {
        let ssh_cmd = ssh_command_for_key(key_path);
        tracing::debug!("Using configured SSH key: {key_path:?}");
        let mut snapshot = repo.config_snapshot_mut();
        snapshot.set_value(
            &gix::config::tree::Core::SSH_COMMAND,
            BStr::new(ssh_cmd.as_bytes()),
        )?;
        let _ = snapshot.commit()?;
    }

    // TODO: Handle non-origin remotes (#71)
    let remote = repo.find_remote("origin")?;
    // Can't be a Vec<String>; has to be a Vec<&str> ...
    let mut refspecs = vec!["+HEAD:refs/remotes/origin/HEAD"];
    let branch_refspec;
    if let Some(branch) = &config.reference {
        branch_refspec = format!("+refs/heads/{branch}:refs/heads/{branch}");
        refspecs.push(&branch_refspec);
    }
    let ref_name = config.reference.as_deref().unwrap_or("HEAD");
    let remote = remote.with_refspecs(&refspecs, gix::remote::Direction::Fetch)?;
    tracing::info!("Pulling {ref_name:?} from remote {:?}", config.url);
    tracing::debug!("refspecs: {refspecs:?}");
    // If this is None, then the reference doesn't exist locally (yet), and this is the first time
    // we're pulling it.
    let before = repo.rev_parse_single(ref_name).ok();

    let connection = remote.connect(gix::remote::Direction::Fetch)?;
    let connection = apply_https_credentials(config, connection)?;
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

/// Deepen a shallow repository by fetching `depth` more commits from the remote.
///
/// Returns `true` if new commits were fetched, `false` if no new history was available.
/// The caller should use [shallow_commits](gix::Repository::shallow_commits) before and after
/// to determine the old boundary OIDs for walking newly available commits.
#[tracing::instrument(level = "debug", skip_all, fields(url = %config.url, depth))]
pub fn deepen(
    config: &crate::config::RepositoryConfig,
    repo: &mut gix::Repository,
    depth: usize,
) -> eyre::Result<bool> {
    debug_assert!(repo.is_bare());

    let boundary_before: Vec<gix::ObjectId> = repo
        .shallow_commits()?
        .map(|commits| commits.iter().copied().collect())
        .unwrap_or_default();

    // Configure SSH key if provided, before connecting.
    if let Some(key_path) = &config.ssh_private_key {
        let ssh_cmd = ssh_command_for_key(key_path);
        tracing::debug!("Using configured SSH key: {key_path:?}");
        let mut snapshot = repo.config_snapshot_mut();
        snapshot.set_value(
            &gix::config::tree::Core::SSH_COMMAND,
            BStr::new(ssh_cmd.as_bytes()),
        )?;
        let _ = snapshot.commit()?;
    }

    let ref_name = config.reference.as_deref().unwrap_or("HEAD");
    let remote = repo.find_remote("origin")?;
    let mut refspecs = vec!["+HEAD:refs/remotes/origin/HEAD"];
    let branch_refspec;
    if let Some(branch) = &config.reference {
        branch_refspec = format!("+refs/heads/{branch}:refs/heads/{branch}");
        refspecs.push(&branch_refspec);
    }
    let remote = remote.with_refspecs(&refspecs, gix::remote::Direction::Fetch)?;
    tracing::info!(
        "Deepening {ref_name:?} by {depth} commits from {:?}",
        config.url
    );
    tracing::debug!("refspecs: {refspecs:?}");

    let connection = remote.connect(gix::remote::Direction::Fetch)?;
    let connection = apply_https_credentials(config, connection)?;
    let options = gix::remote::ref_map::Options::default();
    let prepare = connection.prepare_fetch(gix::progress::Discard, options)?;
    let prepare = prepare.with_shallow(gix::remote::fetch::Shallow::Deepen(depth as u32));
    let interrupt = std::sync::atomic::AtomicBool::new(false);
    let outcome = prepare.receive(gix::progress::Discard, &interrupt)?;

    update_local_repo(repo, ref_name, &outcome.ref_map)?;

    let boundary_after: Vec<gix::ObjectId> = repo
        .shallow_commits()?
        .map(|commits| commits.iter().copied().collect())
        .unwrap_or_default();

    let fetched = boundary_before != boundary_after;
    if fetched {
        tracing::info!(
            "Deepened: boundary changed from {} to {} commits",
            boundary_before.len(),
            boundary_after.len()
        );
    } else {
        tracing::info!("No new commits fetched (boundary unchanged)");
    }
    Ok(fetched)
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
#[tracing::instrument(level = "debug", skip_all, fields(url = %config.url))]
pub fn clone_repository(
    config: &crate::config::RepositoryConfig,
    force: bool,
    shallow: Option<usize>,
) -> eyre::Result<gix::Repository> {
    let start = std::time::Instant::now();
    tracing::info!(
        "Cloning {:?} (ref={}) to {} ...",
        config.url,
        config.reference.as_deref().unwrap_or("HEAD"),
        config.path.display()
    );
    let parent_dir = config.path.parent().ok_or(eyre::eyre!(
        "Failed to determine parent directory of {}",
        config.path.display()
    ))?;
    if !parent_dir.exists() {
        std::fs::create_dir_all(parent_dir)?;
    }

    if config.path.exists() {
        tracing::warn!("{} already exists ...", config.path.display());
        if force {
            tracing::info!("Deleting and overwriting {} ...", config.path.display());
            remove_dir_contents(&config.path).wrap_err(format!(
                "Failed to clone {:?} to {}",
                config.url,
                config.path.display()
            ))?;
            // Proceed to clone as normal
        } else {
            // Check the existing checkout's remote URL; if it matches, do a pull instead of a clone
            let mut existing_repo = gix::discover(&config.path)?;
            let remote = existing_repo.find_remote("origin")?;
            let existing_url = remote
                .url(gix::remote::Direction::Fetch)
                .ok_or(eyre::eyre!("Failed to find remote.origin.url"))?;
            if existing_url.to_string() == config.url {
                tracing::info!("... URLs match; using existing checkout and pulling");
                pull_branch(config, &mut existing_repo)?;
                return Ok(existing_repo);
            }
            eyre::bail!(
                "{} already exists with a different remote URL: {existing_url:?}",
                config.path.display()
            );
        }
    }

    let url = gix::Url::from_bytes(BString::from(config.url.as_bytes()).as_ref())?;
    let create_opts = gix::create::Options {
        destination_must_be_empty: true,
        ..Default::default()
    };
    let open_opts = gix::open::Options::default();
    let prepare = gix::clone::PrepareFetch::new(
        url,
        &config.path,
        gix::create::Kind::Bare,
        create_opts,
        open_opts,
    )?;

    let prepare = if let Some(depth) = shallow {
        let depth = std::num::NonZeroU32::new(depth as u32)
            .ok_or_else(|| eyre::eyre!("shallow depth must be > 0"))?;
        tracing::info!("Shallow clone with depth={depth}");
        prepare.with_shallow(gix::remote::fetch::Shallow::DepthAtRemote(depth))
    } else {
        prepare
    };

    let branch = config.reference.clone();

    // Configure the remote with the right refspecs for fetching just the configure branch and the
    // remote's HEAD.
    let mut prepare = prepare.configure_remote(move |mut remote| {
        // Can't be a Vec<String>; has to be a Vec<&str> ...
        let mut refspecs = vec!["+HEAD:refs/remotes/origin/HEAD"];
        let branch_refspec;
        if let Some(branch) = &branch {
            branch_refspec = format!("+refs/heads/{branch}:refs/heads/{branch}");
            refspecs.push(&branch_refspec);
        }
        // By default, gix will set the default wildcard refspec, which would fetch everything. But
        // we want to only fetch what the user configured.
        remote.replace_refspecs(&refspecs, gix::remote::Direction::Fetch)?;
        Ok(remote)
    });

    // Configure SSH key if provided.
    if let Some(key_path) = &config.ssh_private_key {
        let ssh_cmd = ssh_command_for_key(key_path);
        tracing::debug!("Cloning with configured SSH key: {key_path:?}");
        prepare = prepare.with_in_memory_config_overrides([format!("core.sshCommand={ssh_cmd}")]);
    }

    // Configure HTTPS credentials if provided.
    if let Some(password) = &config.https_password {
        let username = config
            .remote_username
            .as_deref()
            .ok_or_else(|| {
                eyre::eyre!(
                    "https_password is set but remote_username is not; \
                     both are required for HTTPS authentication"
                )
            })?
            .to_string();
        let password = password.clone();
        tracing::debug!("Cloning with configured HTTPS credentials (username={username:?})");
        #[allow(clippy::result_large_err)]
        {
            prepare = prepare.configure_connection(move |conn| {
                let username = username.clone();
                let password = password.clone();
                conn.set_credentials(move |action| match action {
                    gix::credentials::helper::Action::Get(ctx) => {
                        Ok(Some(gix::credentials::protocol::Outcome {
                            identity: gix::sec::identity::Account {
                                username: username.clone(),
                                password: password.clone(),
                                oauth_refresh_token: None,
                            },
                            next: gix::credentials::helper::NextAction::from(ctx),
                        }))
                    }
                    _ => Ok(None),
                });
                Ok(())
            });
        }
    }

    let interrupt = std::sync::atomic::AtomicBool::new(false);
    let (repo, _outcome) = prepare.fetch_only(gix::progress::Discard, &interrupt)?;
    let elapsed = start.elapsed();
    tracing::info!("... Finished cloning {:?} after {elapsed:.2?}", config.url);
    Ok(repo)
}

#[cfg(test)]
mod tests {
    use herostratus_tests::fixtures;
    use herostratus_tests::fixtures::repository;

    use super::*;

    #[test]
    fn test_find_local_repository() {
        let temp_repo = repository::Builder::new()
            .commit("Initial commit")
            .build()
            .unwrap();
        let repo = find_local_repository(temp_repo.tempdir.path()).unwrap();
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
        let (upstream, mut downstream) = fixtures::repository::upstream_downstream_empty().unwrap();
        let commit1 = upstream.commit("commit1").create().unwrap();
        let commit2 = upstream.commit("commit2").create().unwrap();

        let url = downstream
            .repo
            .find_remote("origin")
            .unwrap()
            .url(gix::remote::Direction::Fetch)
            .unwrap()
            .to_string();
        let config = crate::config::RepositoryConfig {
            reference: None, // HEAD
            url,
            ..Default::default()
        };

        // Try to find the upstream commit in the downstream repository. Can't find it, because the
        // remote hasn't been fetched from yet.
        assert!(downstream.repo.find_commit(commit1).is_err());
        assert!(downstream.repo.find_commit(commit2).is_err());

        let fetched_commits = pull_branch(&config, &mut downstream.repo).unwrap();
        assert_eq!(fetched_commits, 2);

        // Now that we pulled, we can find the commits
        assert!(downstream.repo.find_commit(commit1).is_ok());
        assert!(downstream.repo.find_commit(commit2).is_ok());

        assert_eq!(
            downstream.repo.head().unwrap().name(),
            upstream.repo.head().unwrap().name()
        );
        assert_eq!(downstream.repo.head().unwrap().id().unwrap(), commit2);

        let commit3 = upstream.commit("commit3").create().unwrap();
        let fetched_commits = pull_branch(&config, &mut downstream.repo).unwrap();
        assert_eq!(fetched_commits, 1);

        assert!(downstream.repo.find_commit(commit3).is_ok());

        assert_eq!(
            downstream.repo.head().unwrap().name(),
            upstream.repo.head().unwrap().name()
        );
        assert_eq!(downstream.repo.head().unwrap().id().unwrap(), commit3);
    }

    #[test]
    fn test_pull_custom_default_branch_name() {
        let (upstream, mut downstream) = fixtures::repository::upstream_downstream_empty().unwrap();
        upstream.set_branch("trunk").unwrap();

        let commit1 = upstream.commit("commit1").create().unwrap();
        let commit2 = upstream.commit("commit2").create().unwrap();

        let url = downstream
            .repo
            .find_remote("origin")
            .unwrap()
            .url(gix::remote::Direction::Fetch)
            .unwrap()
            .to_string();
        let config = crate::config::RepositoryConfig {
            reference: None, // HEAD
            url,
            ..Default::default()
        };

        let fetched_commits = pull_branch(&config, &mut downstream.repo).unwrap();
        assert_eq!(fetched_commits, 2);

        assert!(downstream.repo.find_commit(commit1).is_ok());
        assert!(downstream.repo.find_commit(commit2).is_ok());

        assert_eq!(
            downstream.repo.head().unwrap().name(),
            upstream.repo.head().unwrap().name()
        );
        assert_eq!(downstream.repo.head().unwrap().id().unwrap(), commit2);

        let commit3 = upstream.commit("commit3").create().unwrap();
        let fetched_commits = pull_branch(&config, &mut downstream.repo).unwrap();
        assert_eq!(fetched_commits, 1);

        assert!(downstream.repo.find_commit(commit3).is_ok());

        assert_eq!(
            downstream.repo.head().unwrap().name(),
            upstream.repo.head().unwrap().name()
        );
        assert_eq!(downstream.repo.head().unwrap().id().unwrap(), commit3);
    }

    #[test]
    fn test_pull_specific_branch() {
        let (upstream, mut downstream) = fixtures::repository::upstream_downstream_empty().unwrap();
        upstream.set_branch("dev").unwrap();

        let commit1 = upstream.commit("commit1").create().unwrap();
        let commit2 = upstream.commit("commit2").create().unwrap();

        let url = downstream
            .repo
            .find_remote("origin")
            .unwrap()
            .url(gix::remote::Direction::Fetch)
            .unwrap()
            .to_string();
        let config = crate::config::RepositoryConfig {
            reference: Some("dev".to_string()),
            url,
            ..Default::default()
        };

        let fetched_commits = pull_branch(&config, &mut downstream.repo).unwrap();
        assert_eq!(fetched_commits, 2);

        assert!(downstream.repo.find_commit(commit1).is_ok());
        assert!(downstream.repo.find_commit(commit2).is_ok());

        assert_eq!(
            downstream.repo.head().unwrap().name(),
            upstream.repo.head().unwrap().name()
        );
        assert_eq!(downstream.repo.head().unwrap().id().unwrap(), commit2);

        let commit3 = upstream.commit("commit3").create().unwrap();
        let fetched_commits = pull_branch(&config, &mut downstream.repo).unwrap();
        assert_eq!(fetched_commits, 1);

        assert!(downstream.repo.find_commit(commit3).is_ok());

        assert_eq!(
            downstream.repo.head().unwrap().name(),
            upstream.repo.head().unwrap().name()
        );
        assert_eq!(downstream.repo.head().unwrap().id().unwrap(), commit3);
    }

    #[test]
    fn test_pulling_creates_a_local_branch() {
        let (upstream, mut downstream) = fixtures::repository::upstream_downstream().unwrap();
        upstream.set_branch("branch1").unwrap();
        upstream.commit("commit on branch1").create().unwrap();

        let url = downstream
            .repo
            .find_remote("origin")
            .unwrap()
            .url(gix::remote::Direction::Fetch)
            .unwrap()
            .to_string();
        let config = crate::config::RepositoryConfig {
            reference: Some("branch1".to_string()),
            url,
            ..Default::default()
        };

        assert!(pull_branch(&config, &mut downstream.repo).is_ok());
        assert!(downstream.repo.find_reference("branch1").is_ok());
    }

    #[test]
    fn test_fast_fetch_single_reference() {
        let (upstream, mut downstream) = fixtures::repository::upstream_downstream().unwrap();
        upstream.set_branch("branch1").unwrap();
        upstream.set_branch("branch2").unwrap();
        let commit2 = upstream.commit("commit on branch2").create().unwrap();

        upstream.set_branch("branch1").unwrap();
        upstream.commit("commit on branch1").create().unwrap();

        assert!(upstream.repo.find_reference("branch1").is_ok());
        assert!(upstream.repo.find_reference("branch2").is_ok());

        let url = downstream
            .repo
            .find_remote("origin")
            .unwrap()
            .url(gix::remote::Direction::Fetch)
            .unwrap()
            .to_string();
        let config = crate::config::RepositoryConfig {
            reference: Some("branch1".to_string()),
            url,
            ..Default::default()
        };

        assert!(pull_branch(&config, &mut downstream.repo).is_ok());

        // We find the branch1 that was fetched
        assert!(downstream.repo.find_reference("branch1").is_ok());

        // But branch2 wasn't fetched
        assert!(downstream.repo.find_reference("branch2").is_err());

        // And the commit on branch2 wasn't fetched
        assert!(downstream.repo.find_commit(commit2).is_err());
    }

    #[test]
    fn test_clone_file_url() {
        let upstream = repository::Builder::new()
            .commit("Initial commit")
            .build()
            .unwrap();
        let commit1 = upstream.commit("commit1").create().unwrap();
        let tempdir = tempfile::tempdir().unwrap();
        let downstream_dir = tempdir.path().join("downstream");

        let config = crate::config::RepositoryConfig {
            reference: None, // HEAD
            url: format!("file://{}", upstream.tempdir.path().display()),
            path: downstream_dir.clone(),
            ..Default::default()
        };
        let force = false;
        let downstream = clone_repository(&config, force, None).unwrap();

        let result = downstream.find_commit(commit1);
        assert!(result.is_ok());
    }

    #[test]
    fn test_clone_fast_fetch_single_branch() {
        let upstream = repository::Builder::new()
            .commit("Initial commit")
            .build()
            .unwrap();

        upstream.set_branch("branch1").unwrap();
        let commit1 = upstream.commit("commit1").create().unwrap();
        upstream.set_branch("branch2").unwrap();
        let commit2 = upstream.commit("commit2").create().unwrap();
        upstream.set_branch("branch1").unwrap();
        let tempdir = tempfile::tempdir().unwrap();
        let downstream_dir = tempdir.path().join("downstream");

        let config = crate::config::RepositoryConfig {
            reference: Some("branch1".to_string()),
            url: format!("file://{}", upstream.tempdir.path().display()),
            path: downstream_dir.clone(),
            ..Default::default()
        };
        let force = false;
        let downstream = clone_repository(&config, force, None).unwrap();

        let result = downstream.find_commit(commit1);
        assert!(result.is_ok());

        let result = downstream.find_commit(commit2);
        assert!(result.is_err());
    }

    #[test]
    fn test_clone_https_url() {
        let tempdir = tempfile::tempdir().unwrap();
        let downstream_dir = tempdir.path().join("downstream");

        let config = crate::config::RepositoryConfig {
            reference: Some("test/fixup".into()), // A small branch that's cheaper to fetch than the default
            url: "https://github.com/Notgnoshi/herostratus.git".to_string(),
            path: downstream_dir.clone(),
            ..Default::default()
        };
        let force = false;
        let _downstream = clone_repository(&config, force, None).unwrap();
    }

    #[test]
    #[cfg_attr(feature = "ci", ignore = "Requires SSH (not available in CI)")]
    fn test_clone_ssh_alternative_url() {
        let tempdir = tempfile::tempdir().unwrap();
        let downstream_dir = tempdir.path().join("downstream");

        let config = crate::config::RepositoryConfig {
            reference: Some("test/fixup".into()), // A small branch that's cheaper to fetch than the default
            url: "git@github.com:Notgnoshi/herostratus.git".to_string(),
            path: downstream_dir.clone(),
            ..Default::default()
        };
        let force = false;
        let _downstream = clone_repository(&config, force, None).unwrap();
    }

    #[test]
    #[cfg_attr(feature = "ci", ignore = "Requires SSH (not available in CI)")]
    fn test_clone_ssh_url() {
        let tempdir = tempfile::tempdir().unwrap();
        let downstream_dir = tempdir.path().join("downstream");

        let config = crate::config::RepositoryConfig {
            reference: Some("test/fixup".into()), // A small branch that's cheaper to fetch than the default
            // TODO: git:// protocol times out without cloning anything
            url: "ssh://git@github.com/Notgnoshi/herostratus.git".to_string(),
            path: downstream_dir.clone(),
            ..Default::default()
        };
        let force = false;
        let _downstream = clone_repository(&config, force, None).unwrap();
    }

    #[test]
    fn test_clone_directory_already_exists() {
        let upstream = repository::Builder::new()
            .commit("Initial commit")
            .build()
            .unwrap();

        let tempdir = tempfile::tempdir().unwrap();
        let downstream_dir = tempdir.path().join("downstream");
        std::fs::create_dir_all(&downstream_dir).unwrap();

        let config = crate::config::RepositoryConfig {
            reference: None, // HEAD
            url: format!("file://{}", upstream.tempdir.path().display()),
            path: downstream_dir.clone(),
            ..Default::default()
        };
        let force = false;
        let result = clone_repository(&config, force, None);
        assert!(result.is_err());

        // Create a sentinel file to test whether the directory was deleted, or the clone was just
        // created on top of the existing contents.
        let sentinel = downstream_dir.join("sentinel.txt");
        std::fs::File::create(&sentinel).unwrap();
        assert!(sentinel.exists());

        let force = true;
        let result = clone_repository(&config, force, None);
        assert!(result.is_ok());
        assert!(!sentinel.exists());
    }

    #[test]
    fn test_clone_already_cloned_does_a_fetch() {
        let upstream = repository::Builder::new()
            .commit("Initial commit")
            .build()
            .unwrap();
        let tempdir = tempfile::tempdir().unwrap();
        let downstream_dir = tempdir.path().join("downstream");

        let config = crate::config::RepositoryConfig {
            reference: None, // HEAD
            url: format!("file://{}", upstream.tempdir.path().display()),
            path: downstream_dir.clone(),
            ..Default::default()
        };

        let force = false;
        let _downstream = clone_repository(&config, force, None).unwrap();
        // Create a sentinel file to test whether the directory was deleted
        let sentinel = downstream_dir.join("sentinel.txt");
        std::fs::File::create(&sentinel).unwrap();
        assert!(sentinel.exists());

        // Now add a new commit to the upstream so we can test that another clone does a fetch
        // instead of failing.
        let new_commit = upstream.commit("new commit").create().unwrap();

        let downstream = clone_repository(&config, force, None).unwrap();
        let result = downstream.find_commit(new_commit);
        assert!(result.is_ok());
        // The repo wasn't cleared out and re-cloned; the sentinel file still exists
        assert!(sentinel.exists());
    }

    #[test]
    fn test_shallow_clone() {
        let upstream = repository::Builder::new()
            .commit("commit1")
            .commit("commit2")
            .commit("commit3")
            .commit("commit4")
            .commit("commit5")
            .build()
            .unwrap();
        let tempdir = tempfile::tempdir().unwrap();
        let downstream_dir = tempdir.path().join("downstream");

        let config = crate::config::RepositoryConfig {
            reference: None,
            url: format!("file://{}", upstream.tempdir.path().display()),
            path: downstream_dir.clone(),
            ..Default::default()
        };
        let shallow = Some(2);
        let force = false;
        let downstream = clone_repository(&config, force, shallow).unwrap();

        // Only 2 commits should be reachable
        let head = crate::git::rev::parse("HEAD", &downstream).unwrap();
        let commits: Vec<_> = crate::git::rev::walk(head, &downstream)
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();
        assert_eq!(commits.len(), 2);
    }

    #[test]
    fn test_deepen_returns_false_when_no_more_history() {
        let upstream = repository::Builder::new()
            .commit("commit1")
            .commit("commit2")
            .build()
            .unwrap();
        let tempdir = tempfile::tempdir().unwrap();
        let downstream_dir = tempdir.path().join("downstream");

        let config = crate::config::RepositoryConfig {
            reference: None,
            url: format!("file://{}", upstream.tempdir.path().display()),
            path: downstream_dir.clone(),
            ..Default::default()
        };
        let force = false;
        let mut downstream = clone_repository(&config, force, Some(3)).unwrap();

        // Already have all commits; deepening should return false
        let deepened = deepen(&config, &mut downstream, 4).unwrap();
        assert!(!deepened);
    }

    /// Helper to create a merge commit with multiple parents
    fn create_merge_commit(
        repo: &gix::Repository,
        subject: &str,
        parents: Vec<gix::ObjectId>,
        time: gix::date::SecondsSinceUnixEpoch,
    ) -> eyre::Result<gix::ObjectId> {
        // Use the first parent's tree
        let tree_id = repo.find_commit(parents[0])?.tree_id()?;
        let sig = gix::actor::Signature {
            name: "Herostratus".into(),
            email: "Herostratus@example.com".into(),
            time: gix::date::Time::new(time, 0),
        };
        let mut buf_a = gix::date::parse::TimeBuf::default();
        let authored = sig.to_ref(&mut buf_a);
        let mut buf_c = gix::date::parse::TimeBuf::default();
        let committed = sig.to_ref(&mut buf_c);

        let parent_ids: Vec<gix::Id<'_>> = parents
            .iter()
            .map(|oid| repo.find_commit(*oid).map(|c| c.id()))
            .collect::<Result<_, _>>()?;

        let commit_id =
            repo.commit_as(committed, authored, "HEAD", subject, tree_id, parent_ids)?;
        Ok(commit_id.detach())
    }

    #[test]
    fn test_deepen_with_merge_commits() {
        // Create a repo with non-linear history:
        //
        //   A(1000) - B(2000) - E(5000, merge B+D)
        //              \       /
        //   C(3000) --- D(4000)
        //
        // C is on a feature branch rooted at B.
        let upstream = repository::Builder::new()
            .commit("A")
            .time(1000)
            .commit("B")
            .time(2000)
            .build()
            .unwrap();

        // Create feature branch from B
        let b_oid = upstream.repo.head_commit().unwrap().id().detach();
        upstream.set_branch("feature").unwrap();
        upstream.commit("C").time(3000).create().unwrap();
        let d_oid = upstream.commit("D").time(4000).create().unwrap().detach();

        // Switch back to main and create merge commit
        upstream.set_branch("main").unwrap();
        let _merge_oid =
            create_merge_commit(&upstream.repo, "E (merge)", vec![b_oid, d_oid], 5000).unwrap();

        // Verify upstream has 5 reachable commits
        let head = crate::git::rev::parse("HEAD", &upstream.repo).unwrap();
        let upstream_commits: Vec<_> = crate::git::rev::walk(head, &upstream.repo)
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();
        assert_eq!(
            upstream_commits.len(),
            5,
            "Upstream should have 5 commits: A, B, C, D, E"
        );

        // Shallow clone with depth=2
        let tempdir = tempfile::tempdir().unwrap();
        let downstream_dir = tempdir.path().join("downstream");
        let config = crate::config::RepositoryConfig {
            reference: None,
            url: format!("file://{}", upstream.tempdir.path().display()),
            path: downstream_dir.clone(),
            ..Default::default()
        };
        let mut downstream = clone_repository(&config, false, Some(2)).unwrap();

        assert!(downstream.is_shallow());

        let head = crate::git::rev::parse("HEAD", &downstream).unwrap();
        let initial_commits: Vec<_> = crate::git::rev::walk(head, &downstream)
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();

        // Read boundary before deepening -- these are the OIDs whose parents are missing
        let boundary_before = downstream.shallow_commits().unwrap().unwrap();
        let boundary_oids: Vec<gix::ObjectId> = boundary_before.iter().copied().collect();
        // With a merge at HEAD and depth=2, there should be 2 boundary commits
        // (one per parent chain through the merge)
        assert_eq!(boundary_oids.len(), 2);

        // Deepen by 2
        let deepened = deepen(&config, &mut downstream, 2).unwrap();
        assert!(deepened);

        // Walk from the old boundary OIDs to pick up newly available commits
        let walk = downstream.rev_walk(boundary_oids.clone());
        let walk = walk.sorting(gix::revision::walk::Sorting::ByCommitTime(
            gix::traverse::commit::simple::CommitTimeOrder::NewestFirst,
        ));
        let new_commits: Vec<_> = walk
            .all()
            .unwrap()
            .filter_map(|r| r.ok())
            .map(|info| info.id)
            .filter(|oid| !boundary_oids.contains(oid))
            .collect();

        // Total reachable should be the full history
        let total: Vec<_> = crate::git::rev::walk(head, &downstream)
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();

        // Initial + new from boundary walk should equal total
        let combined = initial_commits.len() + new_commits.len();
        assert_eq!(
            combined,
            total.len(),
            "Initial ({}) + new from boundary walk ({}) should equal total ({})",
            initial_commits.len(),
            new_commits.len(),
            total.len()
        );
    }
}
