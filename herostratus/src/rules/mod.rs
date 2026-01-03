//! The achievements builtin to Herostratus
mod h001_fixup;
mod h002_h003_subject_line;
mod h004_non_unicode;
mod h005_empty_commit;
mod h006_whitespace_only;
mod rule_builtins;
#[cfg(test)]
pub(crate) mod test_rules;

pub use h002_h003_subject_line::{H002Config, H003Config};
pub use rule_builtins::{builtin_rules, builtin_rules_all};

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
}
