use eyre::WrapErr;
use rayon::prelude::*;

use super::commit_context::CommitContext;
use super::observation::Observation;
use super::observer::{DiffAction, Observer};
use super::observer_data::ObserverData;
use crate::git::mailmap::MailmapResolver;

/// Runs [Observer]s against commits in a repository, producing [ObserverData] messages.
///
/// For each commit, the engine resolves the author via the mailmap, calls each observer's
/// [Observer::on_commit], then runs the diff lifecycle for observers that opt in via
/// [Observer::is_interested_in_diff]. See the [Observer] trait for the full lifecycle.
///
/// Results are emitted as [ObserverData] messages in protocol order:
///
/// * [ObserverData::CommitStart]
/// * zero or more [ObserverData::Observation]s
/// * [ObserverData::CommitComplete]
///
/// # Error handling
///
/// Errors fall into two categories:
///
/// * **Infrastructure errors** (failed to find commit, failed to create diff cache) These are
///   propagated as `Err` and processing is aborted. These indicate a broken repository or
///   environment.
/// * **Observer errors** (an observer's `on_commit`, `on_diff_change`, etc. returns `Err`) are
///   logged via `tracing::warn!` and skipped. The failing observer produces no observation for
///   that commit, but other observers continue, and [ObserverData::CommitComplete] is always
///   emitted.
///
/// # Parallelism
///
/// Both `on_commit` and the diff lifecycle are parallelized across observers using rayon. Each
/// rayon worker thread gets a cached thread-local [gix::Repository] handle (created once per
/// worker, amortized across all commits). The `find_commit` call per observer per commit is
/// redundant but cheap (packfiles are mmap'd, OS page cache serves repeated reads).
///
/// For diffs, changes are collected into owned
/// [ChangeDetached](gix::object::tree::diff::ChangeDetached) form first, then each observer
/// processes the full set of changes independently in parallel.
pub(crate) struct ObserverEngine<'repo> {
    repo: &'repo gix::Repository,
    // Shared with each thread in the rayon thread pool
    sync_repo: gix::ThreadSafeRepository,
    // Each thread in the thread pool caches its own thread-local Repository (created from the
    // shared sync_repo).
    tl_repo: thread_local::ThreadLocal<gix::Repository>,

    observers: Vec<Box<dyn Observer>>,
    mailmap: MailmapResolver,

    // This cache is unbounded and needs to be reset periodically to avoid infinite memory growth.
    // Don't reset it every commit, because each commit needs to lookup itself and its parent(s).
    // But we shouldn't *never* reset it, because then we'd end up holding the whole history in
    // memory. So we reset it every N commits processed.
    diff_cache: gix::diff::blob::Platform,
    num_commits_processed: u64,
}

impl<'repo> ObserverEngine<'repo> {
    pub fn new(
        repo: &'repo gix::Repository,
        observers: Vec<Box<dyn Observer>>,
        mailmap: MailmapResolver,
    ) -> eyre::Result<Self> {
        let diff_cache = repo
            .diff_resource_cache_for_tree_diff()
            .wrap_err("Failed to create diff cache")?;
        let sync_repo = repo.clone().into_sync();
        Ok(Self {
            repo,
            sync_repo,
            tl_repo: thread_local::ThreadLocal::new(),
            observers,
            mailmap,
            diff_cache,
            num_commits_processed: 0,
        })
    }

    /// Process a single commit. Returns [ObserverData] in protocol order.
    ///
    /// Observer errors are logged and skipped. [ObserverData::CommitComplete] is always emitted,
    /// even if observers error.
    ///
    /// Infrastructure errors (commit not found, mailmap resolution failed) propagate as `Err`.
    pub fn process_commit(&mut self, oid: gix::ObjectId) -> eyre::Result<Vec<ObserverData>> {
        let commit = self
            .repo
            .find_commit(oid)
            .wrap_err_with(|| format!("Failed to find commit {oid}"))?;
        self.num_commits_processed += 1;

        let author = self.mailmap.resolve_author(&commit)?;
        let ctx = CommitContext {
            oid,
            author_name: author.name.to_string(),
            author_email: author.email.to_string(),
        };

        let mut data = vec![ObserverData::CommitStart(ctx)];

        let sync_repo = &self.sync_repo;
        let tl_repo = &self.tl_repo;
        let observations: Vec<_> = self
            .observers
            .par_iter_mut()
            .filter_map(|obs| {
                let repo = tl_repo.get_or(|| sync_repo.to_thread_local());
                let commit = repo.find_commit(oid).ok()?;
                match obs.on_commit(&commit, repo) {
                    Ok(obs) => obs,
                    Err(e) => {
                        tracing::warn!("Observer error in on_commit: {e}");
                        None
                    }
                }
            })
            .collect();
        for obs in observations {
            data.push(ObserverData::Observation(obs));
        }

        if self.observers.iter().any(|o| o.is_interested_in_diff()) {
            let diff_observations = self.diff_commit(&commit)?;
            for obs in diff_observations {
                data.push(ObserverData::Observation(obs));
            }
        }

        // S.W.A.G. - Scientific Wild Ass Guess
        //
        // We need to clear the cache so it doesn't grow unboundedly, but we want enough data in
        // the cache for it to be effective.
        const CLEAR_CACHE_EVERY_N: u64 = 50;
        if self
            .num_commits_processed
            .is_multiple_of(CLEAR_CACHE_EVERY_N)
        {
            self.diff_cache.clear_resource_cache_keep_allocation();
        }

        data.push(ObserverData::CommitComplete);
        Ok(data)
    }

    /// Process all commits, sending [ObserverData] through the channel.
    ///
    /// Stops gracefully (returns `Ok`) if the receiver is dropped.
    pub fn run(
        &mut self,
        oids: impl IntoIterator<Item = gix::ObjectId>,
        tx: &std::sync::mpsc::Sender<ObserverData>,
    ) -> eyre::Result<()> {
        for oid in oids {
            let messages = self.process_commit(oid)?;
            for msg in messages {
                if tx.send(msg).is_err() {
                    return Ok(());
                }
            }
        }
        Ok(())
    }

    /// Run the diff lifecycle for a single commit.
    ///
    /// Calls `on_diff_start` / `on_diff_change` / `on_diff_end` on each diff-interested observer.
    /// Merge commits (>1 parent) skip the tree diff entirely -- the individual pre-merge commits
    /// already had their diffs observed. `on_diff_start` and `on_diff_end` are still called so
    /// observers can maintain consistent internal state.
    ///
    /// Changes are collected into owned [ChangeDetached](gix::object::tree::diff::ChangeDetached)
    /// form, then each observer processes the full set independently in parallel.
    fn diff_commit(&mut self, commit: &gix::Commit) -> eyre::Result<Vec<Observation>> {
        for observer in &mut self.observers {
            if observer.is_interested_in_diff()
                && let Err(e) = observer.on_diff_start()
            {
                tracing::warn!("Observer error in on_diff_start: {e}");
            }
        }

        // Skip diff computation for merge commits (>1 parent)
        let mut parents = commit.parent_ids();
        let parent = parents.next();
        if parents.next().is_some() {
            return self.collect_diff_end();
        }

        let commit_tree = commit
            .tree()
            .wrap_err_with(|| format!("Failed to get tree for commit {}", commit.id()))?;
        let parent_tree = match parent {
            Some(pid) => match self.repo.find_commit(pid) {
                Ok(parent) => parent
                    .tree()
                    .wrap_err_with(|| format!("Failed to get tree for parent commit {pid}"))?,
                // Shallow clone -- parent commit is missing, so diff against empty tree.
                Err(_) => self.repo.empty_tree(),
            },
            // Root commit -- no parent, so diff against empty tree.
            None => self.repo.empty_tree(),
        };

        let mut changes_iter = parent_tree
            .changes()
            .wrap_err("Failed to create tree changes iterator")?;
        changes_iter.options(|o| {
            o.track_rewrites(None);
        });

        // Collect all changes into owned form so we can dispatch to observers in parallel.
        let diff_cache = &mut self.diff_cache;
        let mut changes = Vec::new();
        let outcome =
            changes_iter.for_each_to_obtain_tree_with_cache(&commit_tree, diff_cache, |change| {
                changes.push(change.detach());
                Ok::<_, eyre::Report>(gix::object::tree::diff::Action::Continue(()))
            });

        match outcome {
            Ok(_) => {}
            Err(gix::object::tree::diff::for_each::Error::Diff(
                gix::diff::tree_with_rewrites::Error::Diff(gix::diff::tree::Error::Cancelled),
            )) => {}
            Err(e) => {
                return Err(e).wrap_err_with(|| format!("Failed to diff commit {}", commit.id()));
            }
        }

        // Run each diff-interested observer over the collected changes in parallel.
        let sync_repo = &self.sync_repo;
        let tl_repo = &self.tl_repo;
        let diff_observations: Vec<_> = self
            .observers
            .par_iter_mut()
            .filter(|obs| obs.is_interested_in_diff())
            .filter_map(|obs| {
                let repo = tl_repo.get_or(|| sync_repo.to_thread_local());
                for change in &changes {
                    match obs.on_diff_change(change, repo) {
                        Ok(DiffAction::Cancel) => break,
                        Ok(DiffAction::Continue) => {}
                        Err(e) => {
                            tracing::warn!("Observer error in on_diff_change: {e}");
                            break;
                        }
                    }
                }
                match obs.on_diff_end() {
                    Ok(obs) => obs,
                    Err(e) => {
                        tracing::warn!("Observer error in on_diff_end: {e}");
                        None
                    }
                }
            })
            .collect();

        Ok(diff_observations)
    }

    /// Call on_diff_end on all diff-interested observers, collecting observations.
    ///
    /// Per the [Observer] lifecycle, on_diff_end is always called regardless of errors or
    /// [DiffAction::Cancel].
    fn collect_diff_end(&mut self) -> eyre::Result<Vec<Observation>> {
        let mut observations = Vec::new();
        for observer in &mut self.observers {
            if observer.is_interested_in_diff() {
                match observer.on_diff_end() {
                    Ok(Some(obs)) => observations.push(obs),
                    Ok(None) => {}
                    Err(e) => {
                        tracing::warn!("Observer error in on_diff_end: {e}");
                    }
                }
            }
        }
        Ok(observations)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc;

    use herostratus_tests::fixtures::repository;

    use super::*;
    use crate::observer::test_observers::{AlwaysObserver, DummyDiffObserver, NeverObserver};

    fn default_mailmap() -> MailmapResolver {
        MailmapResolver::new(gix::mailmap::Snapshot::default(), None, None).unwrap()
    }

    fn default_ctx(oid: gix::ObjectId) -> CommitContext {
        CommitContext {
            oid,
            // matches the default author used by the repository::Builder fixture
            author_name: "Herostratus".to_string(),
            author_email: "Herostratus@example.com".to_string(),
        }
    }

    #[test]
    fn no_observers() {
        let temp_repo = repository::Builder::new()
            .commit("Initial commit")
            .build()
            .unwrap();
        let oid = crate::git::rev::parse("HEAD", &temp_repo.repo).unwrap();

        let mut engine =
            ObserverEngine::new(&temp_repo.repo, Vec::new(), default_mailmap()).unwrap();
        let data = engine.process_commit(oid).unwrap();

        assert_eq!(
            data,
            [
                ObserverData::CommitStart(default_ctx(oid)),
                ObserverData::CommitComplete,
            ]
        );
    }

    #[test]
    fn always_observer_emits_dummy() {
        let temp_repo = repository::Builder::new()
            .commit("Initial commit")
            .build()
            .unwrap();
        let oid = crate::git::rev::parse("HEAD", &temp_repo.repo).unwrap();

        let observers: Vec<Box<dyn Observer>> = vec![Box::new(AlwaysObserver)];
        let mut engine =
            ObserverEngine::new(&temp_repo.repo, observers, default_mailmap()).unwrap();
        let data = engine.process_commit(oid).unwrap();

        assert_eq!(
            data,
            [
                ObserverData::CommitStart(default_ctx(oid)),
                ObserverData::Observation(Observation::Dummy),
                ObserverData::CommitComplete,
            ]
        );
    }

    #[test]
    fn never_observer_emits_nothing() {
        let temp_repo = repository::Builder::new()
            .commit("Initial commit")
            .build()
            .unwrap();
        let oid = crate::git::rev::parse("HEAD", &temp_repo.repo).unwrap();

        let observers: Vec<Box<dyn Observer>> = vec![Box::new(NeverObserver)];
        let mut engine =
            ObserverEngine::new(&temp_repo.repo, observers, default_mailmap()).unwrap();
        let data = engine.process_commit(oid).unwrap();

        assert_eq!(
            data,
            [
                ObserverData::CommitStart(default_ctx(oid)),
                ObserverData::CommitComplete,
            ]
        );
    }

    #[test]
    fn diff_observer_emits_on_file_change() {
        let temp_repo = repository::Builder::new()
            .commit("Initial commit")
            .file("hello.txt", b"hello world")
            .build()
            .unwrap();
        let oid = crate::git::rev::parse("HEAD", &temp_repo.repo).unwrap();

        let observers: Vec<Box<dyn Observer>> = vec![Box::new(DummyDiffObserver::default())];
        let mut engine =
            ObserverEngine::new(&temp_repo.repo, observers, default_mailmap()).unwrap();
        let data = engine.process_commit(oid).unwrap();

        assert_eq!(
            data,
            [
                ObserverData::CommitStart(default_ctx(oid)),
                ObserverData::Observation(Observation::Dummy),
                ObserverData::CommitComplete,
            ]
        );
    }

    #[test]
    fn diff_skipped_for_merge_commit() {
        // Create a repo with a merge commit: two branches merged together.
        let temp_repo = repository::Builder::new()
            .commit("Initial commit")
            .file("base.txt", b"base")
            .build()
            .unwrap();

        // Create a side branch with a file change
        temp_repo.set_branch("side").unwrap();
        temp_repo
            .commit("side commit")
            .file("side.txt", b"side")
            .create()
            .unwrap();

        // Switch back to main and add a different file
        temp_repo.set_branch("main").unwrap();
        temp_repo
            .commit("main commit")
            .file("main.txt", b"main")
            .create()
            .unwrap();

        // Create a merge commit (two parents -> diff will be skipped)
        let main_head = temp_repo.repo.head_commit().unwrap();
        let side_ref = temp_repo.repo.find_reference("refs/heads/side").unwrap();
        let author = main_head.author().unwrap();
        let oid = temp_repo
            .repo
            .commit_as(
                author,
                author,
                "HEAD",
                "Merge side into main",
                main_head.tree_id().unwrap(),
                [main_head.id(), side_ref.id()],
            )
            .unwrap()
            .detach();

        // DummyDiffObserver should NOT see any diff changes for the merge commit
        let observers: Vec<Box<dyn Observer>> = vec![Box::new(DummyDiffObserver::default())];
        let mut engine =
            ObserverEngine::new(&temp_repo.repo, observers, default_mailmap()).unwrap();
        let data = engine.process_commit(oid).unwrap();

        assert_eq!(
            data,
            [
                ObserverData::CommitStart(default_ctx(oid)),
                ObserverData::CommitComplete,
            ]
        );
    }

    #[test]
    fn mailmap_resolution_in_commit_context() {
        let temp_repo = repository::Builder::new()
            .commit("Initial commit")
            .build()
            .unwrap();
        let oid = crate::git::rev::parse("HEAD", &temp_repo.repo).unwrap();

        let mailmap_dir = tempfile::tempdir().unwrap();
        let mailmap_path = mailmap_dir.path().join("mailmap");
        std::fs::write(
            &mailmap_path,
            "Canonical Name <canonical@example.com> Herostratus <Herostratus@example.com>\n",
        )
        .unwrap();

        let mailmap =
            MailmapResolver::new(gix::mailmap::Snapshot::default(), Some(&mailmap_path), None)
                .unwrap();

        let mut engine = ObserverEngine::new(&temp_repo.repo, Vec::new(), mailmap).unwrap();
        let data = engine.process_commit(oid).unwrap();

        let expected_ctx = CommitContext {
            oid,
            author_name: "Canonical Name".to_string(),
            author_email: "canonical@example.com".to_string(),
        };
        assert_eq!(
            data,
            [
                ObserverData::CommitStart(expected_ctx),
                ObserverData::CommitComplete,
            ]
        );
    }

    #[test]
    fn observer_error_doesnt_prevent_other_observers() {
        use std::mem::Discriminant;

        /// An observer that always errors on on_commit
        #[derive(Default)]
        struct ErrorObserver;

        impl Observer for ErrorObserver {
            fn emits(&self) -> Discriminant<Observation> {
                Observation::DUMMY
            }

            fn on_commit(
                &mut self,
                _commit: &gix::Commit,
                _repo: &gix::Repository,
            ) -> eyre::Result<Option<Observation>> {
                Err(eyre::eyre!("intentional test error"))
            }
        }

        let temp_repo = repository::Builder::new()
            .commit("Initial commit")
            .build()
            .unwrap();
        let oid = crate::git::rev::parse("HEAD", &temp_repo.repo).unwrap();

        let observers: Vec<Box<dyn Observer>> =
            vec![Box::new(ErrorObserver), Box::new(AlwaysObserver)];
        let mut engine =
            ObserverEngine::new(&temp_repo.repo, observers, default_mailmap()).unwrap();
        let data = engine.process_commit(oid).unwrap();

        assert_eq!(
            data,
            [
                ObserverData::CommitStart(default_ctx(oid)),
                ObserverData::Observation(Observation::Dummy),
                ObserverData::CommitComplete,
            ]
        );
    }

    #[test]
    fn run_sends_protocol_messages_through_channel() {
        let temp_repo = repository::Builder::new()
            .commit("first")
            .commit("second")
            .build()
            .unwrap();

        let oid1 = crate::git::rev::parse("HEAD~1", &temp_repo.repo).unwrap();
        let oid2 = crate::git::rev::parse("HEAD", &temp_repo.repo).unwrap();

        let observers: Vec<Box<dyn Observer>> = vec![Box::new(AlwaysObserver)];
        let mut engine =
            ObserverEngine::new(&temp_repo.repo, observers, default_mailmap()).unwrap();

        let (tx, rx) = mpsc::channel();
        engine.run(vec![oid1, oid2], &tx).unwrap();
        drop(tx);

        let messages: Vec<_> = rx.iter().collect();
        assert_eq!(
            messages,
            [
                ObserverData::CommitStart(default_ctx(oid1)),
                ObserverData::Observation(Observation::Dummy),
                ObserverData::CommitComplete,
                ObserverData::CommitStart(default_ctx(oid2)),
                ObserverData::Observation(Observation::Dummy),
                ObserverData::CommitComplete,
            ]
        );
    }

    #[test]
    fn run_stops_when_receiver_dropped() {
        let temp_repo = repository::Builder::new()
            .commit("first")
            .commit("second")
            .commit("third")
            .build()
            .unwrap();

        let head = crate::git::rev::parse("HEAD", &temp_repo.repo).unwrap();
        let oids: Vec<_> = crate::git::rev::walk(head, &temp_repo.repo)
            .unwrap()
            .map(|r| r.unwrap())
            .collect();

        let observers: Vec<Box<dyn Observer>> = vec![Box::new(AlwaysObserver)];
        let mut engine =
            ObserverEngine::new(&temp_repo.repo, observers, default_mailmap()).unwrap();

        let (tx, rx) = mpsc::channel();
        drop(rx);

        // Should not error even though the receiver is dropped
        engine.run(oids, &tx).unwrap();
    }

    #[test]
    fn empty_commit_no_diff_observation() {
        // A commit with no file changes -- DummyDiffObserver should not emit
        let temp_repo = repository::Builder::new()
            .commit("first")
            .commit("empty commit")
            .build()
            .unwrap();
        let oid = crate::git::rev::parse("HEAD", &temp_repo.repo).unwrap();

        let observers: Vec<Box<dyn Observer>> = vec![Box::new(DummyDiffObserver::default())];
        let mut engine =
            ObserverEngine::new(&temp_repo.repo, observers, default_mailmap()).unwrap();
        let data = engine.process_commit(oid).unwrap();

        assert_eq!(
            data,
            [
                ObserverData::CommitStart(default_ctx(oid)),
                ObserverData::CommitComplete,
            ]
        );
    }

    #[test]
    fn multiple_always_observers_all_collected() {
        let temp_repo = repository::Builder::new()
            .commit("Initial commit")
            .build()
            .unwrap();
        let oid = crate::git::rev::parse("HEAD", &temp_repo.repo).unwrap();

        let observers: Vec<Box<dyn Observer>> = vec![
            Box::new(AlwaysObserver),
            Box::new(AlwaysObserver),
            Box::new(AlwaysObserver),
        ];
        let mut engine =
            ObserverEngine::new(&temp_repo.repo, observers, default_mailmap()).unwrap();
        let data = engine.process_commit(oid).unwrap();

        assert_eq!(
            data,
            [
                ObserverData::CommitStart(default_ctx(oid)),
                ObserverData::Observation(Observation::Dummy),
                ObserverData::Observation(Observation::Dummy),
                ObserverData::Observation(Observation::Dummy),
                ObserverData::CommitComplete,
            ]
        );
    }

    #[test]
    fn multiple_diff_observers_all_collected() {
        let temp_repo = repository::Builder::new()
            .commit("Initial commit")
            .file("hello.txt", b"hello world")
            .build()
            .unwrap();
        let oid = crate::git::rev::parse("HEAD", &temp_repo.repo).unwrap();

        let observers: Vec<Box<dyn Observer>> = vec![
            Box::new(DummyDiffObserver::default()),
            Box::new(DummyDiffObserver::default()),
        ];
        let mut engine =
            ObserverEngine::new(&temp_repo.repo, observers, default_mailmap()).unwrap();
        let data = engine.process_commit(oid).unwrap();

        assert_eq!(
            data,
            [
                ObserverData::CommitStart(default_ctx(oid)),
                ObserverData::Observation(Observation::Dummy),
                ObserverData::Observation(Observation::Dummy),
                ObserverData::CommitComplete,
            ]
        );
    }

    #[test]
    fn diff_cancel_still_works_per_observer() {
        // DummyDiffObserver cancels after the first change. Verify it still emits Dummy
        // (from on_diff_end) even though it cancelled partway through.
        let temp_repo = repository::Builder::new()
            .commit("Initial commit")
            .file("a.txt", b"aaa")
            .file("b.txt", b"bbb")
            .build()
            .unwrap();
        let oid = crate::git::rev::parse("HEAD", &temp_repo.repo).unwrap();

        let observers: Vec<Box<dyn Observer>> = vec![Box::new(DummyDiffObserver::default())];
        let mut engine =
            ObserverEngine::new(&temp_repo.repo, observers, default_mailmap()).unwrap();
        let data = engine.process_commit(oid).unwrap();

        assert_eq!(
            data,
            [
                ObserverData::CommitStart(default_ctx(oid)),
                ObserverData::Observation(Observation::Dummy),
                ObserverData::CommitComplete,
            ]
        );
    }
}
