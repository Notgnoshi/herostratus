use std::collections::HashMap;
use std::mem::Discriminant;

use crate::achievement::{AchievementKind, Grant, Meta};
use crate::observer::{CommitContext, Observation};
use crate::rules::rule::Rule;
use crate::rules::rule_plugin::RuleFactory;

const META: Meta = Meta {
    id: 10,
    human_id: "most-profound",
    name: "Most Profound",
    description: "The author with the most profanity in their commit messages",
    kind: AchievementKind::Global { revocable: true },
};

/// Grant an achievement to the most profane author in the repository.
///
/// Tracks per-user profanity counts in its cache. At finalize, grants to the user with the highest
/// count. The AchievementLog handles revoking the previous holder when a new leader emerges.
#[derive(Default)]
pub struct MostProfound {
    counts: HashMap<String, usize>,
    leader: Option<(String, String)>,
}

inventory::submit!(RuleFactory::default::<MostProfound>());

#[derive(Default, Clone, serde::Deserialize, serde::Serialize)]
pub struct MostProfoundCache {
    counts: HashMap<String, usize>,
    /// The current leader, so their name survives cache round-trips even if they don't appear in
    /// the next run.
    leader: Option<(String, String)>,
}

impl Rule for MostProfound {
    type Cache = MostProfoundCache;

    fn meta(&self) -> &Meta {
        &META
    }

    fn consumes(&self) -> &'static [Discriminant<Observation>] {
        &[Observation::PROFANITY]
    }

    fn process(&mut self, ctx: &CommitContext, obs: &Observation) -> eyre::Result<Option<Grant>> {
        if !matches!(obs, Observation::Profanity { .. }) {
            return Ok(None);
        }

        let count = self.counts.entry(ctx.author_email.clone()).or_insert(0);
        *count += 1;
        let count = *count;

        // Track the current leader so we can construct a Grant in finalize
        let leader_count = self
            .leader
            .as_ref()
            .and_then(|(_, email)| self.counts.get(email).copied())
            .unwrap_or(0);

        if count > leader_count {
            self.leader = Some((ctx.author_name.clone(), ctx.author_email.clone()));
        }

        Ok(None)
    }

    fn finalize(&mut self) -> eyre::Result<Option<Grant>> {
        let Some((ref name, ref email)) = self.leader else {
            return Ok(None);
        };
        Ok(Some(Grant {
            commit: gix::ObjectId::null(gix::hash::Kind::Sha1),
            author_name: name.clone(),
            author_email: email.clone(),
        }))
    }

    fn init_cache(&mut self, cache: Self::Cache) {
        self.counts = cache.counts;
        self.leader = cache.leader;
    }

    fn fini_cache(&self) -> Self::Cache {
        MostProfoundCache {
            counts: self.counts.clone(),
            leader: self.leader.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx_with(name: &str, email: &str) -> CommitContext {
        CommitContext {
            oid: gix::ObjectId::null(gix::hash::Kind::Sha1),
            author_name: name.to_string(),
            author_email: email.to_string(),
        }
    }

    fn profanity() -> Observation {
        Observation::Profanity {
            word: "damn".to_string(),
        }
    }

    #[test]
    fn grants_to_top_swearer_at_finalize() {
        let mut rule = MostProfound::default();
        let alice = ctx_with("Alice", "alice@example.com");
        let bob = ctx_with("Bob", "bob@example.com");

        // Alice swears 3 times
        for _ in 0..3 {
            rule.process(&alice, &profanity()).unwrap();
        }
        // Bob swears 1 time
        rule.process(&bob, &profanity()).unwrap();

        let grant = rule.finalize().unwrap();
        assert!(grant.is_some());
        let grant = grant.unwrap();
        assert_eq!(grant.author_email, "alice@example.com");
    }

    #[test]
    fn no_grant_without_profanity() {
        let mut rule = MostProfound::default();
        let grant = rule.finalize().unwrap();
        assert!(grant.is_none());
    }

    #[test]
    fn does_not_grant_during_process() {
        let mut rule = MostProfound::default();
        let ctx = ctx_with("Alice", "alice@example.com");
        let grant = rule.process(&ctx, &profanity()).unwrap();
        assert!(
            grant.is_none(),
            "MostProfound should only grant at finalize"
        );
    }

    #[test]
    fn cache_preserves_counts_and_leader() {
        let mut rule = MostProfound::default();
        let alice = ctx_with("Alice", "alice@example.com");

        for _ in 0..3 {
            rule.process(&alice, &profanity()).unwrap();
        }

        let cache = rule.fini_cache();
        assert_eq!(cache.counts.get("alice@example.com"), Some(&3));
        assert_eq!(
            cache.leader,
            Some(("Alice".to_string(), "alice@example.com".to_string()))
        );

        let mut rule2 = MostProfound::default();
        rule2.init_cache(cache);
        assert_eq!(rule2.counts.get("alice@example.com"), Some(&3));
        assert_eq!(rule2.leader.as_ref().unwrap().0, "Alice");
    }

    #[test]
    fn cached_leader_wins_over_new_author_with_fewer() {
        let mut rule = MostProfound::default();
        let alice = ctx_with("Alice", "alice@example.com");

        // Alice swears 5 times in run 1
        for _ in 0..5 {
            rule.process(&alice, &profanity()).unwrap();
        }
        let cache = rule.fini_cache();

        // Run 2: load cache, Bob swears only twice
        let mut rule2 = MostProfound::default();
        rule2.init_cache(cache);

        let bob = ctx_with("Bob", "bob@example.com");
        for _ in 0..2 {
            rule2.process(&bob, &profanity()).unwrap();
        }

        let grant = rule2.finalize().unwrap().unwrap();
        assert_eq!(
            grant.author_email, "alice@example.com",
            "Alice (cached count 5) should beat Bob (count 2)"
        );
        assert_eq!(
            grant.author_name, "Alice",
            "leader name should survive cache round-trip"
        );
    }
}
