//! The API that defines an achievement, and its parsing rules

mod achievement_log;
mod export;
mod grant;
mod meta;
mod meta_achievements;
mod pipeline;
mod pipeline_checkpoint;

pub use export::upsert_repository_csv;
pub use grant::Grant;
pub use meta::{AchievementKind, Meta};
pub use meta_achievements::meta_achievement_metas;
pub use pipeline::{GrantStats, grant};

#[derive(Debug)]
pub struct Achievement {
    pub descriptor_id: usize,
    pub human_id: &'static str,
    /// Resolved display name: [Grant.name_override](Grant::name_override) if set, otherwise
    /// [Meta.name](Meta::name).
    pub name: String,
    /// Resolved display description: [Grant.description_override](Grant::description_override) if
    /// set, otherwise [Meta.description](Meta::description).
    pub description: String,
    pub commit: gix::ObjectId,
    /// The mailmap-resolved user name
    pub user_name: String,
    /// The mailmap-resolved user email
    pub user_email: String,
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
