#[derive(Debug)]
pub struct Achievement {
    pub name: &'static str,
    // TODO: Should this be the git2::Commit instead of the Oid? That'd enable easier serialization
    // of the actual commit message and author details, but it'd also introduce an awkward
    // lifetime.
    pub commit: git2::Oid,
    // TODO: Add the user (how to accommodate mailmaps?)
    // TODO: Identify the repository somehow
}

/// Defines a [Rule] to grant [Achievement]s
// TODO: How could user-contrib rule _scripts_ work? Consume commits via stdin, emit achievement
// JSON on stdout?
pub trait Rule {
    /// The numeric ID of this [Rule]
    ///
    /// Must be unique per-rule. Either the [id], [human_id], or [pretty_id] may be used to
    /// identify a [Rule].
    fn id(&self) -> usize;

    /// The human ID of this [Rule]
    ///
    /// Example: `longest-commit-subject-line`
    ///
    /// Must be unique per-rule. Either the [id], [human_id], or [pretty_id] may be used to
    /// identify a [Rule].
    fn human_id(&self) -> &'static str;

    /// The pretty ID of this [Rule]
    ///
    /// Concatenates the numeric [id] and the human-meaningful [human_id].
    ///
    /// Example: `H42-whats-the-question`
    ///
    /// Must be unique per-rule. Either the [id], [human_id], or [pretty_id] may be used to
    /// identify a [Rule].
    fn pretty_id(&self) -> String {
        format!("H{}-{}", self.id(), self.human_id())
    }

    /// Return the name of the [Achievement] that this rule generates
    ///
    /// The name should generally be humorous, even if the [description] isn't.
    ///
    /// There is expected to be a 1-1 correspondence between [Achievement]s and [Rule]s.
    fn name(&self) -> &'static str;

    /// A short flavor text describing what this [Rule] is all about
    ///
    /// Imagine the short one-sentence descriptions of Steam achievements.
    ///
    /// Examples:
    /// * Use a swear word
    /// * Use the most swear words
    /// * The shortest subject line
    fn description(&self) -> &'static str;

    /// Grant the given [git2::Commit] this rule's [Achievement]
    fn grant(&self, commit: &git2::Commit, _repo: &git2::Repository) -> Achievement {
        Achievement {
            name: self.name(),
            commit: commit.id(),
        }
    }

    /// Process the given [git2::Commit] to generate an [Achievement]
    ///
    /// Notice that this method takes `&mut self`. This is to allow the `Rule` to accumulate state
    /// during commit processing. At the end of processing, [finalize](Self::finalize) will be
    /// called, to generate any achievements from the accumulated state.
    fn process(&mut self, commit: &git2::Commit, repo: &git2::Repository) -> Option<Achievement>;

    /// Called when finished processing all commits
    ///
    /// This exists to enable rules that accumulate state (like calculating the shortest commit
    /// message) to generate achievements once all commits have been visited.
    fn finalize(&mut self, _repo: &git2::Repository) -> Vec<Achievement> {
        Vec::new()
    }
}
