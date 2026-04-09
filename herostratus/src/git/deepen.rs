use eyre::WrapErr;

use crate::config::RepositoryConfig;

/// An iterator that walks commit history, transparently deepening a shallow clone when the current
/// batch of commits is exhausted.
///
/// For non-shallow repositories, this behaves identically to a plain rev_walk -- all commits are
/// available from the start and no deepening occurs.
///
/// For shallow clones, when the buffered commits are drained, the iterator reads the current
/// shallow boundary OIDs, calls [deepen](crate::git::clone::deepen) to fetch more history, then
/// walks from the old boundary OIDs (filtering them out, since they were already yielded) and
/// continues yielding the newly available commits.
///
/// Errors from deepening are fatal and propagated as `Some(Err(...))`.
pub struct DeepeningRevWalk<'repo> {
    repo: &'repo mut gix::Repository,
    config: RepositoryConfig,
    batch_size: usize,
    commits: Vec<gix::ObjectId>,
    index: usize,
    done: bool,
}

impl<'repo> DeepeningRevWalk<'repo> {
    /// Create a new DeepeningRevWalk starting from the given OID.
    ///
    /// The initial rev_walk from `head_oid` is collected eagerly into an internal buffer. This is
    /// fine because we only buffer ObjectIds (20 bytes each), not full commit objects.
    pub fn new(
        head_oid: gix::ObjectId,
        repo: &'repo mut gix::Repository,
        config: RepositoryConfig,
        batch_size: usize,
    ) -> eyre::Result<Self> {
        let commits = collect_walk(repo, vec![head_oid], &[])?;
        Ok(Self {
            repo,
            config,
            batch_size,
            commits,
            index: 0,
            done: false,
        })
    }
}

/// Run a rev_walk from the given starting OIDs, sorted by commit time (newest first), filtering
/// out any OIDs in `exclude`, and collect the results into a Vec.
fn collect_walk(
    repo: &gix::Repository,
    start: Vec<gix::ObjectId>,
    exclude: &[gix::ObjectId],
) -> eyre::Result<Vec<gix::ObjectId>> {
    let walk = repo.rev_walk(start);
    let walk = walk.sorting(gix::revision::walk::Sorting::ByCommitTime(
        gix::traverse::commit::simple::CommitTimeOrder::NewestFirst,
    ));
    let walk = walk.all().wrap_err("Failed to start rev_walk")?;
    let mut oids = Vec::new();
    for item in walk {
        let info = item?;
        if !exclude.contains(&info.id) {
            oids.push(info.id);
        }
    }
    Ok(oids)
}

impl Iterator for DeepeningRevWalk<'_> {
    type Item = eyre::Result<gix::ObjectId>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            // Yield buffered commits while available.
            if self.index < self.commits.len() {
                let oid = self.commits[self.index];
                self.index += 1;
                return Some(Ok(oid));
            }

            // Buffer exhausted. If we already know there's nothing more, stop.
            if self.done {
                return None;
            }

            // If the repo is not shallow, all history is already available.
            if !self.repo.is_shallow() {
                self.done = true;
                return None;
            }

            // Read the current shallow boundary OIDs before deepening.
            let boundary_oids: Vec<gix::ObjectId> = match self.repo.shallow_commits() {
                Ok(Some(commits)) => commits.iter().copied().collect(),
                Ok(None) => {
                    // No shallow boundary means all history is available.
                    self.done = true;
                    return None;
                }
                Err(e) => {
                    self.done = true;
                    return Some(Err(e.into()));
                }
            };

            let deepened = match crate::git::clone::deepen(&self.config, self.repo, self.batch_size)
            {
                Ok(true) => true,
                Ok(false) => {
                    // No more history available from the remote.
                    self.done = true;
                    return None;
                }
                Err(e) => {
                    self.done = true;
                    return Some(Err(e));
                }
            };

            debug_assert!(deepened);

            // Walk from the old boundary OIDs, excluding the boundary OIDs themselves (already
            // yielded in the previous batch).
            match collect_walk(self.repo, boundary_oids.clone(), &boundary_oids) {
                Ok(new_commits) => {
                    if new_commits.is_empty() {
                        // Deepened successfully but no new commits reachable from the old
                        // boundary. This can happen if the boundary shifted but all new commits
                        // are on branches we don't follow.
                        self.done = true;
                        return None;
                    }
                    self.commits = new_commits;
                    self.index = 0;
                    // Loop back to yield from the fresh buffer.
                }
                Err(e) => {
                    self.done = true;
                    return Some(Err(e));
                }
            }
        }
    }
}

#[cfg(test)]
mod test {
    use herostratus_tests::fixtures::repository;

    use super::*;

    #[test]
    fn test_deepening_walk_no_shallow() {
        // Non-shallow repo, batch_size doesn't matter; behaves like plain walk
        let temp_repo = repository::Builder::new()
            .commit("commit1")
            .commit("commit2")
            .commit("commit3")
            .build()
            .unwrap();

        let head = crate::git::rev::parse("HEAD", &temp_repo.repo).unwrap();
        let config = RepositoryConfig::default();
        let mut repo = temp_repo.repo;
        let walk = DeepeningRevWalk::new(head, &mut repo, config, 2).unwrap();
        let oids: Vec<_> = walk.collect::<Result<_, _>>().unwrap();
        assert_eq!(oids.len(), 3);
    }

    #[test]
    fn test_deepening_walk_through_shallow_boundary() {
        // Shallow clone with depth=2, batch_size=2, 6 commits upstream.
        // Should deepen twice to get all 6.
        let upstream = repository::Builder::new()
            .commit("commit1")
            .time(1000)
            .commit("commit2")
            .time(2000)
            .commit("commit3")
            .time(3000)
            .commit("commit4")
            .time(4000)
            .commit("commit5")
            .time(5000)
            .commit("commit6")
            .time(6000)
            .build()
            .unwrap();
        let tempdir = tempfile::tempdir().unwrap();
        let downstream_dir = tempdir.path().join("downstream");

        let config = RepositoryConfig {
            reference: None,
            url: format!("file://{}", upstream.tempdir.path().display()),
            path: downstream_dir.clone(),
            ..Default::default()
        };
        let mut downstream = crate::git::clone::clone_repository(&config, false, Some(2)).unwrap();

        let head = crate::git::rev::parse("HEAD", &downstream).unwrap();
        let walk = DeepeningRevWalk::new(head, &mut downstream, config, 2).unwrap();
        let oids: Vec<_> = walk.collect::<Result<_, _>>().unwrap();
        assert_eq!(oids.len(), 6);
    }

    #[test]
    fn test_deepening_walk_stops_when_consumer_stops() {
        // Same setup but take(3) -- should not fetch all history
        let upstream = repository::Builder::new()
            .commit("commit1")
            .time(1000)
            .commit("commit2")
            .time(2000)
            .commit("commit3")
            .time(3000)
            .commit("commit4")
            .time(4000)
            .commit("commit5")
            .time(5000)
            .commit("commit6")
            .time(6000)
            .build()
            .unwrap();
        let tempdir = tempfile::tempdir().unwrap();
        let downstream_dir = tempdir.path().join("downstream");

        let config = RepositoryConfig {
            reference: None,
            url: format!("file://{}", upstream.tempdir.path().display()),
            path: downstream_dir.clone(),
            ..Default::default()
        };
        let mut downstream = crate::git::clone::clone_repository(&config, false, Some(2)).unwrap();

        let head = crate::git::rev::parse("HEAD", &downstream).unwrap();
        let walk = DeepeningRevWalk::new(head, &mut downstream, config, 2).unwrap();
        let oids: Vec<_> = walk.take(3).collect::<Result<_, _>>().unwrap();
        assert_eq!(oids.len(), 3);
    }
}
