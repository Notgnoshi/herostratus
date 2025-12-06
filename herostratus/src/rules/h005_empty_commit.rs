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
                    self.pretty_id(),
                    e
                );
            })
            .ok()
            .flatten()
    }
}

impl EmptyCommit {
    fn impl_process(
        &self,
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
        let mut cache = repo.diff_resource_cache_for_tree_diff()?;
        let mut found_any_change = false;
        match changes.for_each_to_obtain_tree_with_cache(
            &commit_tree,
            &mut cache,
            |_change| -> eyre::Result<gix::object::tree::diff::Action> {
                on_change(&mut found_any_change)
            },
        ) {
            Ok(_) => {}
            // It's not an error for the diff iterator to cancel iteration
            Err(gix::object::tree::diff::for_each::Error::Diff(
                gix::diff::tree_with_rewrites::Error::Diff(gix::diff::tree::Error::Cancelled),
            )) => {}
            Err(e) => return Err(e.into()),
        }

        if found_any_change {
            Ok(None)
        } else {
            Ok(Some(self.grant(commit, repo)))
        }
    }
}

fn on_change(found_any_change: &mut bool) -> eyre::Result<gix::object::tree::diff::Action> {
    *found_any_change = true;
    Ok(gix::object::tree::diff::Action::Cancel)
}

// It's hard to test this rule in unit tests because the test fixtures I have support *only* empty
// commits. So this rule has an integration test against the main branch of this repository
// instead.
