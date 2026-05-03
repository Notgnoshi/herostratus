use std::mem::Discriminant;

use crate::achievement::{AchievementKind, Grant, Meta};
use crate::observer::{CommitContext, Observation};
use crate::rules::rule::Rule;
use crate::rules::rule_plugin::RuleFactory;

const META: Meta = Meta {
    id: 17,
    human_id: "ex-nihilo",
    name: "Ex Nihilo",
    description: "Create an empty initial commit",
    kind: AchievementKind::PerUser { recurrent: true },
};

#[derive(Default)]
pub struct ExNihilo {
    is_root: bool,
    is_empty: bool,
}

inventory::submit!(RuleFactory::default::<ExNihilo>());

impl Rule for ExNihilo {
    type Cache = ();

    fn meta(&self) -> &Meta {
        &META
    }

    fn consumes(&self) -> &'static [Discriminant<Observation>] {
        &[Observation::PARENT_COUNT, Observation::EMPTY_COMMIT]
    }

    fn commit_start(&mut self, _ctx: &CommitContext) -> eyre::Result<()> {
        // Reset state between commits
        self.is_root = false;
        self.is_empty = false;
        Ok(())
    }

    /// Observations can arrive in any order, so we handle observations in process(), but only
    /// grant in commit_complete() once we've seen all observations.
    fn process(&mut self, _ctx: &CommitContext, obs: &Observation) -> eyre::Result<Option<Grant>> {
        match obs {
            Observation::ParentCount { count: 0 } => {
                self.is_root = true;
            }
            Observation::EmptyCommit => {
                self.is_empty = true;
            }
            _ => {}
        }
        Ok(None)
    }

    fn commit_complete(&mut self, ctx: &CommitContext) -> eyre::Result<Option<Grant>> {
        if self.is_root && self.is_empty {
            Ok(Some(META.grant(ctx)))
        } else {
            Ok(None)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run_rule(observations: &[Observation]) -> Option<Grant> {
        let mut rule = ExNihilo::default();
        let ctx = CommitContext::test("Alice");
        rule.commit_start(&ctx).unwrap();
        for obs in observations {
            rule.process(&ctx, obs).unwrap();
        }
        rule.commit_complete(&ctx).unwrap()
    }

    #[test]
    fn grants_when_root_then_empty() {
        let g = run_rule(&[
            Observation::ParentCount { count: 0 },
            Observation::EmptyCommit,
        ]);
        assert!(g.is_some());
    }

    #[test]
    fn grants_when_empty_then_root() {
        let g = run_rule(&[
            Observation::EmptyCommit,
            Observation::ParentCount { count: 0 },
        ]);
        assert!(g.is_some());
    }

    #[test]
    fn no_grant_for_root_with_content() {
        // ParentCount==0 but no EmptyCommit (root commit that has files)
        let g = run_rule(&[Observation::ParentCount { count: 0 }]);
        assert!(g.is_none());
    }

    #[test]
    fn no_grant_for_non_root_empty_commit() {
        let g = run_rule(&[
            Observation::ParentCount { count: 1 },
            Observation::EmptyCommit,
        ]);
        assert!(g.is_none());
    }
}
