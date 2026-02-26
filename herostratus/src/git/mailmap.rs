use std::path::Path;

use eyre::WrapErr;

/// Resolves commit author identities using Git's mailmap mechanism.
///
/// Wraps a [`gix::mailmap::Snapshot`] that has been pre-loaded (typically from the repository's own
/// `.mailmap` via [`gix::Repository::open_mailmap()`]) and optionally merged with additional
/// Herostratus config mailmap files.
pub struct MailmapResolver {
    snapshot: gix::mailmap::Snapshot,
}

impl MailmapResolver {
    /// Build a MailmapResolver from a pre-loaded snapshot, plus optional global and per-repo
    /// mailmap files.
    ///
    /// The caller is responsible for loading the base snapshot (e.g. via
    /// [`gix::Repository::open_mailmap()`]). Additional mailmap files are merged in order of
    /// increasing priority:
    /// 1. The provided `snapshot` (lowest priority)
    /// 2. Global mailmap file
    /// 3. Per-repo mailmap file (highest priority)
    pub fn new(
        mut snapshot: gix::mailmap::Snapshot,
        global_mailmap: Option<&Path>,
        repo_mailmap: Option<&Path>,
    ) -> eyre::Result<Self> {
        if let Some(path) = global_mailmap {
            merge_file(&mut snapshot, path).wrap_err_with(|| {
                format!("Failed to load global mailmap file: {}", path.display())
            })?;
        }

        if let Some(path) = repo_mailmap {
            merge_file(&mut snapshot, path).wrap_err_with(|| {
                format!("Failed to load repo mailmap file: {}", path.display())
            })?;
        }

        Ok(Self { snapshot })
    }

    /// Resolve the author identity from a commit using the mailmap.
    ///
    /// If the mailmap contains a mapping for the commit's author, the resolved name and email are
    /// returned. Otherwise, the raw author signature is returned as-is.
    pub fn resolve_author(&self, commit: &gix::Commit) -> eyre::Result<gix::actor::Signature> {
        let sig = commit.author()?;
        Ok(self.snapshot.resolve(sig))
    }
}

/// Read a mailmap file from disk and merge its entries into the snapshot.
///
/// Returns an error if the file cannot be read, or if any line fails to parse.
fn merge_file(snapshot: &mut gix::mailmap::Snapshot, path: &Path) -> eyre::Result<()> {
    let contents = std::fs::read(path)
        .wrap_err_with(|| format!("Failed to read mailmap file: {}", path.display()))?;
    let entries: Vec<_> = gix::mailmap::parse(&contents)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| eyre::eyre!("{}: {e}", path.display()))?;
    snapshot.merge(entries);
    Ok(())
}

#[cfg(test)]
mod tests {
    use herostratus_tests::fixtures;

    use super::*;

    #[test]
    fn no_mailmap_passthrough() {
        let temp_repo = fixtures::repository::simplest().unwrap();
        let resolver = MailmapResolver::new(gix::mailmap::Snapshot::default(), None, None).unwrap();
        let head = temp_repo.repo.head_commit().unwrap();
        let author = resolver.resolve_author(&head).unwrap();
        assert_eq!(author.name, "Herostratus");
        assert_eq!(author.email, "Herostratus@example.com");
    }

    #[test]
    fn config_mailmap_resolves() {
        let temp_repo = fixtures::repository::simplest().unwrap();

        let mailmap_dir = tempfile::tempdir().unwrap();
        let mailmap_path = mailmap_dir.path().join("mailmap");
        std::fs::write(
            &mailmap_path,
            "Canonical Name <canonical@example.com> Herostratus <Herostratus@example.com>\n",
        )
        .unwrap();

        let resolver =
            MailmapResolver::new(gix::mailmap::Snapshot::default(), Some(&mailmap_path), None)
                .unwrap();

        let head = temp_repo.repo.head_commit().unwrap();
        let author = resolver.resolve_author(&head).unwrap();
        assert_eq!(author.name, "Canonical Name");
        assert_eq!(author.email, "canonical@example.com");
    }

    #[test]
    fn repo_mailmap_overrides_global() {
        let temp_repo = fixtures::repository::simplest().unwrap();

        let global_dir = tempfile::tempdir().unwrap();
        let global_path = global_dir.path().join("global-mailmap");
        std::fs::write(
            &global_path,
            "Global Name <global@example.com> Herostratus <Herostratus@example.com>\n",
        )
        .unwrap();

        let repo_dir = tempfile::tempdir().unwrap();
        let repo_path = repo_dir.path().join("repo-mailmap");
        std::fs::write(
            &repo_path,
            "Repo Name <repo@example.com> Herostratus <Herostratus@example.com>\n",
        )
        .unwrap();

        let resolver = MailmapResolver::new(
            gix::mailmap::Snapshot::default(),
            Some(&global_path),
            Some(&repo_path),
        )
        .unwrap();

        let head = temp_repo.repo.head_commit().unwrap();
        let author = resolver.resolve_author(&head).unwrap();
        assert_eq!(author.name, "Repo Name");
        assert_eq!(author.email, "repo@example.com");
    }

    #[test]
    fn custom_author_resolved() {
        let temp_repo = fixtures::repository::bare().unwrap();
        fixtures::repository::add_empty_commit_as(
            &temp_repo.repo,
            "test commit",
            "Old Name",
            "old@example.com",
        )
        .unwrap();

        let mailmap_dir = tempfile::tempdir().unwrap();
        let mailmap_path = mailmap_dir.path().join("mailmap");
        std::fs::write(
            &mailmap_path,
            "New Name <new@example.com> Old Name <old@example.com>\n",
        )
        .unwrap();

        let resolver =
            MailmapResolver::new(gix::mailmap::Snapshot::default(), Some(&mailmap_path), None)
                .unwrap();

        let head = temp_repo.repo.head_commit().unwrap();
        let author = resolver.resolve_author(&head).unwrap();
        assert_eq!(author.name, "New Name");
        assert_eq!(author.email, "new@example.com");
    }

    #[test]
    fn unmatched_author_passthrough() {
        let temp_repo = fixtures::repository::bare().unwrap();
        fixtures::repository::add_empty_commit_as(
            &temp_repo.repo,
            "test commit",
            "Unmapped",
            "unmapped@example.com",
        )
        .unwrap();

        // Mailmap maps a different author, not the one in the commit
        let mailmap_dir = tempfile::tempdir().unwrap();
        let mailmap_path = mailmap_dir.path().join("mailmap");
        std::fs::write(
            &mailmap_path,
            "New Name <new@example.com> Old Name <old@example.com>\n",
        )
        .unwrap();

        let resolver =
            MailmapResolver::new(gix::mailmap::Snapshot::default(), Some(&mailmap_path), None)
                .unwrap();

        let head = temp_repo.repo.head_commit().unwrap();
        let author = resolver.resolve_author(&head).unwrap();
        assert_eq!(author.name, "Unmapped");
        assert_eq!(author.email, "unmapped@example.com");
    }
}
