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

const THRESHOLDS: &[usize] = &[5, 10, 25, 100];

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
        let Observation::Profanity { words } = obs else {
            return Ok(None);
        };

        let count = self.counts.entry(ctx.author_email.clone()).or_insert(0);
        let before = *count;
        *count += words.len();
        let after = *count;

        // A single commit can cross more than one threshold when it contains many profanities
        // (e.g., 4 -> 9 crosses 5). Grant at the highest threshold crossed so we emit at most
        // one grant per commit, and so the dynamic name reflects the milestone actually reached.
        let crossed = THRESHOLDS
            .iter()
            .rev()
            .find(|&&t| before < t && after >= t)
            .copied();
        Ok(crossed.map(|t| META.grant(ctx).with_name(format!("{} ({})", META.name, t))))
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

    fn profanity() -> Observation {
        Observation::Profanity {
            words: vec!["damn".to_string()],
        }
    }

    #[test]
    fn no_grant_below_threshold() {
        let mut rule = LikeASailor::default();
        let ctx = CommitContext::test("Alice");
        for _ in 0..4 {
            let grant = rule.process(&ctx, &profanity()).unwrap();
            assert!(grant.is_none());
        }
    }

    #[test]
    fn grants_at_threshold() {
        let mut rule = LikeASailor::default();
        let ctx = CommitContext::test("Alice");
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
        let alice = CommitContext::test("Alice");
        let bob = CommitContext::test("Bob");

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
    fn grant_has_dynamic_name_with_count() {
        let mut rule = LikeASailor::default();
        let ctx = CommitContext::test("Alice");
        let mut grant = None;
        for i in 1..=6 {
            let g = rule.process(&ctx, &profanity()).unwrap();

            if i == 5 {
                assert!(g.is_some(), "granted on profanity #5");
            } else {
                assert!(g.is_none(), "not granted on profanity #{i}");
            }
            if g.is_some() {
                grant = g;
            }
        }
        let grant = grant.expect("expected grant at threshold 5");
        assert_eq!(
            grant.name_override.as_deref(),
            Some("Swears Like a Sailor (5)")
        );
    }

    #[test]
    fn cache_preserves_counts() {
        let mut rule = LikeASailor::default();
        let ctx = CommitContext::test("Alice");

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
    fn counts_multiple_profanities_in_one_commit() {
        let mut rule = LikeASailor::default();
        let ctx = CommitContext::test("Alice");

        // One commit carrying 5 profane words should cross threshold 5.
        let obs = Observation::Profanity {
            words: vec![
                "shit".into(),
                "fuck".into(),
                "damn".into(),
                "hell".into(),
                "ass".into(),
            ],
        };
        let grant = rule.process(&ctx, &obs).unwrap();
        assert!(
            grant.is_some(),
            "expected grant at threshold 5 from a single commit"
        );
        assert_eq!(
            grant.unwrap().name_override.as_deref(),
            Some("Swears Like a Sailor (5)")
        );
    }

    #[test]
    fn skipping_threshold_grants_highest_crossed() {
        let mut rule = LikeASailor::default();
        let ctx = CommitContext::test("Alice");

        // Bring Alice to 4 with single-word commits.
        for _ in 0..4 {
            let obs = Observation::Profanity {
                words: vec!["damn".into()],
            };
            rule.process(&ctx, &obs).unwrap();
        }

        // A single commit with 7 words brings the count from 4 to 11, crossing both 5 and 10.
        let obs = Observation::Profanity {
            words: (0..7).map(|_| "damn".to_string()).collect(),
        };
        let grant = rule.process(&ctx, &obs).unwrap().unwrap();
        assert_eq!(
            grant.name_override.as_deref(),
            Some("Swears Like a Sailor (10)"),
            "should grant the highest threshold crossed, not the lowest"
        );
    }
}
