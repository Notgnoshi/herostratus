use std::mem::Discriminant;

use crate::achievement::{AchievementKind, Grant, Meta};
use crate::observer::{CommitContext, Observation};
use crate::rules::rule::Rule;
use crate::rules::rule_plugin::RuleFactory;

const META: Meta = Meta {
    id: 8,
    human_id: "potty-mouth",
    name: "Potty Mouth",
    description: "Use profanity in a commit message",
    kind: AchievementKind::PerUser { recurrent: false },
};

/// Grant an achievement when a user swears for the first time.
///
/// The rule itself is stateless -- it grants on every Profanity observation. The AchievementLog
/// enforces the per-user deduplication via PerUser { recurrent: false }.
#[derive(Default)]
pub struct PottyMouth;

inventory::submit!(RuleFactory::default::<PottyMouth>());

impl Rule for PottyMouth {
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

    fn profanity() -> Observation {
        Observation::Profanity {
            words: vec!["shit".to_string()],
        }
    }

    #[test]
    fn grants_on_profanity() {
        let mut rule = PottyMouth;
        let grant = rule
            .process(&CommitContext::test("Test"), &profanity())
            .unwrap();
        assert!(grant.is_some());
    }
}
