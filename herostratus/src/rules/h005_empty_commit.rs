use crate::achievement::{Achievement, AchievementDescriptor};
use crate::rules::{Rule, RuleFactory};

const DESCRIPTORS: [AchievementDescriptor; 1] = [AchievementDescriptor {
    id: 5,
    human_id: "empty-commit",
    name: "You can always add more later",
    description: "Create an empty commit containing no changes",
}];

/// Grant achievements for `git commit --allow-empty` (not merge) commits
pub struct EmptyCommit {
    found_any_change: bool,
}

impl Default for EmptyCommit {
    fn default() -> Self {
        Self {
            found_any_change: false,
        }
    }
}

inventory::submit!(RuleFactory::default::<EmptyCommit>());

impl Rule for EmptyCommit {
    type Cache = ();

    fn descriptors(&self) -> &[AchievementDescriptor] {
        &DESCRIPTORS
    }

    fn is_interested_in_diffs(&self) -> bool {
        true
    }

    fn on_diff_start(&mut self, commit: &gix::Commit, _repo: &gix::Repository) {
        let mut parents = commit.parent_ids();
        let _first_parent = parents.next();
        if parents.next().is_some() {
            // It's a merge commit; we don't care about those
            self.found_any_change = true;
        } else {
            self.found_any_change = false;
        }
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
            vec![DESCRIPTORS[0].grant(commit.id)]
        }
    }
}

// It's hard to test this rule in unit tests because the test fixtures I have support *only* empty
// commits. So this rule has an integration test against the main branch of this repository
// instead.
