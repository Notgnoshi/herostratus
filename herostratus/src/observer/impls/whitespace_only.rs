use std::mem::Discriminant;

use crate::bstr::BStr;
use crate::observer::observation::Observation;
use crate::observer::observer::{DiffAction, Observer};
use crate::observer::observer_factory::ObserverFactory;
use crate::utils::is_equal_ignoring_whitespace;

/// Emits [Observation::WhitespaceOnly] when every file change in the commit is a whitespace-only
/// modification.
#[derive(Default)]
pub struct WhitespaceOnlyObserver {
    /// Whether any non-whitespace change was found
    found_non_whitespace_difference: bool,
    /// Whether any change was found at all
    found_any_change: bool,
}

inventory::submit!(ObserverFactory::new::<WhitespaceOnlyObserver>());

impl Observer for WhitespaceOnlyObserver {
    fn emits(&self) -> Discriminant<Observation> {
        Observation::WHITESPACE_ONLY
    }

    fn is_interested_in_diff(&self) -> bool {
        true
    }

    fn on_commit(
        &mut self,
        _commit: &gix::Commit,
        _repo: &gix::Repository,
    ) -> eyre::Result<Option<Observation>> {
        Ok(None)
    }

    fn on_diff_start(&mut self) -> eyre::Result<()> {
        self.found_non_whitespace_difference = false;
        self.found_any_change = false;
        Ok(())
    }

    fn on_diff_change(
        &mut self,
        change: &gix::object::tree::diff::ChangeDetached,
        repo: &gix::Repository,
    ) -> eyre::Result<DiffAction> {
        self.found_any_change = true;

        match change {
            gix::object::tree::diff::ChangeDetached::Modification {
                previous_id,
                id,
                entry_mode,
                ..
            } => {
                if entry_mode.is_commit() {
                    // Submodule updates look like commit entry modes
                    self.found_non_whitespace_difference = true;
                    return Ok(DiffAction::Cancel);
                }
                self.on_modification(repo, *previous_id, *id)
            }

            // Additions, deletions, and rewrites are always non-whitespace changes
            gix::object::tree::diff::ChangeDetached::Addition { .. }
            | gix::object::tree::diff::ChangeDetached::Deletion { .. }
            | gix::object::tree::diff::ChangeDetached::Rewrite { .. } => {
                self.found_non_whitespace_difference = true;
                Ok(DiffAction::Cancel)
            }
        }
    }

    fn on_diff_end(&mut self) -> eyre::Result<Option<Observation>> {
        // Don't claim that empty commits containing no changes are whitespace-only changes!
        if self.found_non_whitespace_difference || !self.found_any_change {
            return Ok(None);
        }
        Ok(Some(Observation::WhitespaceOnly))
    }
}

impl WhitespaceOnlyObserver {
    fn on_modification(
        &mut self,
        repo: &gix::Repository,
        previous_id: gix::ObjectId,
        id: gix::ObjectId,
    ) -> eyre::Result<DiffAction> {
        let before = repo.find_object(previous_id)?;
        let after = repo.find_object(id)?;
        if before.kind == gix::object::Kind::Tree {
            return Ok(DiffAction::Continue);
        }

        let before_s = BStr::new(&before.data);
        let after_s = BStr::new(&after.data);

        if !is_equal_ignoring_whitespace(before_s, after_s) {
            self.found_non_whitespace_difference = true;
            Ok(DiffAction::Cancel)
        } else {
            Ok(DiffAction::Continue)
        }
    }
}

#[cfg(test)]
mod tests {
    use herostratus_tests::fixtures::repository;

    use super::*;
    use crate::observer::impls::test_helpers::observe_all;

    #[test]
    fn whitespace_only_change() {
        let repo = repository::Builder::new()
            .commit("add file")
            .file("hello.txt", b"hello world")
            .commit("whitespace change")
            .file("hello.txt", b"hello  world")
            .build()
            .unwrap();
        let observations = observe_all(&repo, WhitespaceOnlyObserver::default());
        assert_eq!(observations, [Observation::WhitespaceOnly]);
    }

    #[test]
    fn non_whitespace_change() {
        let repo = repository::Builder::new()
            .commit("add file")
            .file("hello.txt", b"hello world")
            .commit("content change")
            .file("hello.txt", b"goodbye world")
            .build()
            .unwrap();
        let observations = observe_all(&repo, WhitespaceOnlyObserver::default());
        assert!(observations.is_empty());
    }

    #[test]
    fn empty_commit_not_detected() {
        // Empty commits (no changes at all) should not be reported as whitespace-only
        let repo = repository::Builder::new()
            .commit("first")
            .commit("empty commit")
            .build()
            .unwrap();
        let observations = observe_all(&repo, WhitespaceOnlyObserver::default());
        assert!(observations.is_empty());
    }

    #[test]
    fn addition_is_not_whitespace_only() {
        let repo = repository::Builder::new()
            .commit("add file")
            .file("hello.txt", b"hello")
            .build()
            .unwrap();
        let observations = observe_all(&repo, WhitespaceOnlyObserver::default());
        assert!(observations.is_empty());
    }
}
