use std::collections::HashSet;

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
/// walks from the old boundary OIDs and continues yielding the newly available commits.
///
/// A commit reachable from HEAD via a short chain (within the original shallow window) can also
/// be an ancestor of a boundary commit on a longer chain. Without cross-batch deduplication, the
/// post-deepen walk from the boundary would re-yield such commits. `seen` tracks every yielded
/// OID across all batches to filter out these cross-batch duplicates.
///
/// Errors from deepening are fatal and propagated as `Some(Err(...))`.
pub struct DeepeningRevWalk<'repo> {
    repo: &'repo mut gix::Repository,
    config: RepositoryConfig,
    batch_size: usize,
    commits: Vec<gix::ObjectId>,
    index: usize,
    done: bool,
    seen: HashSet<gix::ObjectId>,
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
        let commits = collect_walk(repo, vec![head_oid], &HashSet::new())?;
        Ok(Self {
            repo,
            config,
            batch_size,
            commits,
            index: 0,
            done: false,
            seen: HashSet::new(),
        })
    }
}

/// Run a rev_walk from the given starting OIDs, sorted by commit time (newest first), filtering
/// out any OIDs in `exclude`, and collect the results into a Vec.
fn collect_walk(
    repo: &gix::Repository,
    start: Vec<gix::ObjectId>,
    exclude: &HashSet<gix::ObjectId>,
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
            // Yield buffered commits while available, recording them in `seen`.
            if self.index < self.commits.len() {
                let oid = self.commits[self.index];
                self.index += 1;
                self.seen.insert(oid);
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

            // Walk from the old boundary OIDs, excluding everything we've yielded in any prior
            // batch. The exclude set must be the cumulative `seen` set, not just `boundary_oids`,
            // because a commit can be reachable from HEAD via a short chain (already yielded) AND
            // be an ancestor of a boundary commit on a longer chain (about to be re-yielded by this
            // walk).
            match collect_walk(self.repo, boundary_oids, &self.seen) {
                Ok(new_commits) => {
                    if new_commits.is_empty() {
                        // Deepened successfully but no new commits reachable from the old boundary.
                        // This can happen if the boundary shifted but all new commits are on
                        // branches we don't follow, or if every newly-visible commit was already
                        // yielded via another path.
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
    fn test_deepening_walk_preserves_commit_order() {
        // Verify that DeepeningRevWalk yields commits in the same order as a full rev_walk
        // on the complete history.
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

        // Full clone to get the expected ordering
        let tempdir_full = tempfile::tempdir().unwrap();
        let full_config = RepositoryConfig {
            reference: None,
            url: format!("file://{}", upstream.tempdir.path().display()),
            path: tempdir_full.path().join("full"),
            ..Default::default()
        };
        let full_repo = crate::git::clone::clone_repository(&full_config, false, None).unwrap();
        let head = crate::git::rev::parse("HEAD", &full_repo).unwrap();
        let expected: Vec<_> = crate::git::rev::walk(head, &full_repo)
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();

        // Shallow clone + DeepeningRevWalk
        let tempdir_shallow = tempfile::tempdir().unwrap();
        let shallow_config = RepositoryConfig {
            reference: None,
            url: format!("file://{}", upstream.tempdir.path().display()),
            path: tempdir_shallow.path().join("shallow"),
            ..Default::default()
        };
        let mut shallow_repo =
            crate::git::clone::clone_repository(&shallow_config, false, Some(2)).unwrap();
        let head = crate::git::rev::parse("HEAD", &shallow_repo).unwrap();
        let walk = DeepeningRevWalk::new(head, &mut shallow_repo, shallow_config, 2).unwrap();
        let actual: Vec<_> = walk.collect::<Result<_, _>>().unwrap();

        assert_eq!(
            expected.len(),
            actual.len(),
            "Should yield the same number of commits"
        );
        assert_eq!(
            expected, actual,
            "Commit ordering should match full rev_walk"
        );
    }

    #[test]
    fn test_deepening_walk_no_cross_batch_duplicates() {
        // Topology where common ancestor X is reachable from HEAD via two chains of unequal length:
        //
        //   R - X - A ---- M (HEAD)
        //        \        /
        //         Y1-Y2-Y3
        //
        // depth=4 makes the short A-chain (M, A, X, R) fully visible, but cuts the long Y-chain at
        // Y1 (boundary). X is yielded in batch 1 via the A path, and is also an ancestor of the
        // boundary Y1. After deepen, walking from {Y1} reaches X again. Without cross-batch dedup
        // the iterator yields X twice.
        let upstream = repository::Builder::new()
            .commit("R")
            .time(1_000)
            .commit("X")
            .time(2_000)
            .branch("feature")
            .commit("Y1")
            .time(3_000)
            .commit("Y2")
            .time(4_000)
            .commit("Y3")
            .time(5_000)
            .branch("main")
            .commit("A")
            .time(6_000)
            .build()
            .unwrap();

        upstream.merge("feature", "M").time(7_000).create().unwrap();

        let tempdir = tempfile::tempdir().unwrap();
        let downstream_dir = tempdir.path().join("downstream");
        let config = RepositoryConfig {
            reference: None,
            url: format!("file://{}", upstream.tempdir.path().display()),
            path: downstream_dir,
            ..Default::default()
        };
        let mut downstream = crate::git::clone::clone_repository(&config, false, Some(4)).unwrap();
        assert!(downstream.is_shallow());

        let head = crate::git::rev::parse("HEAD", &downstream).unwrap();
        let walk = DeepeningRevWalk::new(head, &mut downstream, config, 2).unwrap();
        let yielded: Vec<gix::ObjectId> = walk.collect::<Result<_, _>>().unwrap();

        let mut counts: std::collections::HashMap<gix::ObjectId, usize> =
            std::collections::HashMap::new();
        for oid in &yielded {
            *counts.entry(*oid).or_insert(0) += 1;
        }
        let dupes: Vec<_> = counts.iter().filter(|(_, c)| **c > 1).collect();
        assert!(
            dupes.is_empty(),
            "DeepeningRevWalk yielded duplicates: {dupes:#?}\n\
             total yielded={}, unique={}",
            yielded.len(),
            counts.len()
        );
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
