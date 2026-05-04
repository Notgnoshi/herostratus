use std::mem::Discriminant;

use crate::achievement::{AchievementKind, Grant, Meta};
use crate::config::RulesConfig;
use crate::observer::{CommitContext, Observation};
use crate::rules::TentacleMergeConfig;
use crate::rules::rule::Rule;
use crate::rules::rule_plugin::RuleFactory;

const META: Meta = Meta {
    id: 16,
    human_id: "cthulhu",
    name: "Ph'nglui mglw'nafh Cthulhu R'lyeh wgah'nagl fhtagn",
    description: "Conduct the unholy rite of binding many parent lineages into a single dark covenant. The stars grow correct.",
    kind: AchievementKind::PerUser { recurrent: true },
};

#[derive(Default)]
pub struct Cthulhu {
    config: TentacleMergeConfig,
}

fn cthulhu_factory(config: &RulesConfig) -> Box<dyn crate::rules::rule_plugin::RulePlugin> {
    Box::new(Cthulhu {
        config: config.tentacle_merge.clone().unwrap_or_default(),
    })
}
inventory::submit!(RuleFactory::new(cthulhu_factory));

impl Rule for Cthulhu {
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
        if *count < self.config.cthulhu_threshold {
            return Ok(None);
        }
        let description = format!(
            "Conduct the unholy rite of binding {count} parent lineages into a single dark covenant. The stars grow correct."
        );
        Ok(Some(META.grant(ctx).with_description(description)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_grant_below_threshold() {
        let mut r = Cthulhu::default();
        let g = r
            .process(
                &CommitContext::test("Alice"),
                &Observation::ParentCount { count: 7 },
            )
            .unwrap();
        assert!(g.is_none());
    }

    #[test]
    fn grants_at_threshold_with_dynamic_description() {
        let mut r = Cthulhu::default();
        let g = r
            .process(
                &CommitContext::test("Alice"),
                &Observation::ParentCount { count: 8 },
            )
            .unwrap()
            .expect("expected a grant at the threshold");
        let description = g
            .description_override
            .expect("expected a per-grant description");
        assert!(
            description.contains("8 parent lineages"),
            "got {description:?}"
        );
    }

    #[test]
    fn grants_at_higher_count() {
        let mut r = Cthulhu::default();
        let g = r
            .process(
                &CommitContext::test("Alice"),
                &Observation::ParentCount { count: 42 },
            )
            .unwrap()
            .expect("expected a grant");
        let description = g.description_override.unwrap();
        assert!(description.contains("42 parent lineages"));
    }
}
