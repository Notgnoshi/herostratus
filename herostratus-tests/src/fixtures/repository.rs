use tempfile::{Builder, TempDir};

pub struct TempRepository {
    pub tempdir: TempDir,
    pub repo: gix::Repository,
}

impl TempRepository {
    /// Consume the TempDir without deleting the on-disk repository
    ///
    /// You probably don't want to use this in the final state of a test, but it can be useful for
    /// troubleshooting when things aren't working as you think they should.
    pub fn forget(&mut self) {
        self.tempdir.disable_cleanup(true)
    }

    pub fn remember(&mut self) {
        self.tempdir.disable_cleanup(false)
    }
}

impl TempRepository {
    pub fn git2(&self) -> git2::Repository {
        git2::Repository::discover(self.tempdir.path()).unwrap()
    }
}

pub fn add_empty_commit<'r>(repo: &'r gix::Repository, message: &str) -> eyre::Result<gix::Id<'r>> {
    let time = 1711656630;
    add_empty_commit_time(repo, message, time)
}

#[tracing::instrument(level="debug", skip_all, fields(path = %repo.path().display()))]
pub fn add_empty_commit_time<'r>(
    repo: &'r gix::Repository,
    message: &str,
    seconds: gix::date::SecondsSinceUnixEpoch,
) -> eyre::Result<gix::Id<'r>> {
    let time = gix::date::Time { seconds, offset: 0 };
    let signature = gix::actor::Signature {
        name: "Herostratus".into(),
        email: "Herostratus@example.com".into(),
        time,
    };
    let mut buf = gix::date::parse::TimeBuf::default();
    let authored = signature.to_ref(&mut buf);
    let mut buf = gix::date::parse::TimeBuf::default();
    let committed = signature.to_ref(&mut buf);

    let tree_id = repo
        .head_tree_id()
        .unwrap_or_else(|_| repo.empty_tree().id());
    let parent = repo.head_commit().ok();
    let parents = if let Some(ref parent) = parent {
        vec![parent.id()]
    } else {
        Vec::new()
    };
    let commit_id = repo.commit_as(authored, committed, "HEAD", message, tree_id, parents)?;
    Ok(commit_id)
}

pub fn bare() -> eyre::Result<TempRepository> {
    let tempdir = Builder::new().prefix("tmp-").suffix(".git").tempdir()?;
    tracing::debug!(
        "Creating bare repo fixture in '{}'",
        tempdir.path().display()
    );

    let options = gix::create::Options {
        destination_must_be_empty: true,
        ..Default::default()
    };
    let repo = gix::ThreadSafeRepository::init(tempdir.path(), gix::create::Kind::Bare, options)?;
    let repo = repo.to_thread_local();

    Ok(TempRepository { tempdir, repo })
}

pub fn simplest() -> eyre::Result<TempRepository> {
    with_empty_commits(&["Initial commit"])
}

pub fn with_empty_commits(messages: &[&str]) -> eyre::Result<TempRepository> {
    let repo = bare()?;

    for message in messages {
        add_empty_commit(&repo.repo, message)?;
    }

    Ok(repo)
}

/// Return a pair of empty [TempRepository]s with the upstream configured as the "origin" remote of
/// the downstream
pub fn upstream_downstream_empty() -> eyre::Result<(TempRepository, TempRepository)> {
    let upstream = with_empty_commits(&[])?;
    let mut downstream = with_empty_commits(&[])?;
    tracing::debug!(
        "Setting {:?} as upstream remote of {:?}",
        upstream.tempdir.path(),
        downstream.tempdir.path()
    );
    let url = format!("file://{}", upstream.tempdir.path().display());
    let mut config = downstream.repo.config_snapshot_mut();
    config.set_raw_value(&"remote.origin.url", url.as_bytes())?;
    config.commit()?;
    let _remote = downstream.repo.find_remote("origin")?;
    Ok((upstream, downstream))
}

pub fn upstream_downstream() -> eyre::Result<(TempRepository, TempRepository)> {
    let (upstream, downstream) = upstream_downstream_empty()?;
    add_empty_commit(&upstream.repo, "Initial upstream commit")?;
    add_empty_commit(&downstream.repo, "Initial downstream commit")?;
    Ok((upstream, downstream))
}

pub fn create_branch<'r>(
    repo: &'r gix::Repository,
    branch_name: &str,
    target: Option<&str>,
) -> eyre::Result<gix::Reference<'r>> {
    let target = target.unwrap_or("HEAD");
    tracing::debug!("Creating branch {branch_name:?} -> {target:?}");
    let rev = repo.rev_parse_single(target)?;
    let branch_name = format!("refs/heads/{branch_name}");
    let reference = repo.reference(
        branch_name.as_str(),
        rev,
        gix::refs::transaction::PreviousValue::Any,
        format!("Herostratus: Creating branch {branch_name:?} at {target:?}"),
    )?;
    Ok(reference)
}

/// Switch to the specified branch, creating it at the current HEAD if necessary
#[tracing::instrument(level = "debug", skip_all, fields(path = %repo.path().display()))]
pub fn set_default_branch(repo: &gix::Repository, branch_name: &str) -> eyre::Result<()> {
    tracing::debug!("Switching to branch {branch_name:?}");
    if repo.try_find_reference(branch_name)?.is_none() {
        // If HEAD doesn't exist yet, we can't create the reference it points to
        if repo.head_id().is_ok() {
            create_branch(repo, branch_name, None)?;
        }
    }

    // Now update the symbolic HEAD ref itself to point to the new branch
    let local_head = gix::refs::FullName::try_from("HEAD")?;
    let new_target = gix::refs::FullName::try_from(format!("refs/heads/{branch_name}"))?;

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_forget() {
        let mut temp = simplest().unwrap();
        temp.forget();

        assert!(temp.repo.path().exists());
        let path = temp.tempdir.path().to_path_buf();
        drop(temp);

        assert!(path.exists());
        std::fs::remove_dir_all(&path).unwrap();
        assert!(!path.exists());

        let mut temp = simplest().unwrap();
        temp.forget();
        temp.remember();
        let path = temp.tempdir.path().to_path_buf();
        drop(temp);
        assert!(!path.exists());
    }

    #[test]
    fn test_in_memory() {
        let odb = git2::Odb::new().unwrap();
        let repo = git2::Repository::from_odb(odb).unwrap();

        // This fails with in-memory Repository / Odb's
        assert!(repo.index().is_err());

        let mut index = git2::Index::new().unwrap();
        repo.set_index(&mut index).unwrap();
        let mut index = repo.index().unwrap();

        // This fails with in-memory Repository / Odb's
        assert!(index.write_tree().is_err());
    }

    #[test]
    fn test_bare_repository() {
        let repo = bare().unwrap();
        assert!(repo.repo.is_bare());

        let default_branch = repo.repo.head_name().unwrap();
        assert_eq!(
            default_branch,
            Some(gix::refs::FullName::try_from("refs/heads/main").unwrap())
        );
    }

    #[test]
    fn test_set_default_branch() {
        let repo = bare().unwrap();
        assert!(repo.repo.is_bare());

        let default_branch = repo.repo.head_name().unwrap();
        assert_eq!(
            default_branch,
            Some(gix::refs::FullName::try_from("refs/heads/main").unwrap())
        );

        set_default_branch(&repo.repo, "trunk").unwrap();
        let default_branch = repo.repo.head_name().unwrap();
        assert_eq!(
            default_branch,
            Some(gix::refs::FullName::try_from("refs/heads/trunk").unwrap())
        );
    }

    #[test]
    fn test_add_empty_commits() {
        let repo = bare().unwrap();

        let commit1 = add_empty_commit(&repo.repo, "commit1").unwrap();
        let head = repo.repo.head_id().unwrap();
        assert_eq!(head, commit1);

        let commit2 = add_empty_commit(&repo.repo, "commit2").unwrap();
        let head = repo.repo.head_id().unwrap();
        assert_eq!(head, commit2);
    }

    #[test]
    fn test_commits_on_branches() {
        let repo = bare().unwrap();

        set_default_branch(&repo.repo, "branch1").unwrap();
        let commit1 = add_empty_commit(&repo.repo, "commit1 on branch1").unwrap();
        let head = repo.repo.head_id().unwrap();
        assert_eq!(head, commit1);

        set_default_branch(&repo.repo, "branch2").unwrap();
        let commit2 = add_empty_commit(&repo.repo, "commit2 on branch2").unwrap();
        let head = repo.repo.head_id().unwrap();
        assert_eq!(head, commit2);
    }
}
