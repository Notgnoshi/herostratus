use std::mem::Discriminant;

use crate::achievement::{AchievementKind, Grant, Meta};
use crate::observer::{CommitContext, Observation};
use crate::rules::rule::Rule;
use crate::rules::rule_plugin::RuleFactory;

const META: Meta = Meta {
    id: 4,
    human_id: "non-unicode",
    name: "But ... How?!",
    description: "Make a commit message containing a non UTF-8 byte",
    kind: AchievementKind::PerUser { recurrent: false },
};

/// Grant an achievement for commits with non-UTF-8 bytes in the message.
pub struct NonUnicode;

impl Default for NonUnicode {
    fn default() -> Self {
        Self
    }
}

inventory::submit!(RuleFactory::default::<NonUnicode>());

impl Rule for NonUnicode {
    type Cache = ();

    fn meta(&self) -> &Meta {
        &META
    }

    fn consumes(&self) -> &'static [Discriminant<Observation>] {
        &[Observation::NON_UNICODE_MESSAGE]
    }

    fn process(&mut self, ctx: &CommitContext, obs: &Observation) -> eyre::Result<Option<Grant>> {
        match obs {
            Observation::NonUnicodeMessage => Ok(Some(META.grant(ctx))),
            _ => Ok(None),
        }
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn grants_on_non_unicode() {
        let mut rule = NonUnicode;
        let grant = rule
            .process(
                &CommitContext::test("Test"),
                &Observation::NonUnicodeMessage,
            )
            .unwrap();
        assert!(grant.is_some());
    }
}
