//! The achievements builtin to Herostratus
mod h001_fixup;

use crate::achievement::{DefaultRule, OpinionatedRule, Rule};

pub fn default_rules() -> Vec<&'static dyn Rule<DefaultRule>> {
    let mut rules = Vec::new();
    for rule in inventory::iter::<&'static dyn Rule<DefaultRule>> {
        rules.push(*rule);
    }

    // TODO: Sort rules by ID (inventory gives them in ¯\_(ツ)_/¯ order)

    rules
}

pub fn opinionated_rules() -> Vec<&'static dyn Rule<OpinionatedRule>> {
    let mut rules = Vec::new();
    for rule in inventory::iter::<&'static dyn Rule<OpinionatedRule>> {
        rules.push(*rule);
    }

    // TODO: Sort rules by ID (inventory gives them in ¯\_(ツ)_/¯ order)

    rules
}
