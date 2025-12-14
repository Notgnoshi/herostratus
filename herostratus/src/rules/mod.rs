//! The achievements builtin to Herostratus
mod h001_fixup;
mod h002_h003_subject_line;
mod h004_non_unicode;
mod h005_empty_commit;
mod h006_whitespace_only;
#[cfg(test)]
pub(crate) mod test_rules;

pub use h002_h003_subject_line::{H002Config, H003Config};

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
    let excludes = rules_config.exclude.as_deref().unwrap_or_default();
    let includes = rules_config.include.as_deref().unwrap_or_default();

    let mut rules = Vec::new();
    // Each Rule uses inventory::submit! to register a factory to build themselves with.
    for factory in inventory::iter::<RuleFactory>.into_iter() {
        let rule = factory.build(rules_config);
        rules.push(rule);
    }

    for rule in &mut rules {
        let mut ids_to_disable = Vec::new();
        let mut ids_to_enable = Vec::new();

        // Check each rule for matching excludes/includes
        for desc in rule.get_descriptors() {
            for exclude in excludes {
                if exclude == "all" || desc.id_matches(exclude) {
                    ids_to_disable.push(desc.id);
                }
            }
            for include in includes {
                if desc.id_matches(include) {
                    ids_to_enable.push(desc.id);
                }
            }
        }

        for id in ids_to_disable {
            rule.disable_by_id(id);
        }
        for id in ids_to_enable {
            rule.enable_by_id(id);
        }
    }

    // Only keep rules that have at least one enabled descriptor
    rules.retain(|r| r.get_descriptors().iter().all(|d| d.enabled));

    rules
}

pub fn builtin_rules_all() -> Vec<Box<dyn Rule>> {
    builtin_rules(None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::achievement::AchievementDescriptor;

    #[track_caller]
    fn get_all_descriptors() -> Vec<AchievementDescriptor> {
        let rules = builtin_rules_all();
        let descs: Vec<_> = rules
            .iter()
            .flat_map(|r| r.get_descriptors())
            .cloned()
            .collect();
        assert!(!descs.is_empty());
        descs
    }

    #[test]
    fn no_rules_have_duplicate_metadata() {
        let descs = get_all_descriptors();

        // All-pairs comparison, skipping self-comparisons
        for desc1 in &descs {
            for desc2 in &descs {
                if std::ptr::eq(desc1, desc2) {
                    continue;
                }

                assert_ne!(desc1.id, desc2.id);
                assert_ne!(desc1.human_id, desc2.human_id);
                assert_ne!(desc1.pretty_id(), desc2.pretty_id());
                assert_ne!(desc1.name, desc2.name);
                assert_ne!(desc1.description, desc2.description);
            }
        }
    }

    #[test]
    fn all_rules_have_expected_metadata() {
        let descs = get_all_descriptors();
        for desc in &descs {
            // These two assertions, combined with the no-duplicate IDs test above, when combined,
            // imply that the rule IDs start at 1 and count upwards without gaps.
            assert_ne!(desc.id, 0);
            assert!(desc.id <= descs.len());

            // No overriding the pretty ID
            assert_eq!(desc.pretty_id(), format!("H{}-{}", desc.id, desc.human_id));

            // Ensure each are set
            assert!(desc.human_id.len() > 4);
            assert!(desc.name.len() > 4);
            assert!(desc.description.len() > 4);
        }
    }

    #[test]
    fn rule_metadata_characteristics() {
        let descs = get_all_descriptors();
        for desc in &descs {
            // Names start with capitals (if they start with an alphabetic character)
            let first = desc.name.chars().next().unwrap();
            assert!((first.is_alphabetic() && first.is_uppercase()) || first.is_numeric());

            // Names are allowed to be a single word, but descriptions are not
            let words = desc.description.split_whitespace();
            assert!(words.count() > 2);

            // Human IDs are lower-alphabetic-only, separated by hyphens
            let words = desc.human_id.split('-');
            for word in words {
                assert!(word.chars().all(|c| c.is_alphabetic()));
                assert!(word.chars().all(|c| c.is_lowercase()));
            }
        }
    }

    #[test]
    fn exclude_rules() {
        let config = RulesConfig {
            exclude: Some(vec![
                "H2".to_string(),                   // H2, short pretty id
                "longest-subject-line".to_string(), // H3, human id
                "H4-non-unicode".to_string(),       // H4, pretty id
            ]),
            ..Default::default()
        };
        let config = Config {
            rules: Some(config),
            ..Default::default()
        };

        // Rules 1 through 4 are included by default
        let all_ids: Vec<_> = get_all_descriptors().iter().map(|r| r.id).collect();
        assert!(all_ids.contains(&1));
        assert!(all_ids.contains(&2));
        assert!(all_ids.contains(&3));
        assert!(all_ids.contains(&4));

        let rules = builtin_rules(Some(&config));
        let ids: Vec<_> = rules
            .iter()
            .flat_map(|r| r.get_descriptors())
            .map(|d| d.id)
            .collect();

        // let at least one rule through, so we can test that we're not excluding everything
        assert!(ids.contains(&1));
        assert!(!ids.contains(&2));
        assert!(!ids.contains(&3));
        assert!(!ids.contains(&4));
    }

    #[test]
    fn exclude_all_rules() {
        let config = RulesConfig {
            exclude: Some(vec!["all".into()]),
            ..Default::default()
        };
        let config = Config {
            rules: Some(config),
            ..Default::default()
        };

        let rules = builtin_rules(Some(&config));
        assert!(rules.is_empty());
    }

    #[test]
    fn exclude_all_rules_except() {
        let config = RulesConfig {
            exclude: Some(vec!["all".into()]),
            include: Some(vec!["H1".into()]),
            ..Default::default()
        };
        let config = Config {
            rules: Some(config),
            ..Default::default()
        };

        let rules = builtin_rules(Some(&config));
        assert_eq!(rules.len(), 1);
        assert_eq!(
            rules[0].get_descriptors().first().unwrap().human_id,
            "fixup"
        );
    }
}
