use std::mem::Discriminant;

use crate::achievement::{AchievementKind, Grant, Meta};
use crate::observer::{CommitContext, Observation};
use crate::rules::rule::Rule;
use crate::rules::rule_plugin::RuleFactory;

const META: Meta = Meta {
    id: 6,
    human_id: "whitespace-only",
    name: "Whitespace Warrior",
    description: "Make a whitespace-only change",
    kind: AchievementKind::PerUser { recurrent: false },
};

/// Grant an achievement for commits where every file change is whitespace-only.
pub struct WhitespaceOnly;

impl Default for WhitespaceOnly {
    fn default() -> Self {
        Self
    }
}

inventory::submit!(RuleFactory::default::<WhitespaceOnly>());

impl Rule for WhitespaceOnly {
    type Cache = ();

    fn meta(&self) -> &Meta {
        &META
    }

    fn consumes(&self) -> &'static [Discriminant<Observation>] {
        &[Observation::WHITESPACE_ONLY]
    }

    fn process(&mut self, ctx: &CommitContext, obs: &Observation) -> eyre::Result<Option<Grant>> {
        match obs {
            Observation::WhitespaceOnly => Ok(Some(META.grant(ctx))),
            _ => Ok(None),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx() -> CommitContext {
        CommitContext {
            oid: gix::ObjectId::null(gix::hash::Kind::Sha1),
            author_name: "Test".to_string(),
            author_email: "test@example.com".to_string(),
        }
    }

    #[test]
    fn grants_on_whitespace_only() {
        let mut rule = WhitespaceOnly;
        let grant = rule.process(&ctx(), &Observation::WhitespaceOnly).unwrap();
        assert!(grant.is_some());
    }
}
