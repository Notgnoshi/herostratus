use std::mem::Discriminant;

use crate::achievement::{AchievementKind, Grant, Meta};
use crate::config::RulesConfig;
use crate::observer::{CommitContext, Observation};
use crate::rules::TentacleMergeConfig;
use crate::rules::rule::Rule;
use crate::rules::rule_plugin::RuleFactory;

const META: Meta = Meta {
    id: 15,
    human_id: "octopus",
    name: "So You Have a Thing for Tentacles?",
    description: "Create an octopus merge commit with three or more parents",
    kind: AchievementKind::PerUser { recurrent: true },
};

#[derive(Default)]
pub struct Octopus {
    config: TentacleMergeConfig,
}

fn octopus_factory(config: &RulesConfig) -> Box<dyn crate::rules::rule_plugin::RulePlugin> {
    Box::new(Octopus {
        config: config.tentacle_merge.clone().unwrap_or_default(),
    })
}
inventory::submit!(RuleFactory::new(octopus_factory));

impl Rule for Octopus {
    type Cache = ();

    fn meta(&self) -> &Meta {
        &META
    }

    fn consumes(&self) -> &'static [Discriminant<Observation>] {
        &[Observation::PARENT_COUNT]
    }

    fn process(&mut self, ctx: &CommitContext, obs: &Observation) -> eyre::Result<Option<Grant>> {
        let Observation::ParentCount { count } = obs else {
            return Ok(None);
        };
        if *count >= self.config.octopus_threshold && *count < self.config.cthulhu_threshold {
            return Ok(Some(META.grant(ctx)));
        }
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_grant_for_two_parents() {
        let mut r = Octopus::default();
        let g = r
            .process(
                &CommitContext::test("Alice"),
                &Observation::ParentCount { count: 2 },
            )
            .unwrap();
        assert!(g.is_none());
    }

    #[test]
    fn grants_just_below_cthulhu_threshold() {
        let mut r = Octopus::default();
        let g = r
            .process(
                &CommitContext::test("Alice"),
                &Observation::ParentCount { count: 7 },
            )
            .unwrap();
        assert!(g.is_some());
    }

    #[test]
    fn no_grant_at_cthulhu_threshold() {
        let mut r = Octopus::default();
        let g = r
            .process(
                &CommitContext::test("Alice"),
                &Observation::ParentCount { count: 8 },
            )
            .unwrap();
        assert!(g.is_none());
    }
}
