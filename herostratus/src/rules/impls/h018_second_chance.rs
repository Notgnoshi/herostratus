use std::mem::Discriminant;

use crate::achievement::{AchievementKind, Grant, Meta};
use crate::observer::{CommitContext, Observation};
use crate::rules::rule::Rule;
use crate::rules::rule_plugin::RuleFactory;

const META: Meta = Meta {
    id: 18,
    human_id: "second-chance",
    name: "Second Chance",
    description: "Add an additional root commit to a repository",
    kind: AchievementKind::PerUser { recurrent: true },
};

/// The dynamic name for a root commit at the given chronological index
fn name_for_ordinal(index: usize) -> &'static str {
    match index {
        2 => "Second Chance",
        3 => "Third Time's the Charm",
        4 => "Convergent Timelines",
        _ => "Multiverse Collapse",
    }
}

#[derive(Default, Clone, serde::Deserialize, serde::Serialize)]
pub struct SecondChanceCache {
    /// Total number of root commits Herostratus has ever observed for this repo, across runs.
    pub count: usize,
}

#[derive(Default)]
pub struct SecondChance {
    cache: SecondChanceCache,
    /// Root commits observed during the current run, kept until [Rule::finalize]. This allows us
    /// to sort them by timestamp upon finlization to ensure we grant the right milestones to the
    /// right commits upon first run, and upon subsequent runs with the checkpoints.
    buffered_roots: Vec<CommitContext>,
}

inventory::submit!(RuleFactory::default::<SecondChance>());

impl Rule for SecondChance {
    type Cache = SecondChanceCache;

    fn meta(&self) -> &Meta {
        &META
    }

    fn consumes(&self) -> &'static [Discriminant<Observation>] {
        &[Observation::PARENT_COUNT]
    }

    fn process(&mut self, ctx: &CommitContext, obs: &Observation) -> eyre::Result<Option<Grant>> {
        if matches!(obs, Observation::ParentCount { count: 0 }) {
            self.buffered_roots.push(ctx.clone());
        }
        Ok(None)
    }

    fn finalize(&mut self) -> eyre::Result<Vec<Grant>> {
        let mut buffer = std::mem::take(&mut self.buffered_roots);
        buffer.sort_by_key(|c| c.commit_timestamp);

        let mut grants = Vec::new();
        for ctx in buffer {
            self.cache.count += 1;
            let index = self.cache.count;
            if index == 1 {
                // The absolute-first root ever observed is "the original"; no grant.
                continue;
            }
            let name = name_for_ordinal(index);
            grants.push(META.grant(&ctx).with_name(name.to_string()));
        }
        Ok(grants)
    }

    fn init_cache(&mut self, cache: Self::Cache) {
        self.cache = cache;
    }

    fn fini_cache(&self) -> Self::Cache {
        self.cache.clone()
    }
}

#[cfg(test)]
mod tests {
    use chrono::{TimeZone, Utc};

    use super::*;

    fn ctx_at(name: &str, seconds: i64) -> CommitContext {
        let mut ctx = CommitContext::test(name);
        ctx.commit_timestamp = Utc.timestamp_opt(seconds, 0).unwrap();
        ctx
    }

    fn run_with_roots(roots: &[CommitContext]) -> (Vec<Grant>, SecondChanceCache) {
        let mut rule = SecondChance::default();
        for ctx in roots {
            rule.commit_start(ctx).unwrap();
            rule.process(ctx, &Observation::ParentCount { count: 0 })
                .unwrap();
            rule.commit_complete(ctx).unwrap();
        }
        let grants = rule.finalize().unwrap();
        let cache = rule.fini_cache();
        (grants, cache)
    }

    #[test]
    fn first_ever_root_does_not_grant() {
        let (grants, cache) = run_with_roots(&[ctx_at("Alice", 100)]);
        assert!(grants.is_empty());
        assert_eq!(cache.count, 1);
    }

    #[test]
    fn three_roots_in_one_run_grant_in_chronological_order() {
        // Process in walk order (newest first, as the engine would).
        let r3 = ctx_at("Carol", 300);
        let r2 = ctx_at("Bob", 200);
        let r1 = ctx_at("Alice", 100);
        let (grants, cache) = run_with_roots(&[r3, r2, r1]);

        assert_eq!(cache.count, 3);
        assert_eq!(grants.len(), 2);

        // Oldest (Alice, ts=100) is "the original", no grant.
        // Bob (ts=200) is index 2 -> "Second Chance".
        // Carol (ts=300) is index 3 -> "Third Time's the Charm".
        assert_eq!(grants[0].user_name, "Bob");
        assert_eq!(grants[0].name_override.as_deref(), Some("Second Chance"));
        assert_eq!(grants[1].user_name, "Carol");
        assert_eq!(
            grants[1].name_override.as_deref(),
            Some("Third Time's the Charm")
        );
    }

    #[test]
    fn cache_loaded_from_prior_run_appends_ordinals() {
        let mut rule = SecondChance::default();
        rule.init_cache(SecondChanceCache { count: 3 });

        let r = ctx_at("Dave", 50); // older than any prior root, but added later
        rule.commit_start(&r).unwrap();
        rule.process(&r, &Observation::ParentCount { count: 0 })
            .unwrap();
        rule.commit_complete(&r).unwrap();
        let grants = rule.finalize().unwrap();

        assert_eq!(grants.len(), 1);
        assert_eq!(grants[0].user_name, "Dave");
        assert_eq!(
            grants[0].name_override.as_deref(),
            Some("Convergent Timelines"),
            "index 4 -> Convergent Timelines, regardless of timestamp"
        );
        assert_eq!(rule.fini_cache().count, 4);
    }

    #[test]
    fn ignores_non_root_commits() {
        let mut rule = SecondChance::default();
        let ctx = ctx_at("Alice", 100);
        rule.commit_start(&ctx).unwrap();
        rule.process(&ctx, &Observation::ParentCount { count: 1 })
            .unwrap();
        rule.commit_complete(&ctx).unwrap();
        let grants = rule.finalize().unwrap();
        assert!(grants.is_empty());
        assert_eq!(rule.fini_cache().count, 0);
    }
}
