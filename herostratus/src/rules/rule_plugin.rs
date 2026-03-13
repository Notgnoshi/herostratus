use std::mem::Discriminant;

use crate::achievement::{Grant, Meta};
use crate::config::RulesConfig;
use crate::observer::{CommitContext, Observation};
use crate::rules::rule::Rule;

/// The external interface for [Rule]s.
///
/// Type-erases the [Rule::Cache] associated type so rules can be stored as `Box<dyn RulePlugin>`.
/// The blanket `impl<R: Rule> RulePlugin for R` converts `Cache` to/from [serde_json::Value] at
/// the boundary.
pub trait RulePlugin {
    /// Determine if this rule cares about caching.
    fn has_cache(&self) -> bool;
    /// Initialize the cache for this rule.
    fn init_cache(&mut self, cache: serde_json::Value) -> eyre::Result<()>;
    /// Finalize the cache for this rule.
    fn fini_cache(&self) -> eyre::Result<serde_json::Value>;

    /// Static metadata about the achievement this rule grants.
    fn meta(&self) -> &Meta;
    /// Which observation variants this rule consumes.
    fn consumes(&self) -> &'static [Discriminant<Observation>];
    /// Called when a new commit begins.
    fn commit_start(&mut self, ctx: &CommitContext) -> eyre::Result<()>;
    /// Process a single observation.
    fn process(&mut self, ctx: &CommitContext, obs: &Observation) -> eyre::Result<Option<Grant>>;
    /// Called when all observations for the current commit have been sent.
    fn commit_complete(&mut self, ctx: &CommitContext) -> eyre::Result<Option<Grant>>;
    /// Called after all commits have been processed.
    fn finalize(&mut self) -> eyre::Result<Option<Grant>>;
}

impl<R: Rule> RulePlugin for R {
    // The Rule::Cache type is type erased using serde_json::Value
    fn has_cache(&self) -> bool {
        std::any::TypeId::of::<R::Cache>() != std::any::TypeId::of::<()>()
    }
    fn init_cache(&mut self, cache: serde_json::Value) -> eyre::Result<()> {
        let concrete = match cache {
            serde_json::Value::Null => R::Cache::default(),
            other => serde_json::from_value(other)?,
        };
        <R>::init_cache(self, concrete);
        Ok(())
    }
    fn fini_cache(&self) -> eyre::Result<serde_json::Value> {
        Ok(serde_json::to_value(<R>::fini_cache(self))?)
    }

    // Everything else is forwarded directly to the underlying Rule implementation
    fn meta(&self) -> &Meta {
        <R>::meta(self)
    }
    fn consumes(&self) -> &'static [Discriminant<Observation>] {
        <R>::consumes(self)
    }
    fn commit_start(&mut self, ctx: &CommitContext) -> eyre::Result<()> {
        <R>::commit_start(self, ctx)
    }
    fn process(&mut self, ctx: &CommitContext, obs: &Observation) -> eyre::Result<Option<Grant>> {
        <R>::process(self, ctx, obs)
    }
    fn commit_complete(&mut self, ctx: &CommitContext) -> eyre::Result<Option<Grant>> {
        <R>::commit_complete(self, ctx)
    }
    fn finalize(&mut self) -> eyre::Result<Option<Grant>> {
        <R>::finalize(self)
    }
}

/// A factory to build [Rule]s.
///
/// Each rule registers a [RuleFactory] via [inventory::submit!]. Rules that need configuration
/// provide a custom factory; simple rules use [RuleFactory::default].
pub struct RuleFactory {
    factory: fn(&RulesConfig) -> Box<dyn RulePlugin>,
}

impl RuleFactory {
    /// Provide your own factory to build your rule.
    pub const fn new(factory: fn(&RulesConfig) -> Box<dyn RulePlugin>) -> Self {
        Self { factory }
    }

    /// Create a [RuleFactory] that uses [Default] to build a rule.
    pub const fn default<R: RulePlugin + Default + 'static>() -> Self {
        Self {
            factory: |_| Box::new(R::default()),
        }
    }

    /// Use the factory to build the rule.
    pub fn build(&self, config: &RulesConfig) -> Box<dyn RulePlugin> {
        (self.factory)(config)
    }
}

inventory::collect!(RuleFactory);

/// Get a new instance of each registered rule, applying exclude/include filtering.
pub fn builtin_rules(config: &RulesConfig) -> Vec<Box<dyn RulePlugin>> {
    let excludes = config.exclude.as_deref().unwrap_or_default();
    let includes = config.include.as_deref().unwrap_or_default();

    let rules: Vec<_> = inventory::iter::<RuleFactory>
        .into_iter()
        .map(|f| f.build(config))
        .collect();

    rules
        .into_iter()
        .filter(|rule| {
            let meta = rule.meta();
            let mut disabled = false;
            for exclude in excludes {
                if exclude == "all" || meta.id_matches(exclude) {
                    disabled = true;
                }
            }
            for include in includes {
                if meta.id_matches(include) {
                    disabled = false;
                }
            }
            !disabled
        })
        .collect()
}

/// Get a new instance of each registered rule with default configuration.
pub fn builtin_rules_all() -> Vec<Box<dyn RulePlugin>> {
    builtin_rules(&RulesConfig::default())
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn no_rules_have_duplicate_metadata() {
        let rules = builtin_rules_all();
        for rule1 in &rules {
            for rule2 in &rules {
                if std::ptr::eq(rule1, rule2) {
                    continue;
                }

                assert_ne!(rule1.meta().id, rule2.meta().id);
                assert_ne!(rule1.meta().human_id, rule2.meta().human_id);
                assert_ne!(rule1.meta().name, rule2.meta().name);
                assert_ne!(rule1.meta().description, rule2.meta().description);
            }
        }
    }

    #[test]
    fn all_rules_have_expected_metadata() {
        let rules = builtin_rules_all();
        for rule in rules {
            // All rules have a non-trivial name and description
            assert!(rule.meta().name.len() > 4);
            assert!(rule.meta().description.len() > 4);

            // Names start with capitals, if they start with an alphabetic character
            let first = rule.meta().name.chars().next().unwrap();
            assert!(first.is_numeric() || (first.is_alphabetic() && first.is_uppercase()));

            // Names can be a single word, but descriptions can not
            let words = rule.meta().description.split_whitespace();
            assert!(words.count() > 2);

            // Human IDs are lower-alphabetic, separated by hyphens
            assert!(
                rule.meta()
                    .human_id
                    .chars()
                    .all(|c| c == '-' || (c.is_alphabetic() && c.is_lowercase()))
            );
        }
    }

    #[test]
    fn rule_descriptors_increment_from_one_with_no_misses() {
        let rules = builtin_rules_all();
        let mut ids: Vec<_> = rules.iter().map(|r| r.meta().id).collect();
        // Meta achievements are not RulePlugins, but their IDs must not collide
        for meta in crate::achievement::meta_achievement_metas() {
            ids.push(meta.id);
        }
        ids.sort();

        assert!(!ids.is_empty());

        // NOTE: If a rule is ever removed, this will need to be adjusted (Probably don't want to
        // re-use an old ID). But until then, assert that rules start at 1 and increment without
        // skipping IDs.
        let expected: Vec<_> = (1..=ids.len()).collect();
        assert_eq!(ids, expected);
    }

    #[test]
    fn exclude_rules() {
        // Rules 1 through 4 are included by default
        let all_rules = builtin_rules_all();
        let all_ids: Vec<_> = all_rules.iter().map(|r| r.meta().id).collect();
        assert!(all_ids.contains(&1));
        assert!(all_ids.contains(&2));
        assert!(all_ids.contains(&3));
        assert!(all_ids.contains(&4));

        let config = RulesConfig {
            exclude: Some(vec![
                "H2".to_string(),                   // H2, short pretty id
                "longest-subject-line".to_string(), // H3, human id
                "H4-non-unicode".to_string(),       // H4, pretty id
            ]),
            ..Default::default()
        };
        let rules = builtin_rules(&config);
        let ids: Vec<_> = rules.iter().map(|r| r.meta().id).collect();

        // let at least one rule through, so we can test that we're not excluding everything
        assert!(ids.contains(&1));
        assert!(!ids.contains(&2));
        assert!(!ids.contains(&3));
        assert!(!ids.contains(&4));
    }

    #[test]
    fn exclude_all_rules() {
        let config = RulesConfig {
            exclude: Some(vec!["all".into()]),
            ..Default::default()
        };

        let rules = builtin_rules(&config);
        assert!(rules.is_empty());
    }

    #[test]
    fn exclude_all_rules_except() {
        // exclude everything except H1-fixup
        let config = RulesConfig {
            exclude: Some(vec!["all".into()]),
            include: Some(vec!["H1".into()]),
            ..Default::default()
        };

        let rules = builtin_rules(&config);
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].meta().human_id, "fixup");
    }
}
