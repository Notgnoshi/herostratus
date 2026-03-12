/// What a rule returns to indicate "grant this achievement to this person."
///
/// Contains only what the engine needs to record the grant. The achievement identity comes from the
/// rule's [Meta](super::Meta).
///
/// The person who earns an achievement is not always the commit author -- they could be the
/// committer or some other role. The fields are named `user_name` / `user_email` to reflect this.
/// (Contrast with [CommitContext](crate::observer::CommitContext), which keeps `author_name` /
/// `author_email` since it genuinely represents the git author.)
#[derive(Debug, Clone)]
pub struct Grant {
    pub commit: gix::ObjectId,
    pub user_name: String,
    pub user_email: String,
}
