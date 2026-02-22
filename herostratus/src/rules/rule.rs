use crate::achievement::{Achievement, AchievementDescriptor};

/// Defines a [Rule] to grant [Achievement]s
pub trait Rule {
    /// If a [Rule] needs to cache data between runes, it can define a cache type here.
    ///
    /// An example of a rule that needs to cache data is one that calculates the shortest commit
    /// message encountered so far. The rule processing algorithm uses a cache to avoid
    /// reprocessing commits it's already processed, so such a rule must cache the shortest commit
    /// it has encountered so far so that subsequent runs work correctly.
    ///
    /// If a [Rule] does not need to cache data, define this associated type as `()`.
    /// Unfortunately, using a `Rule<Cache = ()>` generic parameter doesn't work with the
    /// `RulePlugin` blanket implementation, so we unfortunately can't provide a default type of
    /// `()` (since most rules won't need a cache).
    type Cache: Default + serde::Serialize + for<'de> serde::Deserialize<'de> + 'static;

    /// Initialize the [Rule] with the given cache.
    ///
    /// The rule is expected to store the cache, use it during processing, and then return it from
    /// [Rule::fini_cache] once processing is done.
    ///
    /// This method will be called once after the [Rule] is constructed, but before any
    /// [Rule::process] calls are made.
    fn init_cache(&mut self, _cache: Self::Cache) {}

    /// Finalize the cache for this [Rule]
    ///
    /// This method will only be called once after [Rule::finalize] has been called.
    fn fini_cache(&self) -> Self::Cache {
        Self::Cache::default()
    }

    /// Get the list of [AchievementDescriptor]s that this [Rule] can grant
    ///
    /// This allows one [Rule] to grant multiple different types of [Achievement]s, which is useful
    /// for achievement types that can share computation (e.g., shortest commit, longest commit,
    /// etc).
    fn descriptors(&self) -> &[AchievementDescriptor];

    /// Process the given [gix::Commit] to generate an [Achievement]
    ///
    /// Notice that this method takes `&mut self`. This is to allow the `Rule` to accumulate state
    /// during commit processing. At the end of processing, [finalize](Self::finalize) will be
    /// called, to generate any achievements from the accumulated state.
    fn process(&mut self, _commit: &gix::Commit, _repo: &gix::Repository) -> Vec<Achievement> {
        Vec::new()
    }

    /// Called when finished processing all commits
    ///
    /// This exists to enable rules that accumulate state (like calculating the shortest commit
    /// message) to generate achievements once all commits have been visited.
    fn finalize(&mut self, _repo: &gix::Repository) -> Vec<Achievement> {
        Vec::new()
    }

    /// Indicates whether this [Rule] would like to receive commit diffs
    ///
    /// If a rule is interested in diffs, then for each commit processed, the following methods
    /// will be called in order:
    /// 1. [process](Self::process)
    /// 2. [on_diff_start](Self::on_diff_start)
    /// 3. [on_diff_change](Self::on_diff_change) for each change
    /// 4. [on_diff_end](Self::on_diff_end)
    ///
    /// If `on_diff_change` returns `Action::Cancel`, or an `Err`, no further changes will be
    /// passed to the rule for that commit. This acts as an early-out mechanism to save on
    /// computation.
    fn is_interested_in_diffs(&self) -> bool {
        false
    }

    /// Start the diff for the given commit
    fn on_diff_start(&mut self, _commit: &gix::Commit, _repo: &gix::Repository) {}

    /// Process a single change from the diff
    ///
    /// If this method returns `Action::Cancel`, no further changes will be passed to the rule
    fn on_diff_change(
        &mut self,
        _commit: &gix::Commit,
        _repo: &gix::Repository,
        _change: &gix::object::tree::diff::Change,
    ) -> eyre::Result<gix::object::tree::diff::Action> {
        Ok(gix::object::tree::diff::Action::Cancel)
    }

    /// Handle the end of the diff for the given commit
    ///
    /// Will be called regardless of the return value for `on_diff_change`
    fn on_diff_end(&mut self, _commit: &gix::Commit, _repo: &gix::Repository) -> Vec<Achievement> {
        Vec::new()
    }
}
