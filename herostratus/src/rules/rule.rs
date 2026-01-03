use crate::achievement::{Achievement, AchievementDescriptor};

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
