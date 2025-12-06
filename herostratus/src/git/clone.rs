use std::path::{Path, PathBuf};

use eyre::WrapErr;

use crate::bstr::{BStr, BString};

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
) -> eyre::Result<usize> {
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
    repo: &gix::Repository,
) -> eyre::Result<usize> {
    debug_assert!(repo.is_bare());
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
#[tracing::instrument(level = "debug", skip_all, fields(url = %config.url))]
pub fn clone_repository(
    config: &crate::config::RepositoryConfig,
    force: bool,
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
            let existing_repo = gix::discover(&config.path)?;
            let remote = existing_repo.find_remote("origin")?;
            let existing_url = remote
                .url(gix::remote::Direction::Fetch)
                .ok_or(eyre::eyre!("Failed to find remote.origin.url"))?;
            if existing_url.to_string() == config.url {
                tracing::info!("... URLs match; using existing checkout and pulling");
                pull_branch(config, &existing_repo)?;
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
    let interrupt = std::sync::atomic::AtomicBool::new(false);
    let (repo, _outcome) = prepare.fetch_only(gix::progress::Discard, &interrupt)?;
    let elapsed = start.elapsed();
    tracing::info!("... Finished cloning {:?} after {elapsed:.2?}", config.url);
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
        let url = remote
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
        let result = downstream.repo.find_commit(commit1);
        assert!(result.is_err());
        let result = downstream.repo.find_commit(commit2);
        assert!(result.is_err());

        let fetched_commits = pull_branch(&config, &downstream.repo).unwrap();
        assert_eq!(fetched_commits, 2);

        // Now that we pulled, we can find the commits
        let result = downstream.repo.find_commit(commit1);
        assert!(result.is_ok());
        let result = downstream.repo.find_commit(commit2);
        assert!(result.is_ok());

        let downstream_head = downstream.repo.head().unwrap();
        let upstream_head = upstream.repo.head().unwrap();
        assert_eq!(downstream_head.name(), upstream_head.name());
        assert_eq!(downstream_head.id().unwrap(), commit2);

        let commit3 = fixtures::repository::add_empty_commit(&upstream.repo, "commit3").unwrap();
        let fetched_commits = pull_branch(&config, &downstream.repo).unwrap();
        assert_eq!(fetched_commits, 1);

        let result = downstream.repo.find_commit(commit3);
        assert!(result.is_ok());

        let downstream_head = downstream.repo.head().unwrap();
        let upstream_head = upstream.repo.head().unwrap();
        assert_eq!(downstream_head.name(), upstream_head.name());
        assert_eq!(downstream_head.id().unwrap(), commit3);
    }

    #[test]
    fn test_pull_custom_default_branch_name() {
        let (upstream, downstream) = fixtures::repository::upstream_downstream_empty().unwrap();
        fixtures::repository::set_default_branch(&upstream.repo, "trunk").unwrap();

        let commit1 = fixtures::repository::add_empty_commit(&upstream.repo, "commit1").unwrap();
        let commit2 = fixtures::repository::add_empty_commit(&upstream.repo, "commit2").unwrap();

        let remote = downstream.repo.find_remote("origin").unwrap();
        let url = remote
            .url(gix::remote::Direction::Fetch)
            .unwrap()
            .to_string();
        let config = crate::config::RepositoryConfig {
            reference: None, // HEAD
            url,
            ..Default::default()
        };

        let fetched_commits = pull_branch(&config, &downstream.repo).unwrap();
        assert_eq!(fetched_commits, 2);

        let result = downstream.repo.find_commit(commit1);
        assert!(result.is_ok());
        let result = downstream.repo.find_commit(commit2);
        assert!(result.is_ok());

        let downstream_head = downstream.repo.head().unwrap();
        let upstream_head = upstream.repo.head().unwrap();
        assert_eq!(downstream_head.name(), upstream_head.name());
        assert_eq!(downstream_head.id().unwrap(), commit2);

        let commit3 = fixtures::repository::add_empty_commit(&upstream.repo, "commit3").unwrap();
        let fetched_commits = pull_branch(&config, &downstream.repo).unwrap();
        assert_eq!(fetched_commits, 1);

        let result = downstream.repo.find_commit(commit3);
        assert!(result.is_ok());

        let downstream_head = downstream.repo.head().unwrap();
        let upstream_head = upstream.repo.head().unwrap();
        assert_eq!(downstream_head.name(), upstream_head.name());
        assert_eq!(downstream_head.id().unwrap(), commit3);
    }

    #[test]
    fn test_pull_specific_branch() {
        let (upstream, downstream) = fixtures::repository::upstream_downstream_empty().unwrap();
        fixtures::repository::set_default_branch(&upstream.repo, "dev").unwrap();

        let commit1 = fixtures::repository::add_empty_commit(&upstream.repo, "commit1").unwrap();
        let commit2 = fixtures::repository::add_empty_commit(&upstream.repo, "commit2").unwrap();

        let remote = downstream.repo.find_remote("origin").unwrap();
        let url = remote
            .url(gix::remote::Direction::Fetch)
            .unwrap()
            .to_string();
        let config = crate::config::RepositoryConfig {
            reference: Some("dev".to_string()),
            url,
            ..Default::default()
        };

        let fetched_commits = pull_branch(&config, &downstream.repo).unwrap();
        assert_eq!(fetched_commits, 2);

        let result = downstream.repo.find_commit(commit1);
        assert!(result.is_ok());
        let result = downstream.repo.find_commit(commit2);
        assert!(result.is_ok());

        let downstream_head = downstream.repo.head().unwrap();
        let upstream_head = upstream.repo.head().unwrap();
        assert_eq!(downstream_head.name(), upstream_head.name());
        assert_eq!(downstream_head.id().unwrap(), commit2);

        let commit3 = fixtures::repository::add_empty_commit(&upstream.repo, "commit3").unwrap();
        let fetched_commits = pull_branch(&config, &downstream.repo).unwrap();
        assert_eq!(fetched_commits, 1);

        let result = downstream.repo.find_commit(commit3);
        assert!(result.is_ok());

        let downstream_head = downstream.repo.head().unwrap();
        let upstream_head = upstream.repo.head().unwrap();
        assert_eq!(downstream_head.name(), upstream_head.name());
        assert_eq!(downstream_head.id().unwrap(), commit3);
    }

    #[test]
    fn test_pulling_creates_a_local_branch() {
        let (upstream, downstream) = fixtures::repository::upstream_downstream().unwrap();
        fixtures::repository::set_default_branch(&upstream.repo, "branch1").unwrap();
        fixtures::repository::add_empty_commit(&upstream.repo, "commit on branch1").unwrap();

        let remote = downstream.repo.find_remote("origin").unwrap();
        let url = remote
            .url(gix::remote::Direction::Fetch)
            .unwrap()
            .to_string();
        let config = crate::config::RepositoryConfig {
            reference: Some("branch1".to_string()),
            url,
            ..Default::default()
        };

        let result = pull_branch(&config, &downstream.repo);
        assert!(result.is_ok());

        let result = downstream.repo.find_reference("branch1");
        assert!(result.is_ok());
    }

    #[test]
    fn test_fast_fetch_single_reference() {
        let (upstream, downstream) = fixtures::repository::upstream_downstream().unwrap();
        fixtures::repository::set_default_branch(&upstream.repo, "branch1").unwrap();
        fixtures::repository::set_default_branch(&upstream.repo, "branch2").unwrap();
        let commit2 =
            fixtures::repository::add_empty_commit(&upstream.repo, "commit on branch2").unwrap();

        fixtures::repository::set_default_branch(&upstream.repo, "branch1").unwrap();
        fixtures::repository::add_empty_commit(&upstream.repo, "commit on branch1").unwrap();

        let result = upstream.repo.find_reference("branch1");
        assert!(result.is_ok());
        let result = upstream.repo.find_reference("branch2");
        assert!(result.is_ok());

        let remote = downstream.repo.find_remote("origin").unwrap();
        let url = remote
            .url(gix::remote::Direction::Fetch)
            .unwrap()
            .to_string();
        let config = crate::config::RepositoryConfig {
            reference: Some("branch1".to_string()),
            url,
            ..Default::default()
        };

        let result = pull_branch(&config, &downstream.repo);
        assert!(result.is_ok());

        // We find the branch1 that was fetched
        let result = downstream.repo.find_reference("branch1");
        assert!(result.is_ok());

        // But branch2 wasn't fetched
        let result = downstream.repo.find_reference("branch2");
        assert!(result.is_err());

        // And the commit on branch2 wasn't fetched
        let result = downstream.repo.find_commit(commit2);
        assert!(result.is_err());
    }

    #[test]
    fn test_clone_file_url() {
        let upstream = fixtures::repository::simplest().unwrap();
        let commit1 = fixtures::repository::add_empty_commit(&upstream.repo, "commit1").unwrap();
        let tempdir = tempfile::tempdir().unwrap();
        let downstream_dir = tempdir.path().join("downstream");

        let config = crate::config::RepositoryConfig {
            reference: None, // HEAD
            url: format!("file://{}", upstream.tempdir.path().display()),
            path: downstream_dir.clone(),
            ..Default::default()
        };
        let force = false;
        let downstream = clone_repository(&config, force).unwrap();

        let result = downstream.find_commit(commit1);
        assert!(result.is_ok());
    }

    #[test]
    fn test_clone_fast_fetch_single_branch() {
        let upstream = fixtures::repository::simplest().unwrap();

        fixtures::repository::set_default_branch(&upstream.repo, "branch1").unwrap();
        let commit1 = fixtures::repository::add_empty_commit(&upstream.repo, "commit1").unwrap();
        fixtures::repository::set_default_branch(&upstream.repo, "branch2").unwrap();
        let commit2 = fixtures::repository::add_empty_commit(&upstream.repo, "commit2").unwrap();
        fixtures::repository::set_default_branch(&upstream.repo, "branch1").unwrap();
        let tempdir = tempfile::tempdir().unwrap();
        let downstream_dir = tempdir.path().join("downstream");

        let config = crate::config::RepositoryConfig {
            reference: Some("branch1".to_string()),
            url: format!("file://{}", upstream.tempdir.path().display()),
            path: downstream_dir.clone(),
            ..Default::default()
        };
        let force = false;
        let downstream = clone_repository(&config, force).unwrap();

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
        let _downstream = clone_repository(&config, force).unwrap();
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
        let _downstream = clone_repository(&config, force).unwrap();
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
        let _downstream = clone_repository(&config, force).unwrap();
    }

    #[test]
    fn test_clone_directory_already_exists() {
        let upstream = fixtures::repository::simplest().unwrap();

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
        let result = clone_repository(&config, force);
        assert!(result.is_err());

        // Create a sentinel file to test whether the directory was deleted, or the clone was just
        // created on top of the existing contents.
        let sentinel = downstream_dir.join("sentinel.txt");
        std::fs::File::create(&sentinel).unwrap();
        assert!(sentinel.exists());

        let force = true;
        let result = clone_repository(&config, force);
        assert!(result.is_ok());
        assert!(!sentinel.exists());
    }

    #[test]
    fn test_clone_already_cloned_does_a_fetch() {
        let upstream = fixtures::repository::simplest().unwrap();
        let tempdir = tempfile::tempdir().unwrap();
        let downstream_dir = tempdir.path().join("downstream");

        let config = crate::config::RepositoryConfig {
            reference: None, // HEAD
            url: format!("file://{}", upstream.tempdir.path().display()),
            path: downstream_dir.clone(),
            ..Default::default()
        };

        let force = false;
        let _downstream = clone_repository(&config, force).unwrap();
        // Create a sentinel file to test whether the directory was deleted
        let sentinel = downstream_dir.join("sentinel.txt");
        std::fs::File::create(&sentinel).unwrap();
        assert!(sentinel.exists());

        // Now add a new commit to the upstream so we can test that another clone does a fetch
        // instead of failing.
        let new_commit =
            fixtures::repository::add_empty_commit(&upstream.repo, "new commit").unwrap();

        let downstream = clone_repository(&config, force).unwrap();
        let result = downstream.find_commit(new_commit);
        assert!(result.is_ok());
        // The repo wasn't cleared out and re-cloned; the sentinel file still exists
        assert!(sentinel.exists());
    }
}
