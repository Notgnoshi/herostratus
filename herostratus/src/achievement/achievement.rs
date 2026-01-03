#[derive(Debug)]
pub struct Achievement {
    pub name: &'static str,
    pub commit: gix::ObjectId,
    // TODO: Add the user (how to accommodate mailmaps?)
    // TODO: Identify the repository somehow
}

/// Describes an [Achievement] that a [RulePlugin](crate::rules::RulePlugin) can grant
#[derive(Clone, Debug)]
pub struct AchievementDescriptor {
    /// Whether the [RulePlugin](crate::rules::RulePlugin) this descriptor belongs to will grant
    /// achievements described by this descriptor
    pub enabled: bool,

    /// The numeric ID of this [Achievement]
    ///
    /// Must be unique per-rule. Either the [id](Self::id), [human_id](Self::human_id), or
    /// [pretty_id](Self::pretty_id) may be used to identify an [Achievement].
    pub id: usize,

    /// The human ID of this [Achievement]
    ///
    /// Example: `longest-commit-subject-line`
    ///
    /// Must be unique per-rule. Either the [id](Self::id), [human_id](Self::human_id), or
    /// [pretty_id](Self::pretty_id) may be used to identify an [Achievement].
    pub human_id: &'static str,

    /// The name of the [Achievement] that this rule generates
    ///
    /// The name should generally be humorous, even if the [description](Self::description) isn't.
    pub name: &'static str,

    /// A short flavor text describing what this [Achievement] is all about
    ///
    /// Imagine the short one-sentence descriptions of Steam achievements.
    ///
    /// Examples:
    /// * Use a swear word
    /// * Use the most swear words
    /// * The shortest subject line
    pub description: &'static str,
}

impl AchievementDescriptor {
    /// Determine if the given ID matches this [AchievementDescriptor]
    pub fn id_matches(&self, id: &str) -> bool {
        id == self.id.to_string()
            || id == format!("H{}", self.id)
            || id == self.human_id
            || id == self.pretty_id()
    }

    /// The pretty ID of the [Achievement]s that this [AchievementDescriptor] describes.
    ///
    /// Concatenates the numeric [id](Self::id) and the human-meaningful [human_id](Self::id).
    ///
    /// Example: `H42-whats-the-question`
    ///
    /// Must be unique per-rule. Either the [id](Self::id), [human_id](Self::human_id), or
    /// [pretty_id](Self::pretty_id) may be used to identify an [Achievement].
    pub fn pretty_id(&self) -> String {
        format!("H{}-{}", self.id, self.human_id)
    }
}
