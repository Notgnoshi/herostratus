/// Per-commit metadata that pairs with observations flowing through the channel.
///
/// Rules see `CommitContext` + `Observation` -- they never touch the raw `gix::Commit`. Mailmap
/// resolution happens once in the ObserverEngine before constructing this struct.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommitContext {
    pub oid: gix::ObjectId,
    pub author_name: String,
    pub author_email: String,
}
