use tempfile::{tempdir, TempDir};

pub struct TempRepository<R = git2::Repository> {
    pub tempdir: TempDir,
    pub repo: R,
}

pub fn add_empty_commit(repo: &git2::Repository, message: &str) -> eyre::Result<()> {
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
    tracing::debug!("Created commit {oid:?} with message {message:?}");

    Ok(())
}

pub fn add_empty_commit2(repo: &gix::Repository, message: &str) -> eyre::Result<()> {
    let mut head = repo.head()?;
    let head_oid = head.peel_to_object_in_place().ok();
    let parents = if let Some(ref parent) = head_oid {
        vec![parent.id()]
    } else {
        Vec::new()
    };
    let tree = if let Some(ref tree) = head_oid {
        tree.id().detach()
    } else {
        gix::ObjectId::empty_tree(gix::hash::Kind::Sha1)
    };

    let id = repo.commit("HEAD", message, tree, parents)?;
    tracing::debug!("Created commit {:?} with message {message:?}", id);

    Ok(())
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

#[cfg(test)]
mod tests {
    use git2::{Index, Odb, Repository};
    use herostratus::git;

    use super::*;

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
