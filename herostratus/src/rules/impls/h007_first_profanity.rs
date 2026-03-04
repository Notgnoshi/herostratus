use std::mem::Discriminant;

use crate::achievement::{AchievementKind, Grant, Meta};
use crate::observer::{CommitContext, Observation};
use crate::rules::rule::Rule;
use crate::rules::rule_plugin::RuleFactory;

const META: Meta = Meta {
    id: 7,
    human_id: "first-profanity",
    name: "First!",
    description: "Be the first person to swear in the repository",
    kind: AchievementKind::Global { revocable: false },
};

/// Grant an achievement to the first person who swears in the repository.
///
/// The rule itself is stateless -- it grants on every Profanity observation. The AchievementLog
/// enforces the "first person only" semantics via Global { revocable: false }.
#[derive(Default)]
pub struct FirstProfanity;

inventory::submit!(RuleFactory::default::<FirstProfanity>());

impl Rule for FirstProfanity {
    type Cache = ();

    fn meta(&self) -> &Meta {
        &META
    }

    fn consumes(&self) -> &'static [Discriminant<Observation>] {
        &[Observation::PROFANITY]
    }

    fn process(&mut self, ctx: &CommitContext, obs: &Observation) -> eyre::Result<Option<Grant>> {
        match obs {
            Observation::Profanity { .. } => Ok(Some(META.grant(ctx))),
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

    fn profanity() -> Observation {
        Observation::Profanity {
            word: "damn".to_string(),
        }
    }

    #[test]
    fn grants_on_profanity() {
        let mut rule = FirstProfanity;
        let grant = rule.process(&ctx(), &profanity()).unwrap();
        assert!(grant.is_some());
    }

    #[test]
    fn ignores_other_observations() {
        let mut rule = FirstProfanity;
        let grant = rule.process(&ctx(), &Observation::Fixup).unwrap();
        assert!(grant.is_none());
    }
}
