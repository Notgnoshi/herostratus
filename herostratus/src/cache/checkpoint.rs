use std::str::FromStr;

use serde::Deserialize;

use crate::cache::utils::JsonFileCache;

/// A [Checkpoint] identifies a commit and what [Rule](crate::achievement::Rule)s have been
/// processed on all ancestors up to and including that commit.
///
/// This cache is provided to enable avoiding re-processing commits that have already been
/// processed.
#[derive(Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct Checkpoint {
    /// A 40-char hexadecimal SHA1 commit hash of the last processed commit
    #[serde(
        serialize_with = "serialize_object_id",
        deserialize_with = "deserialize_object_id"
    )]
    pub commit: Option<gix::ObjectId>,

    /// The integer rule IDs of the rules that were run on the last processed commit
    pub rules: Vec<usize>,
}

/// A checkpoint for a specific repository / branch pair
///
/// Saved to `<data dir>/cache/<name>/checkpoint.json`
pub type CheckpointCache = JsonFileCache<Checkpoint>;

impl CheckpointCache {
    /// Load the checkpoint cache from the data dir for the given repository
    pub fn from_data_dir<P: AsRef<std::path::Path>>(data_dir: P, name: &str) -> eyre::Result<Self> {
        let cache_path = data_dir
            .as_ref()
            .join("cache")
            .join(name)
            .join("checkpoint.json");
        Self::load(cache_path)
    }
}

pub fn serialize_object_id<S>(
    object_id: &Option<gix::ObjectId>,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    match object_id {
        Some(oid) => serializer.serialize_str(&oid.to_string()),
        None => serializer.serialize_none(),
    }
}

pub fn deserialize_object_id<'de, D>(deserializer: D) -> Result<Option<gix::ObjectId>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let opt = Option::<String>::deserialize(deserializer)?;
    match opt {
        Some(s) => gix::ObjectId::from_str(&s)
            .map(Some)
            .map_err(serde::de::Error::custom),
        None => Ok(None),
    }
}
