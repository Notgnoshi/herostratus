use std::mem::Discriminant;

use crate::achievement::{AchievementKind, Grant, Meta};
use crate::config::RulesConfig;
use crate::observer::{CommitContext, Observation};
use crate::rules::rule::Rule;
use crate::rules::rule_plugin::RuleFactory;

#[derive(Debug, Clone, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct H003Config {
    pub length_threshold: usize,
}

impl Default for H003Config {
    fn default() -> Self {
        Self {
            length_threshold: 72,
        }
    }
}

const META: Meta = Meta {
    id: 3,
    human_id: "longest-subject-line",
    name: "50 characters was more of a suggestion anyways",
    description: "The longest subject line",
    kind: AchievementKind::Global { revocable: true },
};

#[derive(Default, Clone, serde::Deserialize, serde::Serialize)]
pub struct LongestCache {
    longest_length: Option<usize>,
}

/// Grant an achievement for the longest subject line in the repository.
pub struct LongestSubject {
    threshold: usize,
    cache: LongestCache,
    candidate: Option<Grant>,
}

impl Default for LongestSubject {
    fn default() -> Self {
        Self {
            threshold: 72,
            cache: LongestCache::default(),
            candidate: None,
        }
    }
}

fn longest_subject_factory(config: &RulesConfig) -> Box<dyn crate::rules::rule_plugin::RulePlugin> {
    Box::new(LongestSubject {
        threshold: config
            .h3_longest_subject_line
            .as_ref()
            .map_or(72, |c| c.length_threshold),
        ..Default::default()
    })
}
inventory::submit!(RuleFactory::new(longest_subject_factory));

impl Rule for LongestSubject {
    type Cache = LongestCache;

    fn meta(&self) -> &Meta {
        &META
    }

    fn consumes(&self) -> &'static [Discriminant<Observation>] {
        &[Observation::SUBJECT_LENGTH]
    }

    fn process(&mut self, ctx: &CommitContext, obs: &Observation) -> eyre::Result<Option<Grant>> {
        let Observation::SubjectLength { length } = obs else {
            return Ok(None);
        };

        let dominated_by_threshold = *length <= self.threshold;
        let dominated_by_cache = self.cache.longest_length.is_some_and(|l| *length <= l);
        if dominated_by_threshold || dominated_by_cache {
            return Ok(None);
        }

        self.cache.longest_length = Some(*length);
        self.candidate = Some(META.grant(ctx));
        Ok(None)
    }

    fn finalize(&mut self) -> eyre::Result<Option<Grant>> {
        Ok(self.candidate.take())
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

    use super::*;

    #[test]
    fn grants_longest() {
        let mut rule = LongestSubject {
            threshold: 5,
            ..Default::default()
        };
        rule.process(
            &CommitContext::test("Alice"),
            &Observation::SubjectLength { length: 10 },
        )
        .unwrap();
        rule.process(
            &CommitContext::test("Bob"),
            &Observation::SubjectLength { length: 8 },
        )
        .unwrap();
        let grant = rule.finalize().unwrap();
        assert!(grant.is_some());
        assert_eq!(grant.unwrap().user_name, "Alice");
    }

    #[test]
    fn threshold_filters() {
        let mut rule = LongestSubject {
            threshold: 100,
            ..Default::default()
        };
        rule.process(
            &CommitContext::test("Alice"),
            &Observation::SubjectLength { length: 80 },
        )
        .unwrap();
        let grant = rule.finalize().unwrap();
        assert!(grant.is_none());
    }

    #[test]
    fn cache_preserves_across_runs() {
        let mut rule = LongestSubject {
            threshold: 5,
            ..Default::default()
        };
        rule.process(
            &CommitContext::test("Alice"),
            &Observation::SubjectLength { length: 100 },
        )
        .unwrap();
        let cache = rule.fini_cache();
        assert_eq!(cache.longest_length, Some(100));

        let mut rule2 = LongestSubject {
            threshold: 5,
            ..Default::default()
        };
        rule2.init_cache(cache);
        // Length 80 is longer than threshold (5) but not longer than cached (100)
        rule2
            .process(
                &CommitContext::test("Bob"),
                &Observation::SubjectLength { length: 80 },
            )
            .unwrap();
        let grant = rule2.finalize().unwrap();
        assert!(grant.is_none());
    }
}
