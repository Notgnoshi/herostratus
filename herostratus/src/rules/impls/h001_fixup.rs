use std::mem::Discriminant;

use crate::achievement::{AchievementKind, Grant, Meta};
use crate::observer::{CommitContext, Observation};
use crate::rules::rule::Rule;
use crate::rules::rule_plugin::RuleFactory;

const META: Meta = Meta {
    id: 1,
    human_id: "fixup",
    name: "I'll fix that up later",
    description: "Prefix a commit message with a !fixup marker",
    kind: AchievementKind::PerUser { recurrent: false },
};

/// Grant an achievement for commits starting with a fixup/squash/amend/WIP/etc prefix.
pub struct Fixup;

impl Default for Fixup {
    fn default() -> Self {
        Self
    }
}

inventory::submit!(RuleFactory::default::<Fixup>());

impl Rule for Fixup {
    type Cache = ();

    fn meta(&self) -> &Meta {
        &META
    }

    fn consumes(&self) -> &'static [Discriminant<Observation>] {
        &[Observation::FIXUP]
    }

    fn process(&mut self, ctx: &CommitContext, obs: &Observation) -> eyre::Result<Option<Grant>> {
        match obs {
            Observation::Fixup => Ok(Some(META.grant(ctx))),
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
    fn grants_on_fixup() {
        let mut rule = Fixup;
        let grant = rule.process(&ctx(), &Observation::Fixup).unwrap();
        assert!(grant.is_some());
    }
}
