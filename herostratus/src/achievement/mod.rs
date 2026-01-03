//! The API that defines an achievement, and its parsing rules
#[allow(clippy::module_inception)]
mod achievement;
mod process_rules;

pub use achievement::{Achievement, AchievementDescriptor};
pub use process_rules::{grant, grant_with_rules};
