use crate::achievement::{Achievement, Rule, RuleFactory};

/// Grant achievements for `git commit --allow-empty` (not merge) commits
#[derive(Default)]
pub struct EmptyCommit;

inventory::submit!(RuleFactory::default::<EmptyCommit>());

impl Rule for EmptyCommit {
    fn id(&self) -> usize {
        5
    }
    fn human_id(&self) -> &'static str {
        "empty-commit"
    }
    fn name(&self) -> &'static str {
        "You can always add more later"
    }
    fn description(&self) -> &'static str {
        "Create an empty commit containing no changes"
    }

    fn process(&mut self, commit: &gix::Commit, repo: &gix::Repository) -> Option<Achievement> {
        self.impl_process(commit, repo)
            .inspect_err(|e| {
                tracing::error!(
                    "Error processing commit {} for rule {}: {}",
                    commit.id(),
                    self.human_id(),
                    e
                );
            })
            .ok()
            .flatten()
    }
}

impl EmptyCommit {
    fn impl_process(
        &mut self,
        commit: &gix::Commit,
        repo: &gix::Repository,
    ) -> eyre::Result<Option<Achievement>> {
        // Get the parent of the commit, which may not exist if it's the root commit.
        let mut parents = commit.parent_ids();
        let parent = parents.next();
        if parents.next().is_some() {
            // This is a merge commit, and we want to skip it
            return Ok(None);
        }

        let commit_tree = commit.tree()?;
        let parent_tree = match parent {
            Some(pid) => {
                let parent_commit = repo.find_commit(pid)?;
                parent_commit.tree()?
            }
            None => repo.empty_tree(),
        };

        let mut changes = parent_tree.changes()?;
        changes.options(|o| {
            o.track_rewrites(None);
        });
        // Calculating the stats is easier (I think) that calculating the diff and checking if it's
        // empty. For rules that actually need to look at the diff,
        // Platform::for_each_to_obtain_tree_with_cache() should be used.
        let stats = changes.stats(&commit_tree)?;

        if stats.lines_added == 0 && stats.lines_removed == 0 && stats.files_changed == 0 {
            return Ok(Some(self.grant(commit, repo)));
        }

        Ok(None)
    }
}

// It's hard to test this rule in unit tests because the test fixtures I have support *only* empty
// commits. So this rule has an integration test against the main branch of this repository
// instead.
