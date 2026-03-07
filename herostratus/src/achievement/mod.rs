//! The API that defines an achievement, and its parsing rules

mod achievement_log;
mod grant;
mod meta;
mod pipeline;
pub(crate) mod pipeline_checkpoint;

pub use grant::Grant;
pub use meta::{AchievementKind, Meta};
pub use pipeline::{GrantStats, grant, grant_with_rules};

#[derive(Debug)]
pub struct Achievement {
    pub descriptor_id: usize,
    pub name: &'static str,
    pub commit: gix::ObjectId,
    /// The mailmap-resolved author name
    pub author_name: String,
    /// The mailmap-resolved author email
    pub author_email: String,
}
