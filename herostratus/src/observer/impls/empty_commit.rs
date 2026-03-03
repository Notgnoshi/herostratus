use std::mem::Discriminant;

use crate::observer::observation::Observation;
use crate::observer::observer::{DiffAction, Observer};
use crate::observer::observer_factory::ObserverFactory;

/// Emits [Observation::EmptyCommit] when a non-merge commit introduces no file changes.
///
/// Merge commits are excluded -- an empty diff is expected and normal for them.
#[derive(Default)]
pub struct EmptyCommitObserver {
    is_merge: bool,
    found_any_change: bool,
}

inventory::submit!(ObserverFactory::new::<EmptyCommitObserver>());

impl Observer for EmptyCommitObserver {
    fn emits(&self) -> Discriminant<Observation> {
        Observation::EMPTY_COMMIT
    }

    fn is_interested_in_diff(&self) -> bool {
        true
    }

    fn on_commit(
        &mut self,
        commit: &gix::Commit,
        _repo: &gix::Repository,
    ) -> eyre::Result<Option<Observation>> {
        self.is_merge = commit.parent_ids().count() > 1;
        Ok(None)
    }

    fn on_diff_start(&mut self) -> eyre::Result<()> {
        self.found_any_change = false;
        Ok(())
    }

    fn on_diff_change(
        &mut self,
        _change: &gix::object::tree::diff::ChangeDetached,
        _repo: &gix::Repository,
    ) -> eyre::Result<DiffAction> {
        if self.is_merge {
            return Ok(DiffAction::Cancel);
        }
        self.found_any_change = true;
        // One change is enough to know it's not empty
        Ok(DiffAction::Cancel)
    }

    fn on_diff_end(&mut self) -> eyre::Result<Option<Observation>> {
        if self.is_merge || self.found_any_change {
            return Ok(None);
        }
        Ok(Some(Observation::EmptyCommit))
    }
}

#[cfg(test)]
mod tests {
    use herostratus_tests::fixtures::repository;

    use super::*;
    use crate::observer::impls::test_helpers::observe_all;

    #[test]
    fn empty_commit_detected() {
        // The repository::Builder creates empty commits by default (no file changes)
        let repo = repository::Builder::new()
            .commit("first")
            .commit("empty commit")
            .build()
            .unwrap();
        let observations = observe_all(&repo, EmptyCommitObserver::default());
        // Both commits are empty (no file changes)
        assert_eq!(
            observations,
            [Observation::EmptyCommit, Observation::EmptyCommit]
        );
    }

    #[test]
    fn non_empty_commit_not_detected() {
        let repo = repository::Builder::new()
            .commit("add file")
            .file("hello.txt", b"hello")
            .build()
            .unwrap();
        let observations = observe_all(&repo, EmptyCommitObserver::default());
        assert!(observations.is_empty());
    }

    #[test]
    fn mixed_commits() {
        let repo = repository::Builder::new()
            .commit("add file")
            .file("hello.txt", b"hello")
            .commit("empty follow-up")
            .build()
            .unwrap();
        let observations = observe_all(&repo, EmptyCommitObserver::default());
        assert_eq!(observations, [Observation::EmptyCommit]);
    }
}
