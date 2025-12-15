use crate::achievement::{Achievement, AchievementDescriptor, Rule, RuleFactory};

/// Grant achievements for `git commit --allow-empty` (not merge) commits
pub struct EmptyCommit {
    descriptors: [AchievementDescriptor; 1],
    found_any_change: bool,
}

impl Default for EmptyCommit {
    fn default() -> Self {
        Self {
            descriptors: [AchievementDescriptor {
                enabled: true,
                id: 5,
                human_id: "empty-commit",
                name: "You can always add more later",
                description: "Create an empty commit containing no changes",
            }],
            found_any_change: false,
        }
    }
}

inventory::submit!(RuleFactory::default::<EmptyCommit>());

impl Rule for EmptyCommit {
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
        self.found_any_change = false;
    }

    fn on_diff_change(
        &mut self,
        _commit: &gix::Commit,
        _repo: &gix::Repository,
        _change: &gix::object::tree::diff::Change,
    ) -> eyre::Result<gix::object::tree::diff::Action> {
        self.found_any_change = true;
        Ok(gix::object::tree::diff::Action::Cancel)
    }

    fn on_diff_end(&mut self, commit: &gix::Commit, _repo: &gix::Repository) -> Vec<Achievement> {
        if self.found_any_change {
            Vec::new()
        } else {
            vec![Achievement {
                name: self.descriptors[0].name,
                commit: commit.id,
            }]
        }
    }
}

// It's hard to test this rule in unit tests because the test fixtures I have support *only* empty
// commits. So this rule has an integration test against the main branch of this repository
// instead.
