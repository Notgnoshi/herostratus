/// What a rule returns to indicate "grant this achievement to this commit's author."
///
/// Contains only what the engine needs to record the grant. The achievement identity comes from the
/// rule's [`Meta`](super::Meta).
#[derive(Debug, Clone)]
pub struct Grant {
    pub commit: gix::ObjectId,
    pub author_name: String,
    pub author_email: String,
}
