//! The achievements builtin to Herostratus

mod impls;
mod rule;
mod rule_engine;
mod rule_plugin;
#[cfg(test)]
mod test_rules;

pub use impls::{H002Config, H003Config, H012Config};
pub use rule_engine::{RuleEngine, RuleOutput};
pub use rule_plugin::{RulePlugin, builtin_rules, builtin_rules_all};
