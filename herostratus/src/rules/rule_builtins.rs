use crate::achievement::{RuleFactory, RulePlugin};
use crate::config::{Config, RulesConfig};

/// Get a new instance of each builtin [RulePlugin] with the given configuration
pub fn builtin_rules(config: Option<&Config>) -> Vec<Box<dyn RulePlugin>> {
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

/// Get a new instance of each builtin [RulePlugin] with default configuration
pub fn builtin_rules_all() -> Vec<Box<dyn RulePlugin>> {
    builtin_rules(None)
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let all_rules = builtin_rules_all();
        let all_ids: Vec<_> = all_rules
            .iter()
            .flat_map(|r| r.get_descriptors())
            .map(|d| d.id)
            .collect();
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
