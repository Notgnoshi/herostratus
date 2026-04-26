use std::mem::Discriminant;

use rustrict::{Censor, Type};

use crate::observer::observation::Observation;
use crate::observer::observer::Observer;
use crate::observer::observer_factory::ObserverFactory;

/// Words that rustrict classifies as PROFANE or OFFENSIVE but that we do not want to
/// count as profanity in commit messages. Compared case-insensitively.
const ALLOWED_WORDS: &[&str] = &["slave"];

/// Returns true when `word` is classified as profanity by rustrict and is not in our
/// allowlist.
///
/// If this filter ever needs richer customization than a small const list, enable
/// rustrict's "customize" feature and register words at startup.
fn is_profane(word: &str) -> bool {
    let analysis = Censor::from_str(word).analyze();
    if !analysis.is((Type::PROFANE | Type::OFFENSIVE) & Type::MILD_OR_HIGHER) {
        return false;
    }
    !ALLOWED_WORDS
        .iter()
        .any(|safe| word.eq_ignore_ascii_case(safe))
}

/// Returns every profane word found in `text`, lowercased, in the order they appear.
///
/// May contain duplicates.
fn find_profane_words<S: AsRef<str>>(text: S) -> Vec<String> {
    text.as_ref()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty() && is_profane(w))
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

    const DEV_TERMS: &[&str] = &[
        "kill",
        "abort",
        "dummy",
        "hack",
        "crack",
        "exec",
        "dump",
        "zombie",
        "daemon",
        "orphan",
        "blacklist",
        "whitelist",
        "master",
        "slave",
        "assertion",
        "assert",
        "assassin",
        "hello",
        "adam",
        "classic",
        "kill_proc",
        "abort_signal",
        "dump_state",
        "exec_path",
        "stupid",
        "idiot",
        "moron",
        "dumb",
    ];

    #[test]
    fn dev_terms_not_flagged_as_profanity() {
        for term in DEV_TERMS {
            let found = find_profane_words(term);
            assert!(
                found.is_empty(),
                "dev term {term:?} was unexpectedly flagged as profanity: {found:?}"
            );
        }
    }

    #[test]
    fn detects_leet_speak() {
        let found = find_profane_words("this sh1t is broken");
        assert!(!found.is_empty(), "rustrict should catch sh1t: {found:?}");

        let found = find_profane_words("fuuuck");
        assert!(
            !found.is_empty(),
            "rustrict should catch repeated-character obfuscation: {found:?}"
        );
    }
}
