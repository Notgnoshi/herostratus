//! The achievements builtin to Herostratus
mod h001_fixup;
mod h002_shortest_subject_line;
mod h003_longest_subject_line;
mod h004_non_unicode;

pub use h002_shortest_subject_line::H002Config;
pub use h003_longest_subject_line::H003Config;

use crate::achievement::{Rule, RuleFactory};
use crate::config::{Config, RulesConfig};

/// Get a new instance of each builtin [Rule]
pub fn builtin_rules(config: Option<&Config>) -> Vec<Box<dyn Rule>> {
    let default_rules_config = RulesConfig::default();
    let rules_config = match config {
        Some(Config {
            repositories: _,
            rules: Some(r),
        }) => r,
        _ => &default_rules_config,
    };

    // Each Rule uses inventory::submit! to register a factory to build themselves with.
    inventory::iter::<RuleFactory>
        .into_iter()
        .map(|factory| factory.build(rules_config))
        .collect()
}

pub fn builtin_rules_all() -> Vec<Box<dyn Rule>> {
    builtin_rules(None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[track_caller]
    fn assert_shares_no_element<T: PartialEq>(
        rules: &[Box<dyn Rule>],
        rule_metadata: &[T],
        desc: &str,
    ) {
        assert_eq!(rules.len(), rule_metadata.len());

        // use n^2 test because it allows keeping each array of metadata in the same order, and
        // it's small n, so no need to reach for fancier algorithm
        for (idx1, e1) in rule_metadata.iter().enumerate() {
            for (idx2, e2) in rule_metadata.iter().enumerate() {
                if idx1 == idx2 {
                    continue;
                }
                if e1 == e2 {
                    panic!(
                        "Rule {:?} shares the same {desc} as rule {:?}",
                        rules[idx1].pretty_id(),
                        rules[idx2].pretty_id()
                    );
                }
            }
        }
    }

    #[test]
    fn no_rules_have_duplicate_metadata() {
        let rules = builtin_rules_all();

        let ids: Vec<_> = rules.iter().map(|r| r.id()).collect();
        let human_ids: Vec<_> = rules.iter().map(|r| r.human_id()).collect();
        let pretty_ids: Vec<_> = rules.iter().map(|r| r.pretty_id()).collect();
        let names: Vec<_> = rules.iter().map(|r| r.name()).collect();
        let descriptions: Vec<_> = rules.iter().map(|r| r.description()).collect();

        assert_shares_no_element(&rules, &ids, "ID");
        assert_shares_no_element(&rules, &human_ids, "human ID");
        assert_shares_no_element(&rules, &pretty_ids, "pretty ID");
        assert_shares_no_element(&rules, &names, "name");
        assert_shares_no_element(&rules, &descriptions, "description");
    }

    #[test]
    fn all_rules_have_expected_metadata() {
        let rules = builtin_rules_all();
        // Check that there's actually a few rules registered
        assert!(rules.len() >= 3);

        for rule in &rules {
            // These two assertions, combined with the no-duplicate IDs test above, when combined,
            // imply that the rule IDs start at 1 and count upwards
            assert_ne!(rule.id(), 0);
            assert!(rule.id() <= rules.len());

            // No overriding the pretty ID
            assert_eq!(
                rule.pretty_id(),
                format!("H{}-{}", rule.id(), rule.human_id())
            );

            // Ensure each are set
            assert!(rule.human_id().len() > 4);
            assert!(rule.name().len() > 4);
            assert!(rule.description().len() > 4);
            assert_ne!(rule.human_id().to_lowercase(), "todo");
            assert_ne!(rule.name().to_lowercase(), "todo");
            assert_ne!(rule.description().to_lowercase(), "todo");
        }
    }
}
