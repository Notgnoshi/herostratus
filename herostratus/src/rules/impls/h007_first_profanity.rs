use std::mem::Discriminant;

use crate::achievement::{AchievementKind, Grant, Meta};
use crate::observer::{CommitContext, Observation};
use crate::rules::rule::Rule;
use crate::rules::rule_plugin::RuleFactory;

const META: Meta = Meta {
    id: 7,
    human_id: "first-profanity",
    name: "First!",
    description: "Be the first person to swear in the repository",
    kind: AchievementKind::Global { revocable: false },
};

/// Grant an achievement to the first person who swears in the repository.
///
/// Since the pipeline walks commits newest-first, the last profanity observation seen is the actual
/// first in the repository. This rule accumulates state and grants at finalize.
///
/// Once the first swearer has been determined and persisted, the cache stores the commit hash so
/// subsequent runs skip processing entirely.
#[derive(Default)]
pub struct FirstProfanity {
    /// The commit hash from a previous run, if already settled.
    settled_commit: Option<String>,
    earliest: Option<Grant>,
}

inventory::submit!(RuleFactory::default::<FirstProfanity>());

/// Stores the commit hash of the first profane commit once determined.
#[derive(Default, Clone, serde::Deserialize, serde::Serialize)]
pub struct FirstProfanityCache {
    commit: Option<String>,
}

impl Rule for FirstProfanity {
    type Cache = FirstProfanityCache;
    const VERSION: u32 = 2;

    fn meta(&self) -> &Meta {
        &META
    }

    fn consumes(&self) -> &'static [Discriminant<Observation>] {
        &[Observation::PROFANITY]
    }

    fn process(&mut self, ctx: &CommitContext, obs: &Observation) -> eyre::Result<Option<Grant>> {
        if self.settled_commit.is_some() {
            return Ok(None);
        }
        if matches!(obs, Observation::Profanity { .. }) {
            self.earliest = Some(META.grant(ctx));
        }
        Ok(None)
    }

    fn finalize(&mut self) -> eyre::Result<Option<Grant>> {
        if self.settled_commit.is_some() {
            return Ok(None);
        }
        if let Some(ref grant) = self.earliest {
            self.settled_commit = Some(grant.commit.to_string());
        }
        Ok(self.earliest.take())
    }

    fn init_cache(&mut self, cache: Self::Cache) {
        self.settled_commit = cache.commit;
    }

    fn fini_cache(&self) -> Self::Cache {
        FirstProfanityCache {
            commit: self.settled_commit.clone(),
        }
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    fn profanity() -> Observation {
        Observation::Profanity {
            words: vec!["damn".to_string()],
        }
    }

    #[test]
    fn grants_last_seen_at_finalize() {
        let mut rule = FirstProfanity::default();
        // Walk order is newest-first, so Alice is visited first, then Bob.
        // Bob being last means Bob was the actual first swearer in the repo.
        let alice = CommitContext::test("Alice");
        let bob = CommitContext::test("Bob");

        assert!(rule.process(&alice, &profanity()).unwrap().is_none());
        rule.process(&bob, &profanity()).unwrap();

        let grant = rule.finalize().unwrap().unwrap();
        assert_eq!(grant.user_email, "bob@example.com");

        let cache = rule.fini_cache();
        assert!(cache.commit.is_some());
    }

    #[test]
    fn settled_cache_skips_processing() {
        let mut rule = FirstProfanity::default();
        rule.init_cache(FirstProfanityCache {
            commit: Some("abc123".to_string()),
        });

        let ctx = CommitContext::test("Alice");
        rule.process(&ctx, &profanity()).unwrap();

        let grant = rule.finalize().unwrap();
        assert!(grant.is_none(), "settled rule should not grant");
    }
}
