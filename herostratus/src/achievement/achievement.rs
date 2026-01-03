use crate::config::RulesConfig;

#[derive(Debug)]
pub struct Achievement {
    pub name: &'static str,
    pub commit: gix::ObjectId,
    // TODO: Add the user (how to accommodate mailmaps?)
    // TODO: Identify the repository somehow
}

type FactoryFunc = fn(&RulesConfig) -> Box<dyn RulePlugin>;

/// A factory to build [Rule]s
///
/// Each [Rule] needs to provide a [RuleFactory] through [inventory::submit!] to register
/// themselves.
pub struct RuleFactory {
    factory: FactoryFunc,
}
// See also: rules/mod.rs:builtin_rules(), and each of the inventory::submit!(...) in each Rule impl
inventory::collect!(RuleFactory);

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

/// Describes an [Achivement] that a [Rule] can grant
#[derive(Clone, Debug)]
pub struct AchievementDescriptor {
    /// Whether the [Rule] this descriptor belongs to will grant achievements described by this
    /// descriptor
    pub enabled: bool,

    /// The numeric ID of this [Rule]
    ///
    /// Must be unique per-rule. Either the [id](Self::id), [human_id](Self::human_id), or
    /// [pretty_id](Self::pretty_id) may be used to identify a [Rule].
    pub id: usize,

    /// The human ID of this [Rule]
    ///
    /// Example: `longest-commit-subject-line`
    ///
    /// Must be unique per-rule. Either the [id](Self::id), [human_id](Self::human_id), or
    /// [pretty_id](Self::pretty_id) may be used to identify a [Rule].
    pub human_id: &'static str,

    /// The name of the [Achievement] that this rule generates
    ///
    /// The name should generally be humorous, even if the [description](Self::description) isn't.
    ///
    /// There is expected to be a 1-1 correspondence between [Achievement]s and [Rule]s.
    pub name: &'static str,

    /// A short flavor text describing what this [Rule] is all about
    ///
    /// Imagine the short one-sentence descriptions of Steam achievements.
    ///
    /// Examples:
    /// * Use a swear word
    /// * Use the most swear words
    /// * The shortest subject line
    pub description: &'static str,
}

impl AchievementDescriptor {
    /// Determine if the given ID matches this [AchievementDescriptor]
    pub fn id_matches(&self, id: &str) -> bool {
        id == self.id.to_string()
            || id == format!("H{}", self.id)
            || id == self.human_id
            || id == self.pretty_id()
    }

    /// The pretty ID of the [Achievement]s that this [AchievementDescriptor] describes.
    ///
    /// Concatenates the numeric [id](Self::id) and the human-meaningful [human_id](Self::id).
    ///
    /// Example: `H42-whats-the-question`
    ///
    /// Must be unique per-rule. Either the [id](Self::id), [human_id](Self::human_id), or
    /// [pretty_id](Self::pretty_id) may be used to identify a [Rule].
    pub fn pretty_id(&self) -> String {
        format!("H{}-{}", self.id, self.human_id)
    }
}

/// The external interface for [Rule]s
///
/// We split the implementation of rules into two different traits: RulePlugin and Rule.
///
/// We use inventory::{submit, collect} to register RuleFactory instances that create the
/// RulePlugins. Then the outside world interacts with rules through the RulePlugin trait. The
/// inventory trait requires object-safe Box<dyn RulePlugin> types, so RulePlugin can't have
/// associated types or generic types, but that's what's most convenient for Rule implementors,
/// especially when it comes to caching.
///
/// So RulePlugin doesn't know about the Cache type, and type-erases it using serde_json::Value
/// (which isn't JSON-specific, it's just a common type erasure method) while implementors of
/// Rule<Config=()> can use the concrete Config type for their rule, without needing to worry about
/// the serialization/deserialization, type erasure, or object-safety requirements.
///
/// A [Rule] is a collection of similar business logic that visits commits in a repository to
/// grant zero or more achievements for each commit visited. It unfortunately complicates the API
/// to have a one-to-many API, but it's intended to improve performance by performing a single
/// computation, and then sharing the result for different achievements that care about it.
pub trait RulePlugin {
    fn name(&self) -> &'static str;
    fn disable_by_id(&mut self, id: usize);
    fn enable_by_id(&mut self, id: usize);

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
    /// Get the name of this [Rule] type
    ///
    /// This is not the name of the [Achievement]s granted by this [Rule], but rather of the [Rule]
    /// itself. This is used for logging, and for caching data specific to particular [Rule]s.
    ///
    /// You probably don't want to override this.
    fn name(&self) -> &'static str {
        let full_name = std::any::type_name::<Self>();
        match full_name.rsplit_once("::") {
            None => full_name,
            Some((_module_path, name)) => name,
        }
    }

    /// Disable granting the [AchievementDescriptor] with the given ID.
    ///
    /// This allows individual [AchievementDescriptor]s to be enabled/disabled for any given Rule.
    fn disable_by_id(&mut self, id: usize) {
        for d in self.get_descriptors_mut() {
            if d.id == id {
                tracing::debug!("Disabling achievement {:?}", d.pretty_id());
                d.enabled = false;
            }
        }
    }

    /// Enable granting the [AchievementDescriptor] with the given ID.
    ///
    /// This allows individual [AchievementDescriptor]s to be enabled/disabled for any given Rule.
    fn enable_by_id(&mut self, id: usize) {
        for d in self.get_descriptors_mut() {
            if d.id == id {
                tracing::debug!("Enabling achievement {:?}", d.pretty_id());
                d.enabled = true;
            }
        }
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

/// Defines a [Rule] to grant [Achievement]s
pub trait Rule {
    /// Get the list of [AchievementDescriptor]s that this [Rule] can grant
    ///
    /// This allows one [Rule] to grant multiple different types of [Achievement]s, which is useful
    /// for achievement types that can share computation (e.g., shortest commit, longest commit,
    /// etc).
    fn get_descriptors(&self) -> &[AchievementDescriptor];
    fn get_descriptors_mut(&mut self) -> &mut [AchievementDescriptor];

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
