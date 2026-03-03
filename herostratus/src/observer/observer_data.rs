use super::commit_context::CommitContext;
use super::observation::Observation;

/// Messages sent through the channel from the ObserverEngine to the RuleEngine.
#[derive(Debug, PartialEq, Eq)]
pub enum ObserverData {
    /// Begins a new commit. Sent once before any observations for that commit.
    CommitStart(CommitContext),

    /// A single observation extracted from the current commit.
    Observation(Observation),

    /// All observations for the current commit have been sent.
    CommitComplete,
}
