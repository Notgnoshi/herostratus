use std::mem::Discriminant;

use crate::achievement::{AchievementKind, Grant, Meta};
use crate::observer::{CommitContext, Observation};
use crate::rules::rule::Rule;
use crate::rules::rule_plugin::RuleFactory;

const META: Meta = Meta {
    id: 14,
    human_id: "added-first-ci",
    name: "Automating your own misery",
    description: "Be the first person to add a CI configuration file",
    kind: AchievementKind::Global { revocable: false },
};

/// Grant an achievement to the first person who adds a CI configuration file to the repository.
///
/// Since the pipeline walks commits newest-first, the last CI config observation seen is the actual
/// first in the repository. This rule accumulates state and grants at finalize.
///
/// Once the first CI adder has been determined and persisted, the cache stores the commit hash so
/// subsequent runs skip processing entirely.
#[derive(Default)]
pub struct AddedFirstCi {
    /// The commit hash from a previous run, if already settled.
    settled_commit: Option<String>,
    earliest: Option<Grant>,
}

inventory::submit!(RuleFactory::default::<AddedFirstCi>());

/// Stores the commit hash of the first CI config commit once determined.
#[derive(Default, Clone, serde::Deserialize, serde::Serialize)]
pub struct AddedFirstCiCache {
    commit: Option<String>,
}

impl Rule for AddedFirstCi {
    type Cache = AddedFirstCiCache;

    fn meta(&self) -> &Meta {
        &META
    }

    fn consumes(&self) -> &'static [Discriminant<Observation>] {
        &[Observation::CI_CONFIG]
    }

    fn process(&mut self, ctx: &CommitContext, obs: &Observation) -> eyre::Result<Option<Grant>> {
        if self.settled_commit.is_some() {
            return Ok(None);
        }
        if matches!(obs, Observation::CiConfig) {
            self.earliest = Some(META.grant(ctx));
        }
        Ok(None)
    }

    fn finalize(&mut self) -> eyre::Result<Vec<Grant>> {
        if self.settled_commit.is_some() {
            return Ok(Vec::new());
        }
        if let Some(ref grant) = self.earliest {
            self.settled_commit = Some(grant.commit.to_string());
        }
        Ok(self.earliest.take().into_iter().collect())
    }

    fn init_cache(&mut self, cache: Self::Cache) {
        self.settled_commit = cache.commit;
    }

    fn fini_cache(&self) -> Self::Cache {
        AddedFirstCiCache {
            commit: self.settled_commit.clone(),
        }
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn grants_last_seen_at_finalize() {
        let mut rule = AddedFirstCi::default();
        // Walk order is newest-first, so Alice is visited first, then Bob.
        // Bob being last means Bob was the actual first CI adder in the repo.
        let alice = CommitContext::test("Alice");
        let bob = CommitContext::test("Bob");

        assert!(
            rule.process(&alice, &Observation::CiConfig)
                .unwrap()
                .is_none()
        );
        rule.process(&bob, &Observation::CiConfig).unwrap();

        let grants = rule.finalize().unwrap();
        assert_eq!(grants.len(), 1);
        assert_eq!(grants[0].user_email, "bob@example.com");

        let cache = rule.fini_cache();
        assert!(cache.commit.is_some());
    }

    #[test]
    fn settled_cache_skips_processing() {
        let mut rule = AddedFirstCi::default();
        rule.init_cache(AddedFirstCiCache {
            commit: Some("abc123".to_string()),
        });

        let ctx = CommitContext::test("Alice");
        rule.process(&ctx, &Observation::CiConfig).unwrap();

        let grants = rule.finalize().unwrap();
        assert!(grants.is_empty(), "settled rule should not grant");
    }

    #[test]
    fn no_ci_config_no_grant() {
        let mut rule = AddedFirstCi::default();
        let grants = rule.finalize().unwrap();
        assert!(grants.is_empty());
    }
}
