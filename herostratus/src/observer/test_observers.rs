use std::mem::Discriminant;

use super::observation::Observation;
use super::observer::{DiffAction, Observer};

/// An observer that never emits anything.
#[derive(Default)]
pub struct NeverObserver;

impl Observer for NeverObserver {
    fn emits(&self) -> Discriminant<Observation> {
        Observation::DUMMY
    }

    fn on_commit(
        &mut self,
        _commit: &gix::Commit,
        _repo: &gix::Repository,
    ) -> eyre::Result<Option<Observation>> {
        Ok(None)
    }
}

/// An observer that always emits [Observation::Dummy].
#[derive(Default)]
pub struct AlwaysObserver;

impl Observer for AlwaysObserver {
    fn emits(&self) -> Discriminant<Observation> {
        Observation::DUMMY
    }

    fn on_commit(
        &mut self,
        _commit: &gix::Commit,
        _repo: &gix::Repository,
    ) -> eyre::Result<Option<Observation>> {
        Ok(Some(Observation::Dummy))
    }
}

/// A diff observer that tracks how many changes it saw and emits [Observation::Dummy] if any.
#[derive(Default)]
pub struct DummyDiffObserver {
    found_change: bool,
}

impl Observer for DummyDiffObserver {
    fn emits(&self) -> Discriminant<Observation> {
        Observation::DUMMY
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
        self.found_change = false;
        Ok(())
    }

    fn on_diff_change(
        &mut self,
        _change: &gix::object::tree::diff::Change,
        _repo: &gix::Repository,
    ) -> eyre::Result<DiffAction> {
        self.found_change = true;
        Ok(DiffAction::Cancel)
    }

    fn on_diff_end(&mut self) -> eyre::Result<Option<Observation>> {
        Ok(self.found_change.then_some(Observation::Dummy))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::observer::observer_factory::ObserverFactory;

    #[test]
    fn test_factory_builds_observer() {
        let factory = ObserverFactory::new::<AlwaysObserver>();
        let observer = factory.build();
        assert_eq!(observer.emits(), Observation::DUMMY);
    }

    #[test]
    fn test_builtin_observers_doesnt_generate_dummies() {
        // builtin_observers() only returns observers registered via inventory::submit!.
        // Test observers are not registered, so this should not include them.
        let observers = crate::observer::observer_factory::builtin_observers();
        for observer in &observers {
            assert_ne!(
                observer.emits(),
                Observation::DUMMY,
                "test observers should not be registered as builtins"
            );
        }
    }

    #[test]
    fn test_never_observer_emits_nothing() {
        let observer = NeverObserver;
        assert_eq!(observer.emits(), Observation::DUMMY);
        assert!(!observer.is_interested_in_diff());
    }

    #[test]
    fn test_always_observer_emits_dummy() {
        let observer = AlwaysObserver;
        assert_eq!(observer.emits(), Observation::DUMMY);
        assert!(!observer.is_interested_in_diff());
    }

    #[test]
    fn test_dummy_diff_observer_is_interested_in_diff() {
        let observer = DummyDiffObserver::default();
        assert!(observer.is_interested_in_diff());
    }
}
