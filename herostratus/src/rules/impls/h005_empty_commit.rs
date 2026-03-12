use std::mem::Discriminant;

use crate::achievement::{AchievementKind, Grant, Meta};
use crate::observer::{CommitContext, Observation};
use crate::rules::rule::Rule;
use crate::rules::rule_plugin::RuleFactory;

const META: Meta = Meta {
    id: 5,
    human_id: "empty-commit",
    name: "You can always add more later",
    description: "Create an empty commit containing no changes",
    kind: AchievementKind::PerUser { recurrent: false },
};

/// Grant an achievement for empty commits (no file changes).
pub struct EmptyCommit;

impl Default for EmptyCommit {
    fn default() -> Self {
        Self
    }
}

inventory::submit!(RuleFactory::default::<EmptyCommit>());

impl Rule for EmptyCommit {
    type Cache = ();

    fn meta(&self) -> &Meta {
        &META
    }

    fn consumes(&self) -> &'static [Discriminant<Observation>] {
        &[Observation::EMPTY_COMMIT]
    }

    fn process(&mut self, ctx: &CommitContext, obs: &Observation) -> eyre::Result<Option<Grant>> {
        match obs {
            Observation::EmptyCommit => Ok(Some(META.grant(ctx))),
            _ => Ok(None),
        }
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn grants_on_empty_commit() {
        let mut rule = EmptyCommit;
        let grant = rule
            .process(&CommitContext::test("Test"), &Observation::EmptyCommit)
            .unwrap();
        assert!(grant.is_some());
    }
}
