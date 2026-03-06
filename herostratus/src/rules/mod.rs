//! The achievements builtin to Herostratus

// Old infrastructure (to be removed after migration)
mod h001_fixup;
mod h002_h003_subject_line;
mod h004_non_unicode;
mod h005_empty_commit;
mod h006_whitespace_only;
mod rule_builtins_old;
mod rule_old;
mod rule_plugin_old;
#[cfg(test)]
pub(crate) mod test_rules_old;

pub use h002_h003_subject_line::{H002Config, H003Config};
pub use rule_builtins_old::{builtin_rules, builtin_rules_all};
// Rule isn't object-safe and is only used by its implementor; the outside world interacts with it
// through Box<dyn RulePlugin> trait objects
pub(in crate::rules) use rule_old::Rule;
pub(in crate::rules) use rule_plugin_old::RuleFactory;
pub use rule_plugin_old::RulePlugin;

// New infrastructure (observer/rule split)
mod impls;
mod rule;
mod rule_engine;
pub(crate) mod rule_plugin;
#[cfg(test)]
pub(crate) mod test_rules;

pub(crate) use rule_engine::{RuleEngine, RuleOutput};
