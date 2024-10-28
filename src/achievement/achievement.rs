use crate::config::RulesConfig;

#[derive(Debug)]
pub struct Achievement {
    pub name: &'static str,
    // TODO: Should this be the git2::Commit instead of the Oid? That'd enable easier serialization
    // of the actual commit message and author details, but it'd also introduce an awkward
    // lifetime.
    pub commit: git2::Oid,
    // TODO: Add the user (how to accommodate mailmaps?)
    // TODO: Identify the repository somehow
}

type FactoryFunc = fn(&RulesConfig) -> Box<dyn Rule>;

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
    pub const fn default<R: Rule + Default + 'static>() -> Self {
        RuleFactory {
            factory: |_config_unused_because_default| Box::new(R::default()) as Box<dyn Rule>,
        }
    }

    /// Use the factory to build the [Rule]
    pub fn build(&self, config: &RulesConfig) -> Box<dyn Rule> {
        (self.factory)(config)
    }
}

/// Defines a [Rule] to grant [Achievement]s
// TODO: How could user-contrib rule _scripts_ work? Consume commits via stdin, emit achievement
// JSON on stdout?
pub trait Rule {
    /// The numeric ID of this [Rule]
    ///
    /// Must be unique per-rule. Either the [id](Self::id), [human_id](Self::human_id), or
    /// [pretty_id](Self::pretty_id) may be used to identify a [Rule].
    fn id(&self) -> usize;

    /// The human ID of this [Rule]
    ///
    /// Example: `longest-commit-subject-line`
    ///
    /// Must be unique per-rule. Either the [id](Self::id), [human_id](Self::human_id), or
    /// [pretty_id](Self::pretty_id) may be used to identify a [Rule].
    fn human_id(&self) -> &'static str;

    /// The pretty ID of this [Rule]
    ///
    /// Concatenates the numeric [id](Self::id) and the human-meaningful [human_id](Self::id).
    ///
    /// Example: `H42-whats-the-question`
    ///
    /// Must be unique per-rule. Either the [id](Self::id), [human_id](Self::human_id), or
    /// [pretty_id](Self::pretty_id) may be used to identify a [Rule].
    fn pretty_id(&self) -> String {
        format!("H{}-{}", self.id(), self.human_id())
    }

    /// Return the name of the [Achievement] that this rule generates
    ///
    /// The name should generally be humorous, even if the [description](Self::description) isn't.
    ///
    /// There is expected to be a 1-1 correspondence between [Achievement]s and [Rule]s.
    fn name(&self) -> &'static str;

    /// A short flavor text describing what this [Rule] is all about
    ///
    /// Imagine the short one-sentence descriptions of Steam achievements.
    ///
    /// Examples:
    /// * Use a swear word
    /// * Use the most swear words
    /// * The shortest subject line
    fn description(&self) -> &'static str;

    /// Grant the given [git2::Commit] this rule's [Achievement]
    fn grant(&self, commit: &git2::Commit, _repo: &git2::Repository) -> Achievement {
        Achievement {
            name: self.name(),
            commit: commit.id(),
        }
    }

    /// Process the given [git2::Commit] to generate an [Achievement]
    ///
    /// Notice that this method takes `&mut self`. This is to allow the `Rule` to accumulate state
    /// during commit processing. At the end of processing, [finalize](Self::finalize) will be
    /// called, to generate any achievements from the accumulated state.
    fn process(&mut self, commit: &git2::Commit, repo: &git2::Repository) -> Option<Achievement>;

    /// Called when finished processing all commits
    ///
    /// This exists to enable rules that accumulate state (like calculating the shortest commit
    /// message) to generate achievements once all commits have been visited.
    fn finalize(&mut self, _repo: &git2::Repository) -> Vec<Achievement> {
        Vec::new()
    }
}

/// Wrap achievement granting in logging
pub trait LoggedRule: Rule {
    fn log_achievement(&self, achievement: &Achievement) {
        debug_assert_eq!(
            achievement.name,
            self.name(),
            "Achievement::name and Rule::name are expected to match"
        );
        tracing::info!("Generated achievement: {achievement:?}");
    }

    // TODO: What's the Rust way to override a base class method?
    fn process_log(
        &mut self,
        commit: &git2::Commit,
        repo: &git2::Repository,
    ) -> Option<Achievement> {
        let achievement = self.process(commit, repo)?;
        self.log_achievement(&achievement);
        Some(achievement)
    }

    fn finalize_log(&mut self, repo: &git2::Repository) -> Vec<Achievement> {
        let achievements = self.finalize(repo);
        if !achievements.is_empty() {
            // This isn't the total number of achievements, just the ones granted at the end
            tracing::debug!(
                "Rule '{}' generated {} achievements after finalization",
                self.name(),
                achievements.len()
            );
            for achievement in &achievements {
                self.log_achievement(achievement);
            }
        }
        achievements
    }
}

impl<T: ?Sized + Rule> LoggedRule for T {}
