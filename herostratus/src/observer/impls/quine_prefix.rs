use std::mem::Discriminant;

use crate::observer::observation::Observation;
use crate::observer::observer::Observer;
use crate::observer::observer_factory::ObserverFactory;

/// Emits [Observation::QuinePrefix] when the commit message contains a prefix of its own commit
/// hash.
///
/// Checks all prefixes of the hex-encoded commit hash (from longest to shortest, minimum 5
/// characters) and reports the longest match found anywhere in the full commit message.
#[derive(Default)]
pub struct QuinePrefixObserver;

inventory::submit!(ObserverFactory::new::<QuinePrefixObserver>());

/// Minimum prefix length the observer will report. Shorter matches are too common to be
/// interesting.
const MIN_OBSERVER_PREFIX: usize = 5;

impl Observer for QuinePrefixObserver {
    fn emits(&self) -> Discriminant<Observation> {
        Observation::QUINE_PREFIX
    }

    fn on_commit(
        &mut self,
        commit: &gix::Commit,
        _repo: &gix::Repository,
    ) -> eyre::Result<Option<Observation>> {
        let hex = commit.id().to_string();
        let message = commit.message_raw()?;
        let message = String::from_utf8_lossy(message.as_ref());

        // Search from longest prefix down to MIN_OBSERVER_PREFIX
        for len in (MIN_OBSERVER_PREFIX..=hex.len()).rev() {
            let prefix = &hex[..len];
            if message.contains(prefix) {
                return Ok(Some(Observation::QuinePrefix {
                    matched_length: len,
                }));
            }
        }

        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use herostratus_tests::fixtures::repository;

    use super::*;
    use crate::observer::impls::test_helpers::observe_all;

    #[test]
    fn no_match_on_normal_commit() {
        let repo = repository::Builder::new()
            .commit("Just a normal commit message")
            .build()
            .unwrap();
        let observations = observe_all(&repo, QuinePrefixObserver);
        assert!(
            observations.is_empty(),
            "Normal commit should not match its own hash prefix"
        );
    }

    // There's an integration test against a real quine commit. Not feasible to write a positive
    // test against a TempRepository, so we just test against the origin/test/quine orphan branch.
}
