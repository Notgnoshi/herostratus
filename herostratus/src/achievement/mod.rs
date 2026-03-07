//! The API that defines an achievement, and its parsing rules

// Old infrastructure (to be removed after migration)
mod achievement_old;
pub(crate) mod checkpoint_strategy_old;
pub(crate) mod engine_old;
mod pipeline_old;

pub use achievement_old::{Achievement, AchievementDescriptor};
pub use pipeline::{GrantStats, grant, grant_with_rules};
// Temporary re-export for old tests that still use the old RulePlugin trait.
// Remove in Stage 3 when the old modules are deleted.
#[cfg(test)]
pub(crate) use pipeline_old::grant_with_rules as grant_with_rules_old;

// New infrastructure (observer/rule split)
mod achievement_log;
mod grant;
mod meta;
mod pipeline;
pub(crate) mod pipeline_checkpoint;

pub use grant::Grant;
pub use meta::{AchievementKind, Meta};
