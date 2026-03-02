/// Controls whether the observer engine continues sending diff changes to an observer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiffAction {
    /// Continue sending diff changes.
    Continue,
    /// Stop sending diff changes for this commit.
    Cancel,
}
