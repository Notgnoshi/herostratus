//! The API that defines an achievement, and its parsing rules
#[allow(clippy::module_inception)]
mod achievement;
mod process_rules;
#[cfg(test)]
mod test_process_rules;

pub use achievement::{Achievement, Rule};
pub use process_rules::process_rules;
