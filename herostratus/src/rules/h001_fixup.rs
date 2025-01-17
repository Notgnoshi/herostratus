use crate::achievement::{Achievement, Rule, RuleFactory};

/// Grant achievements for commits starting with
///
/// * `fixup!`, `squash!`, `amend!` (generated by `git commit --fixup|--squash`)
/// * `WIP:`, `TODO:`, `FIXME:` `DROPME:` (ad-hoc patterns that I've seen in the wild)
#[derive(Default)]
pub struct Fixup;

inventory::submit!(RuleFactory::default::<Fixup>());

const FIXUP_PREFIXES: [&str; 11] = [
    "fixup!", "squash!", "amend!", "WIP", "TODO", "FIXME", "DROPME",
    // avoid false positives by accepting false negatives. Of all these patterns, "wip" is the one
    // that's most likely to be a part of a real word.
    "wip:", "todo", "fixme", "dropme",
];

impl Rule for Fixup {
    fn id(&self) -> usize {
        1
    }
    fn human_id(&self) -> &'static str {
        "fixup"
    }
    fn name(&self) -> &'static str {
        "I meant to fix that up later, I swear!"
    }
    fn description(&self) -> &'static str {
        "Prefix a commit message with a !fixup marker"
    }
    fn process(&mut self, commit: &git2::Commit, repo: &git2::Repository) -> Option<Achievement> {
        let summary = commit.summary()?;
        for pattern in FIXUP_PREFIXES {
            if summary.starts_with(pattern) {
                let achievement = self.grant(commit, repo);
                return Some(achievement);
            }
        }
        None
    }
}
