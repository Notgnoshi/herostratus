use crate::achievement::{Achievement, Rule, RuleFactory};

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
        None
    }
}
