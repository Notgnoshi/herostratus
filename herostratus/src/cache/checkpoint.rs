use std::str::FromStr;

use serde::Deserialize;

use crate::cache::utils::JsonFileCache;

/// A [Checkpoint] identifies a commit and what [RulePlugin](crate::rules::RulePlugin)s have been
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

    /// The `(rule_id, rule_version)` pairs of rules that were processed on all commits up to
    /// [Checkpoint::commit].
    ///
    /// Deserialize accepts two shapes:
    /// - `[1, 2, 3]` - just rule IDs, with an implicit rule version of 1
    /// - `[[1, 1], [2, 2]]` - `(rule_id, rule_version)` pairs
    ///
    /// Serialize always writes the tupled shape.
    #[serde(
        serialize_with = "serialize_rules",
        deserialize_with = "deserialize_rules"
    )]
    pub rules: Vec<(usize, u32)>,
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

fn serialize_object_id<S>(
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

fn deserialize_object_id<'de, D>(deserializer: D) -> Result<Option<gix::ObjectId>, D::Error>
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

fn serialize_rules<S>(rules: &[(usize, u32)], serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    use serde::ser::SerializeSeq;
    let mut seq = serializer.serialize_seq(Some(rules.len()))?;
    for pair in rules {
        seq.serialize_element(&[pair.0 as u64, pair.1 as u64])?;
    }
    seq.end()
}

fn deserialize_rules<'de, D>(deserializer: D) -> Result<Vec<(usize, u32)>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Entry {
        BareId(usize),
        Pair([u64; 2]),
    }

    let entries: Vec<Entry> = Vec::deserialize(deserializer)?;
    Ok(entries
        .into_iter()
        .map(|e| match e {
            Entry::BareId(id) => (id, 1u32),
            Entry::Pair([id, version]) => (id as usize, version as u32),
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserialize_accepts_v1_format_bare_ids() {
        let json = r#"{"commit":null,"rules":[1,2,3]}"#;
        let cp: Checkpoint = serde_json::from_str(json).unwrap();
        assert_eq!(cp.rules, vec![(1, 1), (2, 1), (3, 1)]);
    }

    #[test]
    fn deserialize_accepts_new_tupled_format() {
        let json = r#"{"commit":null,"rules":[[1,1],[2,2],[3,1]]}"#;
        let cp: Checkpoint = serde_json::from_str(json).unwrap();
        assert_eq!(cp.rules, vec![(1, 1), (2, 2), (3, 1)]);
    }

    #[test]
    fn serialize_always_writes_tupled_format() {
        let cp = Checkpoint {
            commit: None,
            rules: vec![(1, 1), (2, 3)],
        };
        let json = serde_json::to_string(&cp).unwrap();
        assert_eq!(json, r#"{"commit":null,"rules":[[1,1],[2,3]]}"#);
    }
}
