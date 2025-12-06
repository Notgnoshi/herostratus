use crate::achievement::{Achievement, Rule, RuleFactory};
use crate::bstr::BStr;
use crate::utils::utf8_whitespace::is_equal_ignoring_whitespace;

#[derive(Default)]
pub struct WhitespaceOnly;

inventory::submit!(RuleFactory::default::<WhitespaceOnly>());

impl Rule for WhitespaceOnly {
    fn id(&self) -> usize {
        6
    }
    fn human_id(&self) -> &'static str {
        "whitespace-only"
    }
    fn name(&self) -> &'static str {
        "Whitespace Warrior"
    }
    fn description(&self) -> &'static str {
        "Make a whitespace-only change"
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

impl WhitespaceOnly {
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

        let mut found_non_whitespace = false;
        // Empty commits won't trigger the on_change callback, so we keep track if any changes were
        // found, because empty commits aren't whitespace changes.
        let mut found_any_change = false;
        // TODO: Does the cache need any custom config options?
        let mut cache = repo.diff_resource_cache_for_tree_diff()?;
        match changes.for_each_to_obtain_tree_with_cache(
            &commit_tree,
            &mut cache,
            |change| -> eyre::Result<gix::object::tree::diff::Action> {
                on_change(
                    commit,
                    repo,
                    change,
                    &mut found_non_whitespace,
                    &mut found_any_change,
                )
            },
        ) {
            Ok(_) => {}
            // It's not an error for the diff iterator to cancel iteration; that means it found a
            // non-whitespace difference, and is short-circuiting.
            Err(gix::object::tree::diff::for_each::Error::Diff(
                gix::diff::tree_with_rewrites::Error::Diff(gix::diff::tree::Error::Cancelled),
            )) => {}
            Err(e) => return Err(e.into()),
        }

        if found_non_whitespace || !found_any_change {
            Ok(None)
        } else {
            Ok(Some(self.grant(commit, repo)))
        }
    }
}

fn on_change(
    commit: &gix::Commit,
    repo: &gix::Repository,
    change: gix::object::tree::diff::Change,
    found_non_whitespace: &mut bool,
    found_any_change: &mut bool,
) -> eyre::Result<gix::object::tree::diff::Action> {
    *found_any_change = true;
    match change {
        gix::object::tree::diff::Change::Modification {
            previous_id,
            id,
            entry_mode,
            ..
        } => {
            // This commit contained a submodule update
            if entry_mode.is_commit() {
                *found_non_whitespace = true;
                return Ok(gix::object::tree::diff::Action::Cancel);
            }
            on_modification(commit, repo, previous_id, id, found_non_whitespace)
        }
        _ => {
            *found_non_whitespace = true;
            Ok(gix::object::tree::diff::Action::Cancel)
        }
    }
}

fn on_modification(
    commit: &gix::Commit,
    repo: &gix::Repository,
    previous_id: gix::Id,
    id: gix::Id,
    found_non_whitespace: &mut bool,
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
        *found_non_whitespace = true;
        Ok(gix::object::tree::diff::Action::Cancel)
    } else {
        Ok(gix::object::tree::diff::Action::Continue)
    }
}
