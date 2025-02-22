use git2::{Repository, Signature, Time};
use tempfile::{TempDir, tempdir};

pub struct TempRepository<R = git2::Repository> {
    pub tempdir: TempDir,
    pub repo: R,
}

impl<R> TempRepository<R> {
    /// Consume the TempDir without deleting the on-disk repository
    ///
    /// You probably don't want to use this in the final state of a test, but it can be useful for
    /// troubleshooting when things aren't working as you think they should.
    pub fn forget(self) -> R {
        let repo = self.repo;
        // consumes the TempDir without deleting it
        let _path = self.tempdir.into_path();
        repo
    }
}

impl TempRepository<git2::Repository> {
    pub fn git2(&self) -> git2::Repository {
        git2::Repository::discover(self.tempdir.path()).unwrap()
    }
    pub fn gix(&self) -> gix::Repository {
        gix::discover(self.tempdir.path()).unwrap()
    }
}
impl TempRepository<gix::Repository> {
    pub fn git2(&self) -> git2::Repository {
        git2::Repository::discover(self.tempdir.path()).unwrap()
    }
    pub fn gix(&self) -> gix::Repository {
        self.repo.clone()
    }
}

pub fn add_empty_commit<'r>(repo: &'r Repository, message: &str) -> eyre::Result<git2::Commit<'r>> {
    let time = Time::new(1711656630, -500);
    add_empty_commit_time(repo, message, time)
}

pub fn add_empty_commit_time<'r>(
    repo: &'r Repository,
    message: &str,
    time: Time,
) -> eyre::Result<git2::Commit<'r>> {
    let mut index = repo.index()?;
    let head = repo.find_reference("HEAD")?;
    let parent = head.peel_to_commit().ok();
    let parents = if let Some(ref parent) = parent {
        vec![parent]
    } else {
        vec![]
    };

    let oid = index.write_tree()?;
    let tree = repo.find_tree(oid)?;

    let signature = Signature::new("Herostratus", "Herostratus@example.com", &time)?;

    let oid = repo.commit(
        Some("HEAD"),
        &signature,
        &signature,
        message,
        &tree,
        &parents,
    )?;
    let commit = repo.find_commit(oid)?;
    tracing::debug!(
        "Created commit {oid:?} with message {message:?} in repo {:?}",
        repo.path()
    );

    Ok(commit)
}

pub fn simplest() -> eyre::Result<TempRepository> {
    with_empty_commits(&["Initial commit"])
}

pub fn with_empty_commits(messages: &[&str]) -> eyre::Result<TempRepository> {
    let tempdir = tempdir()?;
    tracing::debug!("Creating repo fixture in '{}'", tempdir.path().display());

    let repo = Repository::init(tempdir.path())?;

    for message in messages {
        add_empty_commit(&repo, message)?;
    }

    Ok(TempRepository { tempdir, repo })
}

/// Return a pair of empty [TempRepository]s with the upstream configured as the "origin" remote of
/// the downstream
pub fn upstream_downstream_empty() -> eyre::Result<(TempRepository, TempRepository)> {
    let upstream = with_empty_commits(&[])?;
    let downstream = with_empty_commits(&[])?;
    tracing::debug!(
        "Setting {:?} as upstream remote of {:?}",
        upstream.tempdir.path(),
        downstream.tempdir.path()
    );
    downstream.repo.remote_set_url(
        "origin",
        &format!("file://{}", upstream.tempdir.path().display()),
    )?;
    Ok((upstream, downstream))
}

pub fn upstream_downstream() -> eyre::Result<(TempRepository, TempRepository)> {
    let (upstream, downstream) = upstream_downstream_empty()?;
    add_empty_commit(&upstream.repo, "Initial upstream commit")?;
    add_empty_commit(&downstream.repo, "Initial downstream commit")?;
    Ok((upstream, downstream))
}

/// Switch to the specified branch, creating it at the current HEAD if necessary
pub fn switch_branch(repo: &git2::Repository, branch_name: &str) -> eyre::Result<()> {
    tracing::debug!(
        "Switching to branch {branch_name:?} in repo {:?}",
        repo.path()
    );
    // NOTE: gix can create a branch, but can't (yet?) switch to it in the working tree
    //
    // See: https://github.com/GitoxideLabs/gitoxide/discussions/879
    // See: https://github.com/GitoxideLabs/gitoxide/issues/301 (maybe it _is_ supported?)

    if repo.find_reference(branch_name).is_err() {
        tracing::debug!(
            "Failed to find {branch_name:?} in repo {:?} ... creating",
            repo.path()
        );
        let head = repo.head()?;
        let head = head.peel_to_commit()?;
        // If the branch exists, replace it. If it doesn't exist, make it.
        let _branch = repo.branch(branch_name, &head, true)?;
    }

    repo.set_head(format!("refs/heads/{branch_name}").as_str())?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_forget() {
        let temp = simplest().unwrap();
        let repo = temp.forget();

        assert!(repo.path().exists());
        std::fs::remove_dir_all(repo.path()).unwrap();
        assert!(!repo.path().exists());
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
    fn test_new_repository() {
        let temp_repo = simplest().unwrap();

        let branches: Vec<_> = temp_repo.repo.branches(None).unwrap().collect();
        assert_eq!(branches.len(), 1);
        let (branch, branch_type) = branches[0].as_ref().unwrap();
        assert_eq!(branch_type, &git2::BranchType::Local);
        assert!(branch.is_head());
        assert_eq!(branch.name().unwrap().unwrap(), "master");
    }

    #[test]
    fn test_switch_branch() {
        let temp_repo = simplest().unwrap();

        // Create two branches pointing at HEAD
        switch_branch(&temp_repo.repo, "branch1").unwrap();
        switch_branch(&temp_repo.repo, "branch2").unwrap();

        switch_branch(&temp_repo.repo, "branch1").unwrap();
        add_empty_commit(&temp_repo.repo, "commit on branch1").unwrap();

        switch_branch(&temp_repo.repo, "branch2").unwrap();
        add_empty_commit(&temp_repo.repo, "commit on branch2").unwrap();

        let branches: Vec<_> = temp_repo
            .repo
            .branches(None)
            .unwrap()
            .map(|b| b.unwrap().0.name().unwrap().unwrap().to_string())
            .collect();
        assert_eq!(branches, ["branch1", "branch2", "master"]);
    }
}
