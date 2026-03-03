use std::mem::Discriminant;

use crate::observer::observation::Observation;
use crate::observer::observer::Observer;
use crate::observer::observer_factory::ObserverFactory;

/// Emits [Observation::NonUnicodeMessage] when the raw commit message contains bytes that are not
/// valid UTF-8.
#[derive(Default)]
pub struct NonUnicodeObserver;

inventory::submit!(ObserverFactory::new::<NonUnicodeObserver>());

impl Observer for NonUnicodeObserver {
    fn emits(&self) -> Discriminant<Observation> {
        Observation::NON_UNICODE_MESSAGE
    }

    fn on_commit(
        &mut self,
        commit: &gix::Commit,
        _repo: &gix::Repository,
    ) -> eyre::Result<Option<Observation>> {
        let bytes = commit.message_raw_sloppy();
        let is_non_utf8 = std::str::from_utf8(bytes).is_err();
        Ok(is_non_utf8.then_some(Observation::NonUnicodeMessage))
    }
}

// It's not possible to create a commit containing non-unicode bytes from gix or git2, so there's
// no unit test here. There's an integration test against a hand-crafted branch with non-unicode
// bytes in the commit message. See h004_non_unicode.rs for details.
