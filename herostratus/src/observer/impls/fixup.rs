use std::mem::Discriminant;

use crate::observer::observation::Observation;
use crate::observer::observer::Observer;
use crate::observer::observer_factory::ObserverFactory;

const FIXUP_PREFIXES: &[&str] = &[
    "fixup!", "squash!", "amend!", "WIP", "TODO", "FIXME", "DROPME",
    // Avoid false positives by accepting false negatives. Of all these patterns, "wip" is the one
    // that's most likely to be a part of a real word.
    "wip:", "todo", "fixme", "dropme",
];

/// Emits [Observation::Fixup] when the commit subject starts with a fixup/squash/amend/WIP/etc
/// prefix.
#[derive(Default)]
pub struct FixupObserver;

inventory::submit!(ObserverFactory::new::<FixupObserver>());

impl Observer for FixupObserver {
    fn emits(&self) -> Discriminant<Observation> {
        Observation::FIXUP
    }

    fn on_commit(
        &mut self,
        commit: &gix::Commit,
        _repo: &gix::Repository,
    ) -> eyre::Result<Option<Observation>> {
        let msg = commit.message()?;
        let found = FIXUP_PREFIXES
            .iter()
            .any(|p| msg.title.starts_with(p.as_bytes()));
        Ok(found.then_some(Observation::Fixup))
    }
}

#[cfg(test)]
mod tests {
    use herostratus_tests::fixtures::repository;

    use super::*;
    use crate::observer::impls::test_helpers::observe_all;

    #[test]
    fn fixup_prefix() {
        let repo = repository::Builder::new()
            .commit("fixup! some commit")
            .build()
            .unwrap();
        let observations = observe_all(&repo, FixupObserver);
        assert_eq!(observations, [Observation::Fixup]);
    }

    #[test]
    fn squash_prefix() {
        let repo = repository::Builder::new()
            .commit("squash! some commit")
            .build()
            .unwrap();
        let observations = observe_all(&repo, FixupObserver);
        assert_eq!(observations, [Observation::Fixup]);
    }

    #[test]
    fn normal_commit() {
        let repo = repository::Builder::new()
            .commit("Normal commit message")
            .build()
            .unwrap();
        let observations = observe_all(&repo, FixupObserver);
        assert!(observations.is_empty());
    }

    #[test]
    fn case_sensitive_wip() {
        let repo = repository::Builder::new()
            .commit("WIP something")
            .build()
            .unwrap();
        let observations = observe_all(&repo, FixupObserver);
        assert_eq!(observations, [Observation::Fixup]);
    }

    #[test]
    fn lowercase_wip_needs_colon() {
        let repo = repository::Builder::new()
            .commit("wip: something")
            .build()
            .unwrap();
        let observations = observe_all(&repo, FixupObserver);
        assert_eq!(observations, [Observation::Fixup]);
    }

    #[test]
    fn wip_without_colon_no_match() {
        let repo = repository::Builder::new()
            .commit("wipe out old data")
            .build()
            .unwrap();
        let observations = observe_all(&repo, FixupObserver);
        assert!(observations.is_empty());
    }
}
