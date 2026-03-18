use std::collections::HashMap;
use std::path::Path;

pub use crate::achievement::{AchievementLogEvent, AchievementRow, RepositoryRow};

pub fn load_achievements(export_dir: &Path) -> eyre::Result<Vec<AchievementRow>> {
    let path = export_dir.join("achievements.csv");
    let mut reader = csv::Reader::from_path(&path)?;
    let records: Vec<AchievementRow> = reader.deserialize().collect::<Result<_, _>>()?;
    tracing::debug!("Loaded {} achievements from {path:?}", records.len());
    Ok(records)
}

pub fn load_repositories(export_dir: &Path) -> eyre::Result<Vec<RepositoryRow>> {
    let path = export_dir.join("repositories.csv");
    let mut reader = csv::Reader::from_path(&path)?;
    let records: Vec<RepositoryRow> = reader.deserialize().collect::<Result<_, _>>()?;
    tracing::debug!("Loaded {} repositories from {path:?}", records.len());
    Ok(records)
}

pub fn load_events(export_dir: &Path) -> eyre::Result<HashMap<String, Vec<AchievementLogEvent>>> {
    let events_dir = export_dir.join("events");
    let mut all_events = HashMap::new();

    if !events_dir.exists() {
        tracing::debug!("No events directory at {events_dir:?}");
        return Ok(all_events);
    }

    for entry in std::fs::read_dir(&events_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().is_some_and(|ext| ext == "csv") {
            let repo_name = path
                .file_stem()
                .expect("file has extension so it has a stem")
                .to_string_lossy()
                .into_owned();
            let mut reader = csv::Reader::from_path(&path)?;
            let events: Vec<AchievementLogEvent> =
                reader.deserialize().collect::<Result<_, _>>()?;
            tracing::debug!(
                "Loaded {} events for {repo_name:?} from {path:?}",
                events.len()
            );
            all_events.insert(repo_name, events);
        }
    }

    Ok(all_events)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_csv(dir: &Path, name: &str, content: &str) {
        let path = dir.join(name);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(path, content).unwrap();
    }

    #[test]
    fn load_achievements_from_csv() {
        let dir = tempfile::tempdir().unwrap();
        write_csv(
            dir.path(),
            "achievements.csv",
            "id,human_id,name,description,kind\n\
             1,fixup,Leftovers,Prefix a commit with fixup,per-user\n\
             2,shortest,Brevity,The shortest subject line,global-revocable\n",
        );

        let records = load_achievements(dir.path()).unwrap();
        assert_eq!(records.len(), 2);
        assert_eq!(records[0].id, 1);
        assert_eq!(records[0].human_id, "fixup");
        assert_eq!(records[0].kind, "per-user");
        assert_eq!(records[1].id, 2);
        assert_eq!(records[1].kind, "global-revocable");
    }

    #[test]
    fn load_repositories_from_csv() {
        let dir = tempfile::tempdir().unwrap();
        write_csv(
            dir.path(),
            "repositories.csv",
            "name,url,commit_url_prefix,ref,commits_checked,last_checked\n\
             my-repo,https://example.com/repo.git,https://example.com/repo/commit/,main,100,2026-01-01T00:00:00Z\n",
        );

        let records = load_repositories(dir.path()).unwrap();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].name, "my-repo");
        assert_eq!(
            records[0].commit_url_prefix,
            "https://example.com/repo/commit/"
        );
        assert_eq!(records[0].commits_checked, 100);
    }

    #[test]
    fn load_events_from_csv() {
        let dir = tempfile::tempdir().unwrap();
        write_csv(
            dir.path(),
            "events/repo-a.csv",
            "timestamp,event,achievement_id,commit,user_name,user_email\n\
             2026-01-01T00:00:00Z,grant,fixup,0101010101010101010101010101010101010101,Alice,alice@example.com\n\
             2026-01-02T00:00:00Z,grant,fixup,0202020202020202020202020202020202020202,Bob,bob@example.com\n",
        );
        write_csv(
            dir.path(),
            "events/repo-b.csv",
            "timestamp,event,achievement_id,commit,user_name,user_email\n\
             2026-01-03T00:00:00Z,grant,shortest,0303030303030303030303030303030303030303,Alice,alice@example.com\n",
        );

        let events = load_events(dir.path()).unwrap();
        assert_eq!(events.len(), 2);
        assert_eq!(events["repo-a"].len(), 2);
        assert_eq!(events["repo-b"].len(), 1);
        assert_eq!(events["repo-a"][0].user_name, "Alice");
        assert_eq!(
            events["repo-a"][0].event,
            crate::achievement::AchievementEventKind::Grant
        );
        assert_eq!(events["repo-b"][0].achievement_id, "shortest");
    }

    #[test]
    fn load_events_missing_dir() {
        let dir = tempfile::tempdir().unwrap();
        let events = load_events(dir.path()).unwrap();
        assert!(events.is_empty());
    }
}
