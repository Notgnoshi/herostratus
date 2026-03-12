use std::path::Path;

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
