//! The API that defines an achievement, and its parsing rules
#[allow(clippy::module_inception)]
mod achievement;
mod rule;
#[cfg(test)]
mod test_rule;

pub use achievement::Achievement;
pub use rule::{process_rules, Rule};
