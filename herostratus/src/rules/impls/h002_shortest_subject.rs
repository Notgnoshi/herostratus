use std::mem::Discriminant;

use crate::achievement::{AchievementKind, Grant, Meta};
use crate::config::RulesConfig;
use crate::observer::{CommitContext, Observation};
use crate::rules::rule::Rule;
use crate::rules::rule_plugin::RuleFactory;

#[derive(Debug, Clone, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct H002Config {
    pub length_threshold: usize,
}

impl Default for H002Config {
    fn default() -> Self {
        Self {
            length_threshold: 10,
        }
    }
}

const META: Meta = Meta {
    id: 2,
    human_id: "shortest-subject-line",
    name: "Brevity is the soul of wit",
    description: "The shortest subject line",
    kind: AchievementKind::Global { revocable: true },
};

#[derive(Default, Clone, serde::Deserialize, serde::Serialize)]
pub struct ShortestCache {
    shortest_length: Option<usize>,
}

/// Grant an achievement for the shortest subject line in the repository.
pub struct ShortestSubject {
    threshold: usize,
    cache: ShortestCache,
    candidate: Option<Grant>,
}

impl Default for ShortestSubject {
    fn default() -> Self {
        Self {
            threshold: 10,
            cache: ShortestCache::default(),
            candidate: None,
        }
    }
}

fn shortest_subject_factory(
    config: &RulesConfig,
) -> Box<dyn crate::rules::rule_plugin::RulePlugin> {
    Box::new(ShortestSubject {
        threshold: config
            .h2_shortest_subject_line
            .as_ref()
            .map_or(10, |c| c.length_threshold),
        ..Default::default()
    })
}
inventory::submit!(RuleFactory::new(shortest_subject_factory));

impl Rule for ShortestSubject {
    type Cache = ShortestCache;

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

        let dominated_by_threshold = *length >= self.threshold;
        let dominated_by_cache = self.cache.shortest_length.is_some_and(|s| *length >= s);
        if dominated_by_threshold || dominated_by_cache {
            return Ok(None);
        }

        self.cache.shortest_length = Some(*length);
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

    fn ctx(name: &str) -> CommitContext {
        CommitContext {
            oid: gix::ObjectId::null(gix::hash::Kind::Sha1),
            author_name: name.to_string(),
            author_email: format!("{name}@example.com"),
        }
    }

    #[test]
    fn grants_shortest() {
        let mut rule = ShortestSubject {
            threshold: 10,
            ..Default::default()
        };
        rule.process(&ctx("Alice"), &Observation::SubjectLength { length: 5 })
            .unwrap();
        rule.process(&ctx("Bob"), &Observation::SubjectLength { length: 8 })
            .unwrap();
        let grant = rule.finalize().unwrap();
        assert!(grant.is_some());
        assert_eq!(grant.unwrap().author_name, "Alice");
    }

    #[test]
    fn threshold_filters() {
        let mut rule = ShortestSubject {
            threshold: 5,
            ..Default::default()
        };
        rule.process(&ctx("Alice"), &Observation::SubjectLength { length: 8 })
            .unwrap();
        let grant = rule.finalize().unwrap();
        assert!(grant.is_none());
    }

    #[test]
    fn cache_preserves_across_runs() {
        let mut rule = ShortestSubject {
            threshold: 10,
            ..Default::default()
        };
        rule.process(&ctx("Alice"), &Observation::SubjectLength { length: 3 })
            .unwrap();
        let cache = rule.fini_cache();
        assert_eq!(cache.shortest_length, Some(3));

        let mut rule2 = ShortestSubject {
            threshold: 10,
            ..Default::default()
        };
        rule2.init_cache(cache);
        // Length 5 is shorter than threshold (10) but not shorter than cached (3)
        rule2
            .process(&ctx("Bob"), &Observation::SubjectLength { length: 5 })
            .unwrap();
        let grant = rule2.finalize().unwrap();
        assert!(grant.is_none());
    }
}
