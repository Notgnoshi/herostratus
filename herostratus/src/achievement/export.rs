use std::path::Path;

use chrono::Utc;

use crate::achievement::meta::AchievementKind;
use crate::rules::RulePlugin;

/// A row in the achievement catalog CSV.
#[derive(serde::Serialize)]
struct AchievementRow {
    id: usize,
    human_id: &'static str,
    name: &'static str,
    description: &'static str,
    kind: String,
}

fn kind_label(kind: &AchievementKind) -> String {
    match kind {
        AchievementKind::PerUser { recurrent: false } => "per-user".to_string(),
        AchievementKind::PerUser { recurrent: true } => "per-user-repeat".to_string(),
        AchievementKind::Global { revocable: false } => "global".to_string(),
        AchievementKind::Global { revocable: true } => "global-revocable".to_string(),
    }
}

/// Write the achievement catalog CSV to `{data_dir}/export/achievements.csv`.
///
/// This is rewritten on every run from the compiled-in [Meta](super::Meta) data. It changes only
/// when Herostratus is updated with new rules or when the enabled rule set changes.
pub fn write_achievements_csv(data_dir: &Path, rules: &[Box<dyn RulePlugin>]) -> eyre::Result<()> {
    let path = data_dir.join("export").join("achievements.csv");
    if let Some(parent) = path.parent()
        && !parent.exists()
    {
        std::fs::create_dir_all(parent)?;
    }

    let mut rows: Vec<_> = rules
        .iter()
        .map(|rule| {
            let meta = rule.meta();
            AchievementRow {
                id: meta.id,
                human_id: meta.human_id,
                name: meta.name,
                description: meta.description,
                kind: kind_label(&meta.kind),
            }
        })
        .collect();
    rows.sort_by_key(|r| r.id);

    let mut writer = csv::Writer::from_path(&path)?;
    for row in &rows {
        writer.serialize(row)?;
    }
    writer.flush()?;

    tracing::debug!(
        "Wrote achievement catalog ({} rules) to {path:?}",
        rules.len()
    );
    Ok(())
}

/// A row in the repositories CSV.
#[derive(serde::Serialize, serde::Deserialize)]
struct RepositoryRow {
    name: String,
    url: String,
    commit_url_prefix: String,
    #[serde(rename = "ref")]
    reference: String,
    commits_checked: u64,
    last_checked: String,
}

/// Upsert a repository row in `{data_dir}/export/repositories.csv`.
///
/// If a row with the given name already exists, it is updated in place. Otherwise a new row is
/// appended. The file is rewritten on every call.
pub fn upsert_repository_csv(
    data_dir: &Path,
    name: &str,
    url: &str,
    commit_url_prefix: Option<&str>,
    reference: &str,
    commits_checked: u64,
) -> eyre::Result<()> {
    let path = data_dir.join("export").join("repositories.csv");
    if let Some(parent) = path.parent()
        && !parent.exists()
    {
        std::fs::create_dir_all(parent)?;
    }

    let mut rows: Vec<RepositoryRow> = if path.exists() {
        let mut reader = csv::Reader::from_path(&path)?;
        reader.deserialize().collect::<Result<_, _>>()?
    } else {
        Vec::new()
    };

    let new_row = RepositoryRow {
        name: name.to_string(),
        url: url.to_string(),
        commit_url_prefix: commit_url_prefix.unwrap_or("").to_string(),
        reference: reference.to_string(),
        commits_checked,
        last_checked: Utc::now().to_rfc3339(),
    };

    if let Some(existing) = rows.iter_mut().find(|r| r.name == name) {
        let total = existing.commits_checked + commits_checked;
        *existing = new_row;
        existing.commits_checked = total;
    } else {
        rows.push(new_row);
    }

    let mut writer = csv::Writer::from_path(&path)?;
    for row in &rows {
        writer.serialize(row)?;
    }
    writer.flush()?;

    tracing::debug!("Upserted repository {name:?} in {path:?}");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::RulesConfig;
    use crate::rules::builtin_rules;

    #[test]
    fn writes_catalog_csv() {
        let dir = tempfile::tempdir().unwrap();
        let rules = builtin_rules(&RulesConfig::default());
        let num_rules = rules.len();

        write_achievements_csv(dir.path(), &rules).unwrap();

        let path = dir.path().join("export/achievements.csv");
        assert!(path.exists());

        let mut reader = csv::Reader::from_path(&path).unwrap();
        let rows: Vec<csv::StringRecord> = reader.records().map(|r| r.unwrap()).collect();
        assert_eq!(rows.len(), num_rules);

        // Rows are sorted by ID
        assert_eq!(&rows[0][0], "1");
        assert_eq!(&rows[0][1], "fixup");

        // Verify monotonically increasing IDs
        let ids: Vec<usize> = rows.iter().map(|r| r[0].parse().unwrap()).collect();
        for pair in ids.windows(2) {
            assert!(pair[0] < pair[1], "IDs should be sorted: {:?}", ids);
        }
    }

    #[test]
    fn upserts_repository_row() {
        let dir = tempfile::tempdir().unwrap();

        // First insert
        upsert_repository_csv(
            dir.path(),
            "repo-a",
            "https://example.com/a.git",
            None,
            "HEAD",
            10,
        )
        .unwrap();

        // Update same name
        upsert_repository_csv(
            dir.path(),
            "repo-a",
            "https://example.com/a.git",
            Some("https://example.com/a/commit/"),
            "main",
            20,
        )
        .unwrap();

        // Insert different name
        upsert_repository_csv(
            dir.path(),
            "repo-b",
            "https://example.com/b.git",
            None,
            "HEAD",
            5,
        )
        .unwrap();

        let path = dir.path().join("export/repositories.csv");
        let mut reader = csv::Reader::from_path(&path).unwrap();
        let rows: Vec<RepositoryRow> = reader.deserialize().map(|r| r.unwrap()).collect();

        assert_eq!(rows.len(), 2);

        assert_eq!(rows[0].name, "repo-a");
        assert_eq!(rows[0].commit_url_prefix, "https://example.com/a/commit/");
        assert_eq!(rows[0].reference, "main");
        assert_eq!(rows[0].commits_checked, 30);

        assert_eq!(rows[1].name, "repo-b");
        assert_eq!(rows[1].commits_checked, 5);
    }

    #[test]
    fn repository_csv_empty_commit_url_prefix() {
        let dir = tempfile::tempdir().unwrap();

        upsert_repository_csv(dir.path(), "local", "file:///tmp/repo", None, "HEAD", 1).unwrap();

        let path = dir.path().join("export/repositories.csv");
        let mut reader = csv::Reader::from_path(&path).unwrap();
        let rows: Vec<RepositoryRow> = reader.deserialize().map(|r| r.unwrap()).collect();

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].commit_url_prefix, "");
    }

    #[test]
    fn kind_labels() {
        assert_eq!(
            kind_label(&AchievementKind::PerUser { recurrent: false }),
            "per-user"
        );
        assert_eq!(
            kind_label(&AchievementKind::PerUser { recurrent: true }),
            "per-user-repeat"
        );
        assert_eq!(
            kind_label(&AchievementKind::Global { revocable: false }),
            "global"
        );
        assert_eq!(
            kind_label(&AchievementKind::Global { revocable: true }),
            "global-revocable"
        );
    }
}
