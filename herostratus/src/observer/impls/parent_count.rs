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

    #[test]
    fn emits_three_for_octopus_merge() {
        // Distinct timestamps so the rev-walk has a deterministic order.
        let repo = repository::Builder::new()
            .commit("base")
            .time(1_000)
            .build()
            .unwrap();

        // Two side branches diverging from main, then an octopus merge with three parents.
        repo.set_branch("side1").unwrap();
        repo.commit("on side1").time(2_000).create().unwrap();
        repo.set_branch("side2").unwrap();
        repo.commit("on side2").time(3_000).create().unwrap();
        repo.set_branch("main").unwrap();
        repo.merge("side1", "octopus")
            .with_extra_parent("side2")
            .time(4_000)
            .create()
            .unwrap();

        let observations = observe_all(&repo, ParentCountObserver);
        assert_eq!(
            observations,
            [
                Observation::ParentCount { count: 0 }, // base (oldest)
                Observation::ParentCount { count: 1 }, // on side1
                Observation::ParentCount { count: 1 }, // on side2
                Observation::ParentCount { count: 3 }, // octopus merge (main + side1 + side2)
            ]
        );
    }
}
