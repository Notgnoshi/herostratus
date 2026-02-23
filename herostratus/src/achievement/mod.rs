//! The API that defines an achievement, and its parsing rules
#[allow(clippy::module_inception)]
mod achievement;
pub(crate) mod checkpoint_strategy;
pub(crate) mod engine;
mod pipeline;

pub use achievement::{Achievement, AchievementDescriptor};
pub use pipeline::{GrantStats, grant, grant_with_rules};
