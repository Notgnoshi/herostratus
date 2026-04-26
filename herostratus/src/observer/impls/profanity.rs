use std::mem::Discriminant;

use crate::observer::observation::Observation;
use crate::observer::observer::Observer;
use crate::observer::observer_factory::ObserverFactory;

// TODO: This is somewhat naive. It could be improved
const PROFANITY: &[&str] = &[
    "shit", "fuck", "fucking", "damn", "ass", "hell", "bitch", "bastard", "piss",
];

/// Returns every profane word found in `text`, lowercased, in the order they appear.
///
/// May contain duplicates.
fn find_profane_words<S: AsRef<str>>(text: S) -> Vec<String> {
    text.as_ref()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| PROFANITY.iter().any(|p| w.eq_ignore_ascii_case(p)))
        .map(|w| w.to_ascii_lowercase())
        .collect()
}

/// Emits [Observation::Profanity] when the commit message contains profanity
#[derive(Default)]
pub struct ProfanityObserver;

inventory::submit!(ObserverFactory::new::<ProfanityObserver>());

impl Observer for ProfanityObserver {
    fn emits(&self) -> Discriminant<Observation> {
        Observation::PROFANITY
    }

    #[tracing::instrument(
        target = "perf",
        level = "debug",
        name = "Profanity::on_commit",
        skip_all
    )]
    fn on_commit(
        &mut self,
        commit: &gix::Commit,
        _repo: &gix::Repository,
    ) -> eyre::Result<Option<Observation>> {
        let raw = commit.message_raw()?;
        let text = String::from_utf8_lossy(raw.as_ref());
        let words = find_profane_words(text.as_ref());
        Ok((!words.is_empty()).then_some(Observation::Profanity { words }))
    }
}

#[cfg(test)]
mod tests {
    use herostratus_tests::fixtures::repository;

    use super::*;
    use crate::observer::impls::test_helpers::observe_all;

    fn profanity(words: &[&str]) -> Observation {
        Observation::Profanity {
            words: words.iter().map(|w| w.to_string()).collect(),
        }
    }

    #[test]
    fn detects_profanity_in_subject() {
        let repo = repository::Builder::new()
            .commit("Fix the damn tests")
            .build()
            .unwrap();
        let observations = observe_all(&repo, ProfanityObserver);
        assert_eq!(observations, [profanity(&["damn"])]);
    }

    #[test]
    fn detects_profanity_case_insensitive() {
        let repo = repository::Builder::new()
            .commit("SHIT this is broken")
            .build()
            .unwrap();
        let observations = observe_all(&repo, ProfanityObserver);
        assert_eq!(observations, [profanity(&["shit"])]);
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
    fn detects_all_profanities_in_message() {
        let repo = repository::Builder::new()
            .commit("Fix tests\n\nThis fucking shit was broken")
            .build()
            .unwrap();
        let observations = observe_all(&repo, ProfanityObserver);
        assert_eq!(observations, [profanity(&["fucking", "shit"])]);
    }
}
