use std::mem::Discriminant;

use crate::achievement::{AchievementKind, Grant, Meta};
use crate::config::RulesConfig;
use crate::observer::{CommitContext, Observation};
use crate::rules::rule::Rule;
use crate::rules::rule_plugin::RuleFactory;

#[derive(Debug, Clone, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct H012Config {
    /// Minimum size of the short hash prefix to consider. Can't go below 5
    pub min_matched_chars: usize,
}

impl Default for H012Config {
    fn default() -> Self {
        Self {
            min_matched_chars: 7,
        }
    }
}

const META: Meta = Meta {
    id: 12,
    human_id: "quine-commit",
    name: "How Did You Even Do That?!",
    description: "A commit message that contains a prefix of its own commit hash",
    kind: AchievementKind::PerUser { recurrent: true },
};

/// Grant an achievement when a commit message contains a prefix of its own commit hash.
///
/// This is extraordinarily unlikely to happen by chance. The default threshold of 7 hex characters
/// means a 1-in-268-million chance per commit.
pub struct QuineCommit {
    threshold: usize,
}

impl Default for QuineCommit {
    fn default() -> Self {
        Self { threshold: 7 }
    }
}

fn quine_commit_factory(config: &RulesConfig) -> Box<dyn crate::rules::rule_plugin::RulePlugin> {
    Box::new(QuineCommit {
        threshold: config
            .h12_quine_commit
            .as_ref()
            .map_or(7, |c| c.min_matched_chars),
    })
}
inventory::submit!(RuleFactory::new(quine_commit_factory));

impl Rule for QuineCommit {
    type Cache = ();

    fn meta(&self) -> &Meta {
        &META
    }

    fn consumes(&self) -> &'static [Discriminant<Observation>] {
        &[Observation::QUINE_PREFIX]
    }

    fn process(&mut self, ctx: &CommitContext, obs: &Observation) -> eyre::Result<Option<Grant>> {
        let Observation::QuinePrefix { matched_length } = obs else {
            return Ok(None);
        };

        if *matched_length >= self.threshold {
            Ok(Some(META.grant(ctx)))
        } else {
            Ok(None)
        }
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn grants_when_match_meets_threshold() {
        let mut rule = QuineCommit { threshold: 7 };
        let grant = rule
            .process(
                &CommitContext::test("Test"),
                &Observation::QuinePrefix { matched_length: 10 },
            )
            .unwrap();
        assert!(grant.is_some());
    }

    #[test]
    fn rejects_below_threshold() {
        let mut rule = QuineCommit { threshold: 6 };
        let grant = rule
            .process(
                &CommitContext::test("Test"),
                &Observation::QuinePrefix { matched_length: 5 },
            )
            .unwrap();
        assert!(grant.is_none());
    }
}
