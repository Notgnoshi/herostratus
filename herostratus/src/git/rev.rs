use eyre::WrapErr;

pub fn parse(reference: &str, repo: &gix::Repository) -> eyre::Result<gix::ObjectId> {
    let object = repo
        .rev_parse_single(reference)
        .wrap_err("Failed to rev-parse")?;
    let oid = object.detach();
    tracing::debug!(
        "Resolved {reference:?} to {:?} {oid:?}",
        object.object()?.kind
    );
    Ok(oid)
}

pub fn walk(
    oid: gix::ObjectId,
    repo: &gix::Repository,
) -> eyre::Result<impl Iterator<Item = eyre::Result<gix::ObjectId>> + '_> {
    let walk = repo.rev_walk(Some(oid));
    let walk = walk.sorting(gix::revision::walk::Sorting::ByCommitTime(
        gix::traverse::commit::simple::CommitTimeOrder::NewestFirst,
    ));
    let walk = walk.all()?;
    Ok(walk.map(|i| match i {
        Ok(info) => Ok(info.id),
        Err(e) => Err(e.into()),
    }))
}

#[cfg(test)]
mod test {
    use herostratus_tests::fixtures;

    use super::*;

    #[test]
    fn test_rev_parse_and_walk() {
        let temp_repo = fixtures::repository::simplest().unwrap();
        let time = 1711656631;
        fixtures::repository::add_empty_commit_time(&temp_repo.repo, "commit2", time).unwrap();
        let time = 1711656633;
        fixtures::repository::add_empty_commit_time(&temp_repo.repo, "commit3", time).unwrap();
        let time = 1711656632;
        fixtures::repository::add_empty_commit_time(&temp_repo.repo, "commit4", time).unwrap();

        let repo = temp_repo.repo;

        let rev = parse("HEAD", &repo).unwrap();
        let commits: Vec<_> = walk(rev, &repo)
            .unwrap()
            .map(|oid| repo.find_commit(oid.unwrap()).unwrap())
            .collect();
        assert_eq!(commits.len(), 4);
        assert_eq!(commits[0].message().unwrap().summary().as_ref(), "commit4");
        assert_eq!(commits[1].message().unwrap().summary().as_ref(), "commit3");
        assert_eq!(commits[2].message().unwrap().summary().as_ref(), "commit2");
        assert_eq!(
            commits[3].message().unwrap().summary().as_ref(),
            "Initial commit"
        );
    }
}
