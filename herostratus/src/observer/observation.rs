use std::mem::{Discriminant, discriminant};

/// An ephemeral, typed, per-commit fact emitted by an observer and consumed by rules.
///
/// Observations carry only the extracted fact. Commit metadata (oid, author) is carried separately
/// by [CommitContext](super::CommitContext).
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Observation {
    /// The commit subject line starts with a fixup/squash/amend/WIP/TODO/FIXME/DROPME prefix.
    Fixup,

    /// The length (in bytes) of the commit's subject line.
    SubjectLength { length: usize },

    /// The raw commit message contains bytes that are not valid UTF-8.
    NonUnicodeMessage,

    /// The commit introduces no file changes (empty tree diff). Merge commits are excluded.
    EmptyCommit,

    /// Every file change in the commit is a whitespace-only modification.
    WhitespaceOnly,

    /// The commit message contains profanity. Carries the matched word (lowercased).
    Profanity { word: String },

    /// Test-only variant for use in unit tests.
    #[cfg(test)]
    Dummy,
}

impl Observation {
    pub const FIXUP: Discriminant<Self> = discriminant(&Observation::Fixup);
    pub const SUBJECT_LENGTH: Discriminant<Self> =
        discriminant(&Observation::SubjectLength { length: 0 });
    pub const NON_UNICODE_MESSAGE: Discriminant<Self> =
        discriminant(&Observation::NonUnicodeMessage);
    pub const EMPTY_COMMIT: Discriminant<Self> = discriminant(&Observation::EmptyCommit);
    pub const WHITESPACE_ONLY: Discriminant<Self> = discriminant(&Observation::WhitespaceOnly);
    pub const PROFANITY: Discriminant<Self> = {
        let obs = Observation::Profanity {
            word: String::new(),
        };
        let d = discriminant(&obs);
        // we aren't allowed to call Drop in a const context, so leak the observation ...
        std::mem::forget(obs);
        d
    };

    #[cfg(test)]
    pub const DUMMY: Discriminant<Self> = discriminant(&Observation::Dummy);
}
