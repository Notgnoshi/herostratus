use std::mem::Discriminant;

use crate::achievement::{Grant, Meta};
use crate::observer::{CommitContext, Observation};

/// A rule that consumes [Observation]s and grants achievements.
///
/// Rules receive observations (not raw commits) and return [Grant]s. The engine handles variation
/// enforcement (deduplication, revocation) using the achievement log.
///
/// # Lifecycle
///
/// The rule engine calls methods in this order for each commit:
///
/// 1. [commit_start](Self::commit_start)
/// 2. [process](Self::process) -- called once per observation. Rules that don't need to accumulate
///    state across commits emit their grants here. Note that observations are created
///    asynchronously by the observers, so there's no ordering guarantee on the order in which
///    observations for a commit are handled!
/// 3. [commit_complete](Self::commit_complete) -- called once all observations for the current
///    commit have been processed. Rules that accumulate state within a commit may emit their
///    grants here.
///
/// After all commits have been processed, [finalize](Self::finalize) is called once. Rules that
/// accumulate state across commits (e.g., "shortest subject") emit their grants here.
///
/// Any particular rule is expected to only emit grants in *one of* `process`, `commit_complete`,
/// or `finalize`.
///
/// # Implementing a new Rule
///
/// 1. Implement any necessary [Observer](crate::observer::Observer)s to generate the
///    [Observation]s you need
/// 2. Implement the [Rule] trait in the [impls] module
/// 3. Register the rule via [inventory::submit!] using the
///    [RuleFactory](super::rule_plugin::RuleFactory) helper
#[expect(dead_code)]
pub(in crate::rules) trait Rule {
    type Cache: Default + serde::Serialize + for<'de> serde::Deserialize<'de> + 'static;

    /// Static metadata about the achievement this rule grants.
    ///
    /// One rule = one achievement, enforced structurally by the singular return type.
    fn meta(&self) -> &Meta;

    /// Which observation variants this rule consumes.
    ///
    /// Used by the checkpoint system for dependency tracking, not for runtime routing -- every rule
    /// still receives every observation and ignores irrelevant variants.
    fn consumes(&self) -> &'static [Discriminant<Observation>];

    /// Called when a new commit begins. Use to reset per-commit state.
    fn commit_start(&mut self, _ctx: &CommitContext) -> eyre::Result<()> {
        Ok(())
    }

    /// Process a single observation with its commit context. May return a [Grant].
    fn process(&mut self, ctx: &CommitContext, obs: &Observation) -> eyre::Result<Option<Grant>>;

    /// Called when all observations for the current commit have been sent.
    ///
    /// Rules that buffer observations within a commit emit here.
    fn commit_complete(&mut self, _ctx: &CommitContext) -> eyre::Result<Option<Grant>> {
        Ok(None)
    }

    /// Called after all commits have been processed.
    ///
    /// Rules that accumulate state across commits (e.g., "shortest subject") emit here.
    fn finalize(&mut self) -> eyre::Result<Option<Grant>> {
        Ok(None)
    }

    /// Initialize the rule with its persisted cache. Called once before any
    /// [process](Self::process) calls.
    fn init_cache(&mut self, _cache: Self::Cache) {}

    /// Return the cache for persistence. Called once after [finalize](Self::finalize).
    fn fini_cache(&self) -> Self::Cache {
        Self::Cache::default()
    }
}
