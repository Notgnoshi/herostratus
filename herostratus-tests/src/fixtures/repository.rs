use tempfile::{tempdir, TempDir};

pub struct TempRepository<R = git2::Repository> {
    pub tempdir: TempDir,
    pub repo: R,
}

impl<R> TempRepository<R> {
    pub fn forget(self) -> R {
        let repo = self.repo;
        // consumes the TempDir without deleting it
        let _path = self.tempdir.into_path();
        repo
    }
}

pub fn add_empty_commit<'r>(
    repo: &'r git2::Repository,
    message: &str,
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

    let time = git2::Time::new(1711656630, -500);
    let signature = git2::Signature::new("Herostratus", "Herostratus@example.com", &time)?;

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
        "Created commit {oid:?} with message {message:?} in repo {}",
        repo.path().display()
    );

    Ok(commit)
}

pub fn add_empty_commit2<'r>(
    repo: &'r gix::Repository,
    message: &str,
) -> eyre::Result<gix::Commit<'r>> {
    let head = repo.head_commit().ok();
    let parents = if let Some(ref parent) = head {
        vec![parent.id()]
    } else {
        Vec::new()
    };
    let head = repo.head_tree_id().ok();
    let tree = if let Some(ref tree) = head {
        tree.detach()
    } else {
        gix::ObjectId::empty_tree(gix::hash::Kind::Sha1)
    };

    let id = repo.commit("HEAD", message, tree, parents)?;
    let commit = repo.find_commit(id)?;
    tracing::debug!(
        "Created commit {id:?} with message {message:?} in repo {}",
        repo.path().display()
    );

    Ok(commit)
}

pub fn simplest() -> eyre::Result<TempRepository> {
    with_empty_commits(&["Initial commit"])
}

pub fn simplest2() -> eyre::Result<TempRepository<gix::Repository>> {
    with_empty_commits2(&["Initial commit"])
}

pub fn with_empty_commits(messages: &[&str]) -> eyre::Result<TempRepository> {
    let tempdir = tempdir()?;
    tracing::debug!("Creating repo fixture in '{}'", tempdir.path().display());

    let repo = git2::Repository::init(tempdir.path())?;

    for message in messages {
        add_empty_commit(&repo, message)?;
    }

    Ok(TempRepository { tempdir, repo })
}

pub fn with_empty_commits2(messages: &[&str]) -> eyre::Result<TempRepository<gix::Repository>> {
    let tempdir = tempdir()?;
    tracing::debug!("Creating repo fixture in '{}'", tempdir.path().display());
    let repo = gix::init(tempdir.path())?;

    for message in messages {
        add_empty_commit2(&repo, message)?;
    }

    Ok(TempRepository { tempdir, repo })
}

pub fn upstream_downstream() -> eyre::Result<(TempRepository, TempRepository)> {
    let upstream = with_empty_commits(&[])?;
    let downstream = with_empty_commits(&[])?;
    tracing::debug!(
        "Setting {} as upstream remote of {}",
        upstream.tempdir.path().display(),
        downstream.tempdir.path().display()
    );
    downstream.repo.remote_set_url(
        "origin",
        &format!("file://{}", upstream.tempdir.path().display()),
    )?;

    Ok((upstream, downstream))
}

pub fn upstream_downstream2() -> eyre::Result<(
    TempRepository<gix::Repository>,
    TempRepository<gix::Repository>,
)> {
    let upstream = with_empty_commits2(&[])?;
    let mut downstream = with_empty_commits2(&[])?;
    tracing::debug!(
        "Setting {} as upstream remote of {}",
        upstream.tempdir.path().display(),
        downstream.tempdir.path().display()
    );
    // TODO: I can't figure out how to use gix to add a named remote, or how to change the name of
    // a new remote created from a URL. Since this is a test fixture, just shell out to Git and
    // move on.
    let status = std::process::Command::new("git")
        .arg("-C")
        .arg(downstream.tempdir.path())
        .arg("remote")
        .arg("add")
        .arg("origin")
        .arg(format!("file://{}", upstream.tempdir.path().display()))
        .status()?;
    assert!(status.success());
    // BUG: Gotta re-discover the repository for gix to find the new remote ...
    downstream.repo = gix::discover(downstream.tempdir.path())?;
    assert!(downstream.repo.find_remote("origin").is_ok());
    Ok((upstream, downstream))
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

    #[test]
    fn test_new_gix_repository() {
        let temp_repo = with_empty_commits2(&["Initial Commit"]).unwrap();

        let rev = git::rev::parse2("HEAD", &temp_repo.repo).unwrap();
        let commits: Vec<_> = git::rev::walk2(rev, &temp_repo.repo)
            .unwrap()
            .map(|r| temp_repo.repo.find_commit(r.unwrap()).unwrap())
            .collect();
        assert_eq!(commits.len(), 1);
        assert_eq!(commits[0].message().unwrap().title, "Initial Commit");
    }
}
