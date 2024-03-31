#[derive(Debug)]
pub struct Achievement {
    pub name: &'static str,
    // TODO: Add the user (how to accommodate mailmaps?)
    // TODO: Add the commit hash
    // TODO: Identify the repository somehow
}

// TODO: How could user-contrib rule _scripts_ work? Consume commits via stdin, emit achievement
// JSON on stdout?
pub trait Rule {
    // TODO: Add an ID
    // TODO: Add a description

    /// Return the name of the [Achievement] that this rule generates
    ///
    /// There is expected to be a 1-1 correspondence between [Achievement]s and [Rule]s.
    fn name(&self) -> &'static str;

    /// Process the given [git2::Commit] to generate an [Achievement]
    ///
    /// Notice that this method takes `&mut self`. This is to allow the `Rule` to accumulate state
    /// during commit processing. At the end of processing, [finalize](Self::finalize) will be
    /// called, to generate any achievements from the accumulated state.
    fn process(&mut self, commit: &git2::Commit, repo: &git2::Repository) -> Option<Achievement>;

    /// Called when finished processing all commits
    // TODO: There may need to be some lifetimes added / specified, so that implementors of the
    // Rule can store the commit references? But I actually think storing the commits themselves
    // isn't quite so useful?
    fn finalize(&mut self, _repo: &git2::Repository) -> Vec<Achievement> {
        Vec::new()
    }
}
