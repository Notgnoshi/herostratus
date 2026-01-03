use crate::achievement::{Achievement, AchievementDescriptor};
use crate::config::RulesConfig;
use crate::rules::rule::Rule;

type FactoryFunc = fn(&RulesConfig) -> Box<dyn RulePlugin>;

/// A factory to build [Rule]s
///
/// Each [Rule] needs to provide a [RuleFactory] through [inventory::submit!] to register
/// themselves.
pub struct RuleFactory {
    factory: FactoryFunc,
}

// sugar
impl RuleFactory {
    /// Provide your own factory to build your [Rule]
    pub const fn new(factory: FactoryFunc) -> Self {
        Self { factory }
    }

    /// Create a [RuleFactory] that uses [Default] to build your [Rule]
    pub const fn default<R: RulePlugin + Default + 'static>() -> Self {
        RuleFactory {
            factory: |_config_unused_because_default| Box::new(R::default()) as Box<dyn RulePlugin>,
        }
    }

    /// Use the factory to build the [Rule]
    pub fn build(&self, config: &RulesConfig) -> Box<dyn RulePlugin> {
        (self.factory)(config)
    }
}

/// The external interface for [Rule]s
///
/// We split the implementation of rules into two different traits: [RulePlugin] and [Rule].
///
/// We use `inventory::{submit, collect}` to register [RuleFactory] instances that create the
/// [RulePlugin]s. Then the outside world interacts with rules through the [RulePlugin] trait. The
/// inventory trait requires object-safe `Box<dyn RulePlugin>` types, so [RulePlugin] can't have
/// associated types or generic types, but that's what's most convenient for [Rule] implementors,
/// especially when it comes to caching.
///
/// So [RulePlugin] doesn't know about the `Cache` type, and type-erases it using
/// [serde_json::Value] (which isn't JSON-specific, it's just a common type erasure method) while
/// implementors of `Rule<Config=()>` can use the concrete `Config` type for their rule, without
/// needing to worry about the serialization/deserialization, type erasure, or object-safety
/// requirements.
///
/// A [Rule] is a collection of similar business logic that visits commits in a repository to
/// grant zero or more achievements for each commit visited. It unfortunately complicates the API
/// to have a one-to-many API, but it's intended to improve performance by performing a single
/// computation, and then sharing the result for different achievements that care about it.
pub trait RulePlugin {
    /// Get the name of this [Rule] type
    ///
    /// This is not the name of the [Achievement]s granted by this [Rule], but rather of the [Rule]
    /// itself. This is used for logging, and for caching data specific to particular [Rule]s.
    fn name(&self) -> &'static str;
    /// Disable granting the [AchievementDescriptor] with the given ID.
    ///
    /// This allows individual [AchievementDescriptor]s to be enabled/disabled for any given Rule.
    fn disable_by_id(&mut self, id: usize);
    /// Enable granting the [AchievementDescriptor] with the given ID.
    ///
    /// This allows individual [AchievementDescriptor]s to be enabled/disabled for any given Rule.
    fn enable_by_id(&mut self, id: usize);

    /// Determine if this [RulePlugin] cares about caching
    fn has_cache(&self) -> bool;
    /// Initialize the cache for this rule
    fn init_cache(&mut self, cache: serde_json::Value) -> eyre::Result<()>;
    /// Finalize the cache for this rule
    fn fini_cache(&self) -> eyre::Result<serde_json::Value>;

    // The following methods are just forwarded to the Rule trait
    //
    // We don't do a RulePlugin: Rule super trait to inherit these methods; we use a blanket impl
    // instead so that Rule can use generics that the inventory system can't handle.
    fn get_descriptors(&self) -> &[AchievementDescriptor];
    fn get_descriptors_mut(&mut self) -> &mut [AchievementDescriptor];
    fn process(&mut self, commit: &gix::Commit, repo: &gix::Repository) -> Vec<Achievement>;
    fn finalize(&mut self, repo: &gix::Repository) -> Vec<Achievement>;
    fn is_interested_in_diffs(&self) -> bool;
    fn on_diff_start(&mut self, commit: &gix::Commit, repo: &gix::Repository);
    fn on_diff_change(
        &mut self,
        commit: &gix::Commit,
        repo: &gix::Repository,
        change: &gix::object::tree::diff::Change,
    ) -> eyre::Result<gix::object::tree::diff::Action>;
    fn on_diff_end(&mut self, commit: &gix::Commit, repo: &gix::Repository) -> Vec<Achievement>;
}

impl<R> RulePlugin for R
where
    R: Rule,
{
    fn name(&self) -> &'static str {
        let full_name = std::any::type_name::<Self>();
        match full_name.rsplit_once("::") {
            None => full_name,
            Some((_module_path, name)) => name,
        }
    }

    fn disable_by_id(&mut self, id: usize) {
        for d in self.get_descriptors_mut() {
            if d.id == id {
                tracing::debug!("Disabling achievement {:?}", d.pretty_id());
                d.enabled = false;
            }
        }
    }

    fn enable_by_id(&mut self, id: usize) {
        for d in self.get_descriptors_mut() {
            if d.id == id {
                tracing::debug!("Enabling achievement {:?}", d.pretty_id());
                d.enabled = true;
            }
        }
    }

    fn has_cache(&self) -> bool {
        std::any::TypeId::of::<R::Cache>() != std::any::TypeId::of::<()>() // () is the default "no cache" type
    }

    fn init_cache(&mut self, cache: serde_json::Value) -> eyre::Result<()> {
        let concrete_cache = if let serde_json::Value::Null = cache {
            // There was nothing cached previously, so we initialize a new cache object
            R::Cache::default()
        } else {
            serde_json::from_value(cache)?
        };
        <R>::init_cache(self, concrete_cache);
        Ok(())
    }

    fn fini_cache(&self) -> eyre::Result<serde_json::Value> {
        let concrete_cache = <R>::fini_cache(self);
        let erased_cache = serde_json::to_value(concrete_cache)?;
        Ok(erased_cache)
    }

    // Everything else is just forwarded to the Rule impl
    fn get_descriptors(&self) -> &[AchievementDescriptor] {
        <R>::get_descriptors(self)
    }
    fn get_descriptors_mut(&mut self) -> &mut [AchievementDescriptor] {
        <R>::get_descriptors_mut(self)
    }
    fn process(&mut self, commit: &gix::Commit, repo: &gix::Repository) -> Vec<Achievement> {
        <R>::process(self, commit, repo)
    }
    fn finalize(&mut self, repo: &gix::Repository) -> Vec<Achievement> {
        <R>::finalize(self, repo)
    }
    fn is_interested_in_diffs(&self) -> bool {
        <R>::is_interested_in_diffs(self)
    }
    fn on_diff_start(&mut self, commit: &gix::Commit, repo: &gix::Repository) {
        <R>::on_diff_start(self, commit, repo)
    }
    fn on_diff_change(
        &mut self,
        commit: &gix::Commit,
        repo: &gix::Repository,
        change: &gix::object::tree::diff::Change,
    ) -> eyre::Result<gix::object::tree::diff::Action> {
        <R>::on_diff_change(self, commit, repo, change)
    }
    fn on_diff_end(&mut self, commit: &gix::Commit, repo: &gix::Repository) -> Vec<Achievement> {
        <R>::on_diff_end(self, commit, repo)
    }
}
