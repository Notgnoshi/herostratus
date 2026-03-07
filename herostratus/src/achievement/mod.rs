//! The API that defines an achievement, and its parsing rules

mod achievement_log;
mod grant;
mod meta;
mod meta_achievements;
mod pipeline;
mod pipeline_checkpoint;

pub use grant::Grant;
pub use meta::{AchievementKind, Meta};
pub use pipeline::{GrantStats, grant};

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

/// An achievement event emitted by the pipeline.
#[derive(Debug)]
pub enum AchievementEvent {
    /// A new achievement was granted.
    Grant(Achievement),
    /// A previously granted achievement was revoked (for
    /// [Global { revocable: true }](AchievementKind::Global) achievements).
    Revoke(Achievement),
}
