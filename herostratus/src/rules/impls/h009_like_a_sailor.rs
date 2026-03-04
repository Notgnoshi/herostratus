use std::collections::HashMap;
use std::mem::Discriminant;

use crate::achievement::{AchievementKind, Grant, Meta};
use crate::observer::{CommitContext, Observation};
use crate::rules::rule::Rule;
use crate::rules::rule_plugin::RuleFactory;

const META: Meta = Meta {
    id: 9,
    human_id: "like-a-sailor",
    name: "Swears Like a Sailor",
    description: "Use profanity in many commit messages",
    kind: AchievementKind::PerUser { recurrent: true },
};

// TODO: It would be best to have the thresholds defined in the Meta, and include them in the
// generated AchievementEvents
const THRESHOLDS: &[usize] = &[5, 25, 100];

/// Grant an achievement when a user hits profanity count milestones.
///
/// Tracks per-user profanity counts in its cache. Grants at each threshold milestone (5, 25, 100).
/// The AchievementLog allows recurrent per-user grants.
#[derive(Default)]
pub struct LikeASailor {
    counts: HashMap<String, usize>,
}

inventory::submit!(RuleFactory::default::<LikeASailor>());

impl Rule for LikeASailor {
    type Cache = HashMap<String, usize>;

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

        if THRESHOLDS.contains(count) {
            Ok(Some(META.grant(ctx)))
        } else {
            Ok(None)
        }
    }

    fn init_cache(&mut self, cache: Self::Cache) {
        self.counts = cache;
    }

    fn fini_cache(&self) -> Self::Cache {
        self.counts.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx_with_email(email: &str) -> CommitContext {
        CommitContext {
            oid: gix::ObjectId::null(gix::hash::Kind::Sha1),
            author_name: "Test".to_string(),
            author_email: email.to_string(),
        }
    }

    fn profanity() -> Observation {
        Observation::Profanity {
            word: "damn".to_string(),
        }
    }

    #[test]
    fn no_grant_below_threshold() {
        let mut rule = LikeASailor::default();
        let ctx = ctx_with_email("alice@example.com");
        for _ in 0..4 {
            let grant = rule.process(&ctx, &profanity()).unwrap();
            assert!(grant.is_none());
        }
    }

    #[test]
    fn grants_at_threshold() {
        let mut rule = LikeASailor::default();
        let ctx = ctx_with_email("alice@example.com");
        let mut granted = false;
        for _ in 0..5 {
            if let Some(_grant) = rule.process(&ctx, &profanity()).unwrap() {
                granted = true;
            }
        }
        assert!(granted, "expected grant at threshold 5");
    }

    #[test]
    fn counts_are_per_user() {
        let mut rule = LikeASailor::default();
        let alice = ctx_with_email("alice@example.com");
        let bob = ctx_with_email("bob@example.com");

        // Give alice 4 profanities
        for _ in 0..4 {
            rule.process(&alice, &profanity()).unwrap();
        }
        // Give bob 4 profanities
        for _ in 0..4 {
            rule.process(&bob, &profanity()).unwrap();
        }

        // Alice's 5th should trigger
        let grant = rule.process(&alice, &profanity()).unwrap();
        assert!(grant.is_some());

        // Bob's 5th should also trigger
        let grant = rule.process(&bob, &profanity()).unwrap();
        assert!(grant.is_some());
    }

    #[test]
    fn cache_preserves_counts() {
        let mut rule = LikeASailor::default();
        let ctx = ctx_with_email("alice@example.com");

        // Accumulate 3 profanities
        for _ in 0..3 {
            rule.process(&ctx, &profanity()).unwrap();
        }
        let cache = rule.fini_cache();
        assert_eq!(cache.get("alice@example.com"), Some(&3));

        // New rule with loaded cache -- 2 more should hit threshold 5
        let mut rule2 = LikeASailor::default();
        rule2.init_cache(cache);
        rule2.process(&ctx, &profanity()).unwrap(); // 4
        let grant = rule2.process(&ctx, &profanity()).unwrap(); // 5
        assert!(
            grant.is_some(),
            "expected grant at threshold 5 after cache load"
        );
    }

    #[test]
    fn ignores_other_observations() {
        let mut rule = LikeASailor::default();
        let ctx = ctx_with_email("alice@example.com");
        let grant = rule.process(&ctx, &Observation::Fixup).unwrap();
        assert!(grant.is_none());
    }
}
