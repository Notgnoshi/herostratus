use super::grant::Grant;
use crate::observer::CommitContext;

/// Static metadata about an achievement. One per rule.
#[derive(Debug, Clone)]
pub struct Meta {
    /// Numeric ID (e.g., 1 for H001).
    pub id: usize,

    /// Stable string identifier (e.g., "fixup", "shortest-subject-line").
    /// Used in the achievement log, checkpoint, and configuration.
    pub human_id: &'static str,

    /// Display name (e.g., "Leftovers").
    pub name: &'static str,

    /// Short flavor text (e.g., "Leave a fixup! commit in your history").
    pub description: &'static str,

    /// Variation semantics -- how the engine enforces this achievement.
    pub kind: AchievementKind,
}

impl Meta {
    /// Construct a [`Grant`] from a [`CommitContext`].
    pub fn grant(&self, ctx: &CommitContext) -> Grant {
        Grant {
            commit: ctx.oid,
            author_name: ctx.author_name.clone(),
            author_email: ctx.author_email.clone(),
        }
    }

    /// Check if a user-provided string matches this achievement.
    /// Accepts: "1", "H1", "fixup", "H1-fixup".
    pub fn id_matches(&self, id: &str) -> bool {
        id == self.id.to_string()
            || id == format!("H{}", self.id)
            || id == self.human_id
            || id == format!("H{}-{}", self.id, self.human_id)
    }
}

/// How the engine enforces an achievement's variation semantics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AchievementKind {
    /// Each user can hold independently.
    PerUser {
        /// false: at most once per user ("Have you ever done X?")
        /// true: multiple grants per user at rule-defined thresholds
        recurrent: bool,
    },

    /// One holder globally.
    Global {
        /// false: once granted, permanent ("First person to do X")
        /// true: new winner supersedes previous holder ("Best at X")
        revocable: bool,
    },
}
