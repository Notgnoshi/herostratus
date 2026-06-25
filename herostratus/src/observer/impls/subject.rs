use std::mem::Discriminant;

use crate::observer::observation::Observation;
use crate::observer::observer::Observer;
use crate::observer::observer_factory::ObserverFactory;

/// Emits [Observation::Subject] for every commit, carrying the subject line as a lossy-UTF-8 string.
#[derive(Default)]
pub struct SubjectObserver;
inventory::submit!(ObserverFactory::new::<SubjectObserver>());

impl Observer for SubjectObserver {
    fn emits(&self) -> Discriminant<Observation> {
        Observation::SUBJECT
    }

    #[tracing::instrument(
        target = "perf",
        level = "debug",
        name = "Subject::on_commit",
        skip_all
    )]
    fn on_commit(
        &mut self,
        commit: &gix::Commit,
        _repo: &gix::Repository,
    ) -> eyre::Result<Option<Observation>> {
        let msg = commit.message()?;
        let subject = String::from_utf8_lossy(msg.title).into_owned();
        Ok(Some(Observation::Subject { subject }))
    }
}
