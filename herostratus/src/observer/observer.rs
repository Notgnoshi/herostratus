use super::observation::Observation;

/// Extracts facts from commits and emits typed [Observation]s.
///
/// Observers are intended to be stateless across commits. Mutable methods allow for transient
/// per-commit state. Persisting state across multiple commits is possible, but should be
/// discouraged; that's more appropriately handled by rules that consume [Observation]s from
/// multiple commits.
///
/// # Observation Lifecycle
///
/// For each commit, the engine calls observer methods in the following order:
///
/// 1. [on_commit](Self::on_commit) -- called for every commit
/// 2. If [is_interested_in_diff](Self::is_interested_in_diff) returns true:
///    1. [on_diff_start](Self::on_diff_start)
///    2. [on_diff_change](Self::on_diff_change) for each change in the diff
///    3. [on_diff_end](Self::on_diff_end) -- always called, regardless of errors or
///       [DiffAction::Cancel]
///
/// For message-only observers, [on_commit](Self::on_commit) is the sole emission point. For diff
/// observers, [on_commit](Self::on_commit) provides access to commit metadata before the diff
/// lifecycle (e.g., to check parent count and set skip flags), and
/// [on_diff_end](Self::on_diff_end) is the emission point.
///
/// # Implementing a new [Observer]
///
/// Implementing a new observer requires a few distinct steps:
///
/// 1. Define a new [Observation] variant for the observer to emit
/// 2. Implement the [Observer] trait in the [impls](super::impls) module
/// 3. Register the observer via [inventory::submit!] with an
///    [ObserverFactory](super::ObserverFactory)
pub trait Observer {
    /// The observation variant this observer emits.
    fn emits(&self) -> std::mem::Discriminant<Observation>;

    /// Whether this observer needs the computed diff. Default: false.
    ///
    /// Used by the engine to skip diff computation when no observer needs it.
    fn is_interested_in_diff(&self) -> bool {
        false
    }

    /// Called for every commit. Returns zero or one observations.
    fn on_commit(
        &mut self,
        commit: &gix::Commit,
        repo: &gix::Repository,
    ) -> eyre::Result<Option<Observation>>;

    /// Called once before diff hunks for a commit.
    fn on_diff_start(&mut self) -> eyre::Result<()> {
        Ok(())
    }

    /// Called for each file-level change in the diff.
    ///
    /// Return [DiffAction::Cancel] to stop receiving further changes for this commit.
    fn on_diff_change(
        &mut self,
        _change: &gix::object::tree::diff::Change,
        _repo: &gix::Repository,
    ) -> eyre::Result<DiffAction> {
        Ok(DiffAction::Cancel)
    }

    /// Called once after all diff changes, regardless of errors or [DiffAction::Cancel].
    ///
    /// Returns zero or one observations summarizing the diff.
    fn on_diff_end(&mut self) -> eyre::Result<Option<Observation>> {
        Ok(None)
    }
}

/// Controls whether the observer engine continues sending diff changes to an observer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiffAction {
    /// Continue sending diff changes.
    Continue,
    /// Stop sending diff changes for this commit.
    Cancel,
}
