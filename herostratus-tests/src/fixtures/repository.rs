use git2::{Repository, Signature, Time};
use tempfile::{tempdir, TempDir};

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

pub fn add_empty_commit(repo: &Repository, message: &str) -> eyre::Result<()> {
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

    let time = Time::new(1711656630, -500);
    let signature = Signature::new("Herostratus", "Herostratus@example.com", &time)?;

    let oid = repo.commit(
        Some("HEAD"),
        &signature,
        &signature,
        message,
        &tree,
        &parents,
    )?;
    tracing::debug!("Created commit {oid:?}");

    Ok(())
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

#[cfg(test)]
mod tests {
    use git2::{Index, Odb, Repository};
    use herostratus::git;

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
        let odb = Odb::new().unwrap();
        let repo = Repository::from_odb(odb).unwrap();

        // This fails with in-memory Repository / Odb's
        assert!(repo.index().is_err());

        let mut index = Index::new().unwrap();
        repo.set_index(&mut index).unwrap();
        let mut index = repo.index().unwrap();

        // This fails with in-memory Repository / Odb's
        assert!(index.write_tree().is_err());
    }

    #[test]
    fn test_new_repository() {
        let temp_repo = simplest().unwrap();

        let rev = git::rev::parse("HEAD", &temp_repo.repo).unwrap();
        let commits: Vec<_> = git::rev::walk(rev, &temp_repo.repo)
            .unwrap()
            .map(|oid| temp_repo.repo.find_commit(oid.unwrap()).unwrap())
            .collect();
        assert_eq!(commits.len(), 1);
        assert_eq!(commits[0].summary().unwrap(), "Initial commit");
    }
}
