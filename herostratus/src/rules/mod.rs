//! The achievements builtin to Herostratus
mod h001_fixup;
mod h002_h003_subject_line;
mod h004_non_unicode;
mod h005_empty_commit;
mod h006_whitespace_only;
mod rule;
mod rule_builtins;
mod rule_plugin;
#[cfg(test)]
pub(crate) mod test_rules;

pub use h002_h003_subject_line::{H002Config, H003Config};
// Rule isn't object-safe and is only used by its implementor; the outside world interacts with it
// through Box<dyn RulePlugin> trait objects
pub(in crate::rules) use rule::Rule;
pub use rule_builtins::{builtin_rules, builtin_rules_all};
pub(in crate::rules) use rule_plugin::RuleFactory;
pub use rule_plugin::RulePlugin;
