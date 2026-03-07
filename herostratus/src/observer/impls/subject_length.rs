use std::mem::Discriminant;

use crate::observer::observation::Observation;
use crate::observer::observer::Observer;
use crate::observer::observer_factory::ObserverFactory;

/// Emits [Observation::SubjectLength] for every commit, carrying the byte length of the subject
/// line.
#[derive(Default)]
pub struct SubjectLengthObserver;

inventory::submit!(ObserverFactory::new::<SubjectLengthObserver>());

impl Observer for SubjectLengthObserver {
    fn emits(&self) -> Discriminant<Observation> {
        Observation::SUBJECT_LENGTH
    }

    #[tracing::instrument(
        target = "perf",
        level = "debug",
        name = "SubjectLength::on_commit",
        skip_all
    )]
    fn on_commit(
        &mut self,
        commit: &gix::Commit,
        _repo: &gix::Repository,
    ) -> eyre::Result<Option<Observation>> {
        let msg = commit.message()?;
        // Number of bytes, not number of characters, but that's fine for our purposes
        let length = msg.title.len();
        Ok(Some(Observation::SubjectLength { length }))
    }
}

#[cfg(test)]
mod tests {
    use herostratus_tests::fixtures::repository;

    use super::*;
    use crate::observer::impls::test_helpers::observe_all;

    #[test]
    fn emits_length() {
        let repo = repository::Builder::new().commit("Hello").build().unwrap();
        let observations = observe_all(&repo, SubjectLengthObserver);
        assert_eq!(observations, [Observation::SubjectLength { length: 5 }]);
    }

    #[test]
    fn multiple_commits() {
        let repo = repository::Builder::new()
            .commit("Hi")
            .commit("Hello world")
            .build()
            .unwrap();
        let observations = observe_all(&repo, SubjectLengthObserver);
        assert_eq!(
            observations,
            [
                Observation::SubjectLength { length: 2 },
                Observation::SubjectLength { length: 11 },
            ]
        );
    }
}
