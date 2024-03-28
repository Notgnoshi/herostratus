pub mod repository {
    use eyre::Result;
    use git2::{Repository, Signature, Time};
    use tempfile::{tempdir, TempDir};

    pub struct TempRepository {
        pub tempdir: TempDir,
        pub repo: Repository,
    }

    pub fn simplest() -> Result<TempRepository> {
        with_empty_commits(&["Initial commit"])
    }

    pub fn with_empty_commits(messages: &[&str]) -> Result<TempRepository> {
        let tempdir = tempdir()?;
        tracing::debug!("Creating repo fixture in '{}'", tempdir.path().display());

        let repo = Repository::init(tempdir.path())?;
        let mut index = repo.index()?;

        let mut parent = None;

        for message in messages {
            let oid = index.write_tree()?;
            let tree = repo.find_tree(oid)?;

            let time = Time::new(1711656630, -500);
            let signature = Signature::new("Herostratus", "Herostratus@example.com", &time)?;

            let parents = if let Some(ref parent) = parent {
                vec![parent]
            } else {
                vec![]
            };

            let oid = repo.commit(
                Some("HEAD"),
                &signature,
                &signature,
                message,
                &tree,
                &parents,
            )?;
            let commit = repo.find_commit(oid)?;

            tracing::debug!("Created commit {oid:?}");

            parent = Some(commit);
        }
        drop(parent);

        Ok(TempRepository { tempdir, repo })
    }
}

mod tests {
    use git2::{Index, Odb, Repository};

    use super::*;
    use crate::git::{rev_parse, rev_walk};

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
        let temp_repo = repository::simplest().unwrap();

        let rev = rev_parse("HEAD", &temp_repo.repo).unwrap();
        let commits: Vec<_> = rev_walk(rev, &temp_repo.repo)
            .unwrap()
            .map(|oid| temp_repo.repo.find_commit(oid.unwrap()).unwrap())
            .collect();
        assert_eq!(commits.len(), 1);
        assert_eq!(commits[0].summary().unwrap(), "Initial commit");
    }
}
