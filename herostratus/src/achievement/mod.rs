//! The API that defines an achievement, and its parsing rules
#[allow(clippy::module_inception)]
mod achievement;
mod process_rules;

// Rule isn't object-safe and is only used by its implementor; the outside world interacts with it
// through Box<dyn RulePlugin> trait objects
//
// TODO: Refactor modules so that Rule is only visible to rule implementations, and not
// process_rules.
pub(super) use achievement::Rule;
pub use achievement::{Achievement, AchievementDescriptor, RuleFactory, RulePlugin};
pub use process_rules::{grant, grant_with_rules};
