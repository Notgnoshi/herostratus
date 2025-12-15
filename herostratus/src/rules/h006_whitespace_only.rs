use crate::achievement::{Achievement, AchievementDescriptor, Rule, RuleFactory};
use crate::bstr::BStr;
use crate::utils::utf8_whitespace::is_equal_ignoring_whitespace;

pub struct WhitespaceOnly {
    descriptors: [AchievementDescriptor; 1],
    /// Whether any non-whitespace change was found
    found_non_whitespace_difference: bool,
    /// Whether any change was found at all
    found_any_change: bool,
}

impl Default for WhitespaceOnly {
    fn default() -> Self {
        Self {
            descriptors: [AchievementDescriptor {
                enabled: true,
                id: 6,
                human_id: "whitespace-only",
                name: "Whitespace Warrior",
                description: "Make a whitespace-only change",
            }],
            found_non_whitespace_difference: false,
            found_any_change: false,
        }
    }
}

inventory::submit!(RuleFactory::default::<WhitespaceOnly>());

impl Rule for WhitespaceOnly {
    fn get_descriptors(&self) -> &[AchievementDescriptor] {
        &self.descriptors
    }
    fn get_descriptors_mut(&mut self) -> &mut [AchievementDescriptor] {
        &mut self.descriptors
    }

    fn is_interested_in_diffs(&self) -> bool {
        true
    }

    fn on_diff_start(&mut self, _commit: &gix::Commit, _repo: &gix::Repository) {
        self.found_non_whitespace_difference = false;
        self.found_any_change = false;
    }

    fn on_diff_change(
        &mut self,
        commit: &gix::Commit,
        repo: &gix::Repository,
        change: &gix::object::tree::diff::Change,
    ) -> eyre::Result<gix::object::tree::diff::Action> {
        self.found_any_change = true;

        match change {
            gix::object::tree::diff::Change::Modification {
                previous_id,
                id,
                entry_mode,
                ..
            } => {
                if entry_mode.is_commit() {
                    // Submodule updates look like commit entry modes
                    self.found_non_whitespace_difference = true;
                    return Ok(gix::object::tree::diff::Action::Cancel);
                }
                self.on_modification(commit, repo, *previous_id, *id)
            }

            // Additions, deletions, and rewrites are always non-whitespace changes
            gix::object::tree::diff::Change::Addition { .. }
            | gix::object::tree::diff::Change::Deletion { .. }
            | gix::object::tree::diff::Change::Rewrite { .. } => {
                self.found_non_whitespace_difference = true;
                Ok(gix::object::tree::diff::Action::Cancel)
            }
        }
    }

    fn on_diff_end(&mut self, commit: &gix::Commit, _repo: &gix::Repository) -> Vec<Achievement> {
        // Don't claim that empty commits containing no changes are whitespace-only changes!
        if self.found_non_whitespace_difference || !self.found_any_change {
            Vec::new()
        } else {
            vec![Achievement {
                name: self.descriptors[0].name,
                commit: commit.id,
            }]
        }
    }
}

impl WhitespaceOnly {
    fn on_modification(
        &mut self,
        commit: &gix::Commit,
        repo: &gix::Repository,
        previous_id: gix::Id,
        id: gix::Id,
    ) -> eyre::Result<gix::object::tree::diff::Action> {
        let before = repo
            .find_object(previous_id)
            .inspect_err(|e| {
                tracing::error!(
                    "Commit: {commit:?} previous: {previous_id:?} current: {id:?} error: {e:?}"
                )
            })
            .unwrap();
        let after = repo
            .find_object(id)
            .inspect_err(|e| {
                tracing::error!(
                    "Commit: {commit:?} previous: {previous_id:?} current: {id:?} error: {e:?}"
                )
            })
            .unwrap();
        if before.kind == gix::object::Kind::Tree {
            return Ok(gix::object::tree::diff::Action::Continue);
        }

        let before_s = BStr::new(&before.data);
        let after_s = BStr::new(&after.data);

        if !is_equal_ignoring_whitespace(before_s, after_s) {
            self.found_non_whitespace_difference = true;
            Ok(gix::object::tree::diff::Action::Cancel)
        } else {
            Ok(gix::object::tree::diff::Action::Continue)
        }
    }
}
