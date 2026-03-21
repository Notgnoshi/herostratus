use chrono::{DateTime, Utc};

/// Per-commit metadata that pairs with observations flowing through the channel.
///
/// Rules see `CommitContext` + `Observation` -- they never touch the raw `gix::Commit`. Mailmap
/// resolution happens once in the ObserverEngine before constructing this struct.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommitContext {
    pub oid: gix::ObjectId,
    pub author_name: String,
    pub author_email: String,
    /// The committer timestamp from the git commit.
    pub commit_timestamp: DateTime<Utc>,
}

#[cfg(test)]
impl CommitContext {
    /// Create a test CommitContext with a null OID and an email derived from the name.
    ///
    /// The email is `{lowercase_name}@example.com`. The commit timestamp is the Unix epoch.
    pub fn test(name: &str) -> Self {
        Self {
            oid: gix::ObjectId::null(gix::hash::Kind::Sha1),
            author_name: name.to_string(),
            author_email: format!("{}@example.com", name.to_lowercase()),
            commit_timestamp: DateTime::UNIX_EPOCH,
        }
    }
}
