//! The API that defines an achievement, and its parsing rules
#[allow(clippy::module_inception)]
mod achievement;
pub(crate) mod checkpoint_strategy;
pub(crate) mod engine;
mod process_rules;

pub use achievement::{Achievement, AchievementDescriptor};
pub use process_rules::{grant, grant_with_rules};
