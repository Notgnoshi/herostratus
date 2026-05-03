use std::mem::Discriminant;

use crate::observer::observation::Observation;
use crate::observer::observer::Observer;
use crate::observer::observer_factory::ObserverFactory;

/// Emits [Observation::ParentCount] for every commit.
#[derive(Default)]
pub struct ParentCountObserver;

inventory::submit!(ObserverFactory::new::<ParentCountObserver>());

impl Observer for ParentCountObserver {
    fn emits(&self) -> Discriminant<Observation> {
        Observation::PARENT_COUNT
    }

    fn on_commit(
        &mut self,
        commit: &gix::Commit,
        _repo: &gix::Repository,
    ) -> eyre::Result<Option<Observation>> {
        let count = commit.parent_ids().count();
        Ok(Some(Observation::ParentCount { count }))
    }
}

#[cfg(test)]
mod tests {
    use herostratus_tests::fixtures::repository;

    use super::*;
    use crate::observer::impls::test_helpers::observe_all;

    #[test]
    fn emits_zero_for_root_commit() {
        let repo = repository::Builder::new().commit("root").build().unwrap();
        let observations = observe_all(&repo, ParentCountObserver);
        assert_eq!(observations, [Observation::ParentCount { count: 0 }]);
    }

    #[test]
    fn emits_one_for_linear_commits() {
        let repo = repository::Builder::new()
            .commit("first")
            .commit("second")
            .commit("third")
            .build()
            .unwrap();
        let observations = observe_all(&repo, ParentCountObserver);
        assert_eq!(
            observations,
            [
                Observation::ParentCount { count: 0 },
                Observation::ParentCount { count: 1 },
                Observation::ParentCount { count: 1 },
            ]
        );
    }
}
