use std::mem::Discriminant;

use rustrict::{Censor, Type};

use crate::observer::observation::Observation;
use crate::observer::observer::Observer;
use crate::observer::observer_factory::ObserverFactory;

/// Words that rustrict classifies as PROFANE or OFFENSIVE but that we do not want to
/// count as profanity in commit messages. Compared case-insensitively.
const ALLOWED_WORDS: &[&str] = &["slave"];

/// Returns true when `word` plausibly resembles a natural-language word using the ratio of letters
/// to digits as a heuristic.
///
/// This lets us keep rustrict's leet-speak detection, while avoiding false positives on git
/// hashes, timestamps, benchmark results, etc.
fn looks_like_word(word: &str) -> bool {
    let (letters, digits) = word.chars().fold((0usize, 0usize), |(l, d), c| {
        if c.is_alphabetic() {
            (l + 1, d)
        } else if c.is_ascii_digit() {
            (l, d + 1)
        } else {
            (l, d)
        }
    });
    letters > digits
}

/// Returns true when `word` is classified as profanity by rustrict and the match spans the entire
/// token.
///
/// rustrict will match a profane substring inside a longer token (e.g. `Scunthorpe` matches
/// `cunt`, `Dickinson` matches `dick`). We require the censor to replace every character in the
/// token, which rejects substring matches.
fn is_profane(word: &str) -> bool {
    let mut censor = Censor::from_str(word);
    censor.with_censor_first_character_threshold(Type::ANY);
    let (censored, analysis) = censor.censor_and_analyze();
    if !analysis.is((Type::PROFANE | Type::OFFENSIVE) & Type::MILD_OR_HIGHER) {
        return false;
    }
    // censor_and_analyze appends a trailing space before censoring stops.
    let trimmed = censored.trim();
    let token_level_match =
        trimmed.chars().count() == word.chars().count() && trimmed.chars().all(|c| c == '*');
    if !token_level_match {
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
        .filter(|w| !w.is_empty() && looks_like_word(w) && is_profane(w))
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
        // Single-character substitutions and repeated-character obfuscation must
        // still fire; the layered rule only rejects substring matches and
        // numeric-heavy tokens.
        for text in [
            "this sh1t is broken",
            "fuuuck",
            "this 5hit is broken",
            "f4g",
            "a55hole",
        ] {
            let found = find_profane_words(text);
            assert!(
                !found.is_empty(),
                "rustrict should catch leet variant: {text:?} -> {found:?}"
            );
        }
    }

    /// These are real false positives that we want to exclude
    #[test]
    fn no_false_positives_on_numeric_tokens() {
        let cases = [
            // fa9 => fag
            "See https://github.com/Notgnoshi/herostratus/commit/588b41b6e983c393df17689d7659145fbce16fa9",
            // 69514Z => spaz
            "2024-04-06T20:32:46.069514Z DEBUG herostratus::git::clone",
            // 48501x => ahole
            "Estimated Cycles: (+148.501x)",
        ];
        for text in cases {
            let found = find_profane_words(text);
            assert!(
                found.is_empty(),
                "numeric-heavy text should not be flagged: {text:?} -> {found:?}"
            );
        }
    }

    #[test]
    fn no_false_positives_on_substring_matches() {
        for text in [
            "Scunthorpe is a town in England",
            "shittake mushrooms are tasty",
            "Dickinson hauled the load",
            "ohmyfuckinggod", // we can't distinguish this from Scunthorpe style filtering.
        ] {
            let found = find_profane_words(text);
            assert!(
                found.is_empty(),
                "substring-only match should not be flagged: {text:?} -> {found:?}"
            );
        }
    }
}
