mod empty_commit;
mod fixup;
mod non_unicode;
mod profanity;
mod subject_length;
mod whitespace_only;

#[cfg(test)]
pub(crate) mod test_helpers {
    use herostratus_tests::fixtures::repository::TempRepository;

    use crate::git::mailmap::MailmapResolver;
    use crate::observer::{Observation, Observer, ObserverData, ObserverEngine};

    /// Run a single observer against all commits in the repo (oldest first) and collect the
    /// observations it emits.
    pub fn observe_all(
        repo: &TempRepository,
        observer: impl Observer + 'static,
    ) -> Vec<Observation> {
        let mailmap = MailmapResolver::new(gix::mailmap::Snapshot::default(), None, None).unwrap();
        let observers: Vec<Box<dyn Observer>> = vec![Box::new(observer)];
        let mut engine = ObserverEngine::new(&repo.repo, observers, mailmap).unwrap();

        let head = crate::git::rev::parse("HEAD", &repo.repo).unwrap();
        let oids: Vec<_> = crate::git::rev::walk(head, &repo.repo)
            .unwrap()
            .map(|r| r.unwrap())
            .collect();
        // Walk returns newest-first; reverse to process oldest-first
        let oids: Vec<_> = oids.into_iter().rev().collect();

        let (tx, rx) = std::sync::mpsc::channel();
        engine.run(oids, &tx).unwrap();
        drop(tx);

        rx.iter()
            .filter_map(|msg| match msg {
                ObserverData::Observation(obs) => Some(obs),
                _ => None,
            })
            .collect()
    }
}
