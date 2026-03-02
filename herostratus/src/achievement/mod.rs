//! The API that defines an achievement, and its parsing rules

// Old infrastructure (to be removed after migration)
mod achievement_old;
pub(crate) mod checkpoint_strategy_old;
pub(crate) mod engine_old;
mod pipeline_old;

pub use achievement_old::{Achievement, AchievementDescriptor};
pub use pipeline_old::{GrantStats, grant, grant_with_rules};

// New infrastructure (observer/rule split)
mod achievement_log;
mod checkpoint_strategy;
mod grant;
mod meta;
mod pipeline;
