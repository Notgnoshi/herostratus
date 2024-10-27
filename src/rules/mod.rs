//! The achievements builtin to Herostratus
mod h001_fixup;
mod h002_shortest_subject_line;
mod h003_longest_subject_line;

use crate::achievement::{Rule, RuleFactory};

/// Get a new instance of each builtin [Rule]
pub fn builtin_rules() -> Vec<Box<dyn Rule>> {
    // Each Rule uses inventory::submit! to register a factory to build themselves with.
    inventory::iter::<RuleFactory>
        .into_iter()
        .map(|factory| factory.build())
        .collect()
}
