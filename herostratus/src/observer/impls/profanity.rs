use std::mem::Discriminant;

use gix::bstr::ByteSlice;

use crate::observer::observation::Observation;
use crate::observer::observer::Observer;
use crate::observer::observer_factory::ObserverFactory;

// TODO: This is somewhat naive. It could be improved
const PROFANITY: &[&str] = &[
    "shit", "fuck", "fucking", "damn", "ass", "hell", "bitch", "bastard", "piss",
];

/// Returns the first profane word found in `text`, lowercased.
///
/// Splits on non-alphanumeric boundaries and checks each word against the profanity list
/// (case-insensitive).
fn find_profane_word<S: AsRef<str>>(text: S) -> Option<String> {
    text.as_ref()
        .split(|c: char| !c.is_alphanumeric())
        .find(|w| PROFANITY.iter().any(|p| w.eq_ignore_ascii_case(p)))
        .map(|w| w.to_ascii_lowercase())
}

/// Emits [Observation::Profanity] when the commit message contains a profanity keyword.
#[derive(Default)]
pub struct ProfanityObserver;

inventory::submit!(ObserverFactory::new::<ProfanityObserver>());

impl Observer for ProfanityObserver {
    fn emits(&self) -> Discriminant<Observation> {
        Observation::PROFANITY
    }

    fn on_commit(
        &mut self,
        commit: &gix::Commit,
        _repo: &gix::Repository,
    ) -> eyre::Result<Option<Observation>> {
        let msg = commit.message()?;
        // Check both subject and body for profanity
        let mut found = find_profane_word(msg.title.to_str_lossy());
        if found.is_none()
            && let Some(body) = msg.body
        {
            found = find_profane_word(body.to_str_lossy());
        }
        // TODO: This only returns the first result found, because we can't emit multiple
        // observations for a single commit. If we want to count multiple profanities, we'd either
        // need to support emitting multiple observations per commit or encode multiple words in a
        // single observation.
        Ok(found.map(|word| Observation::Profanity { word }))
    }
}

#[cfg(test)]
mod tests {
    use herostratus_tests::fixtures::repository;

    use super::*;
    use crate::observer::impls::test_helpers::observe_all;

    fn profanity(word: &str) -> Observation {
        Observation::Profanity {
            word: word.to_string(),
        }
    }

    #[test]
    fn detects_profanity_in_subject() {
        let repo = repository::Builder::new()
            .commit("Fix the damn tests")
            .build()
            .unwrap();
        let observations = observe_all(&repo, ProfanityObserver);
        assert_eq!(observations, [profanity("damn")]);
    }

    #[test]
    fn detects_profanity_case_insensitive() {
        let repo = repository::Builder::new()
            .commit("SHIT this is broken")
            .build()
            .unwrap();
        let observations = observe_all(&repo, ProfanityObserver);
        assert_eq!(observations, [profanity("shit")]);
    }

    #[test]
    fn no_profanity_in_clean_message() {
        let repo = repository::Builder::new()
            .commit("Add unit tests for the parser")
            .build()
            .unwrap();
        let observations = observe_all(&repo, ProfanityObserver);
        assert!(observations.is_empty());
    }

    #[test]
    fn no_false_positive_on_substring() {
        let repo = repository::Builder::new()
            .commit("Add assertion helpers")
            .build()
            .unwrap();
        let observations = observe_all(&repo, ProfanityObserver);
        assert!(observations.is_empty());
    }

    #[test]
    fn detects_profanity_in_body() {
        let repo = repository::Builder::new()
            .commit("Fix tests\n\nThis fucking shit was broken")
            .build()
            .unwrap();
        let observations = observe_all(&repo, ProfanityObserver);
        assert_eq!(observations, [profanity("fucking")]); // only yields first occurrence
    }
}
