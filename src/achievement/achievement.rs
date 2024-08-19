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
    // TODO: Add an ID
    // TODO: Add a description

    /// Return the name of the [Achievement] that this rule generates
    ///
    /// There is expected to be a 1-1 correspondence between [Achievement]s and [Rule]s.
    fn name(&self) -> &'static str;

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
