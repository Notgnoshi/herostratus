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
