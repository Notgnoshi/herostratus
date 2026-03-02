use std::collections::HashSet;

use eyre::WrapErr;

use crate::achievement::Achievement;
use crate::git::mailmap::MailmapResolver;
use crate::rules::RulePlugin;

/// The rule execution engine. Owns the rules, handles diff dispatch, and tracks commit processing.
pub(crate) struct RuleEngine<'repo> {
    repo: &'repo gix::Repository,
    rules: Vec<Box<dyn RulePlugin>>,
    mailmap: MailmapResolver,

    /// Descriptor IDs permanently disabled by config (exclude/include). Never modified after construction.
    config_disabled: HashSet<usize>,
    /// Descriptor IDs temporarily suppressed during checkpoint suppress-and-continue.
    suppressed: HashSet<usize>,

    // This cache is unbounded and needs to be reset periodically to avoid infinite memory growth.
    // Don't reset it every commit, because each commit needs to lookup itself and its parent(s).
    // But we shouldn't *never* reset it, because then we'd end up holding the whole history in
    // memory. So we reset it every N commits processed.
    diff_cache: gix::diff::blob::Platform,
    num_commits_processed: u64,
}

impl<'repo> RuleEngine<'repo> {
    pub fn new(
        repo: &'repo gix::Repository,
        rules: Vec<Box<dyn RulePlugin>>,
        config_disabled: HashSet<usize>,
        mailmap: MailmapResolver,
    ) -> eyre::Result<Self> {
        let diff_cache = repo
            .diff_resource_cache_for_tree_diff()
            .wrap_err("Failed to create diff cache")?;
        Ok(Self {
            repo,
            rules,
            mailmap,
            config_disabled,
            suppressed: HashSet::new(),
            diff_cache,
            num_commits_processed: 0,
        })
    }

    /// Apply all enabled rules to a single commit (process + diff).
    /// Filters returned achievements by `config_disabled ∪ suppressed`.
    pub fn process_commit(&mut self, oid: gix::ObjectId) -> eyre::Result<Vec<Achievement>> {
        let commit = self
            .repo
            .find_commit(oid)
            .wrap_err_with(|| format!("Failed to find commit {oid}"))?;
        self.num_commits_processed += 1;

        let mut achievements = Vec::new();
        for rule in &mut self.rules {
            achievements.extend(rule.process(&commit, self.repo));
        }

        achievements.extend(self.diff_commit(&commit)?);

        const CLEAR_CACHE_EVERY_N: u64 = 50; // SWAG: Scientific Wild Ass Guess
        if self
            .num_commits_processed
            .is_multiple_of(CLEAR_CACHE_EVERY_N)
        {
            self.diff_cache.clear_resource_cache_keep_allocation();
        }

        // Filter by config_disabled ∪ suppressed
        achievements.retain(|a| {
            !self.config_disabled.contains(&a.descriptor_id)
                && !self.suppressed.contains(&a.descriptor_id)
        });

        if !achievements.is_empty() {
            let author = self.mailmap.resolve_author(&commit)?;
            for a in &mut achievements {
                a.author_name = author.name.to_string();
                a.author_email = author.email.to_string();
            }
        }

        if achievements.len() > 1 {
            tracing::debug!(
                "Generated {} achievements for commit {}",
                achievements.len(),
                commit.id()
            );
        }

        Ok(achievements)
    }

    /// Call finalize() on all rules, returning accumulated achievements.
    /// Filters returned achievements by `config_disabled` only (suppressed rules pass through).
    pub fn finalize(&mut self) -> Vec<Achievement> {
        tracing::debug!("Finalizing rules ...");
        let mut achievements = Vec::new();
        for rule in &mut self.rules {
            achievements.extend(rule.finalize(self.repo));
        }
        achievements.retain(|a| !self.config_disabled.contains(&a.descriptor_id));
        self.resolve_authors(&mut achievements);
        achievements
    }

    /// Returns descriptor IDs not in `config_disabled` (ignores `suppressed`).
    pub fn get_enabled_rule_ids(&self) -> Vec<usize> {
        let descriptors = self.rules.iter().flat_map(|r| r.descriptors());
        descriptors
            .filter_map(|d| (!self.config_disabled.contains(&d.id)).then_some(d.id))
            .collect()
    }

    /// Add descriptor IDs to the suppressed set.
    pub fn suppress_descriptors(&mut self, ids: &[usize]) {
        for id in ids {
            tracing::debug!("Suppressing descriptor {id}");
            self.suppressed.insert(*id);
        }
    }

    /// Finalize rules where ALL descriptors are inactive (config_disabled or suppressed),
    /// then filter by config_disabled only. Returns achievements from those finalized rules.
    pub fn finalize_inactive_rules(&mut self) -> Vec<Achievement> {
        let mut achievements = Vec::new();
        let repo = self.repo;
        for rule in &mut self.rules {
            let all_inactive = rule
                .descriptors()
                .iter()
                .all(|d| self.config_disabled.contains(&d.id) || self.suppressed.contains(&d.id));
            if all_inactive {
                let names: Vec<_> = rule.descriptors().iter().map(|d| d.human_id).collect();
                let rule_name = names.join(",");
                tracing::debug!(
                    "{rule_name:?} doesn't have any new achievements to process; finalizing ..."
                );
                achievements.extend(rule.finalize(repo));
            } else {
                let names: Vec<_> = rule
                    .descriptors()
                    .iter()
                    .filter(|d| {
                        !self.config_disabled.contains(&d.id) && !self.suppressed.contains(&d.id)
                    })
                    .map(|d| d.human_id)
                    .collect();
                let rule_name = names.join(",");
                tracing::warn!(
                    "Continuing to process new rule {rule_name:?} on already-processed commits"
                );
            }
        }
        // Filter by config_disabled only (suppressed rules pass through finalization)
        achievements.retain(|a| !self.config_disabled.contains(&a.descriptor_id));
        self.resolve_authors(&mut achievements);
        achievements
    }

    /// Remove rules where ALL descriptors are inactive (config_disabled or suppressed).
    pub fn retain_active_rules(&mut self) {
        self.rules.retain(|r| {
            !r.descriptors()
                .iter()
                .all(|d| self.config_disabled.contains(&d.id) || self.suppressed.contains(&d.id))
        });
    }

    pub fn rules(&self) -> &[Box<dyn RulePlugin>] {
        &self.rules
    }

    pub fn num_commits_processed(&self) -> u64 {
        self.num_commits_processed
    }

    /// Look up each achievement's commit and resolve the author via the mailmap.
    fn resolve_authors(&self, achievements: &mut [Achievement]) {
        for a in achievements {
            match self.repo.find_commit(a.commit) {
                Ok(commit) => match self.mailmap.resolve_author(&commit) {
                    Ok(author) => {
                        a.author_name = author.name.to_string();
                        a.author_email = author.email.to_string();
                    }
                    Err(e) => {
                        tracing::warn!("Failed to resolve author for commit {}: {e}", a.commit);
                    }
                },
                Err(e) => {
                    tracing::warn!(
                        "Failed to find commit {} for author resolution: {e}",
                        a.commit
                    );
                }
            }
        }
    }

    fn diff_commit(&mut self, commit: &gix::Commit) -> eyre::Result<Vec<Achievement>> {
        // Per-commit tracking of which rules are still active for diff changes.
        // A rule starts active if interested in diffs, and becomes inactive if it
        // returns Action::Cancel from on_diff_change.
        let mut diff_active: Vec<bool> = self
            .rules
            .iter()
            .map(|r| r.is_interested_in_diffs())
            .collect();
        for (idx, rule) in self.rules.iter_mut().enumerate() {
            if diff_active[idx] {
                rule.on_diff_start(commit, self.repo);
            }
        }

        let mut parents = commit.parent_ids();
        let parent = parents.next();
        if parents.next().is_some() {
            // This is a merge commit (has multiple parents), and we want to skip it
            let mut achievements = Vec::new();
            for rule in &mut self.rules {
                if rule.is_interested_in_diffs() {
                    achievements.extend(rule.on_diff_end(commit, self.repo));
                }
            }
            return Ok(achievements);
        }

        let commit_tree = commit
            .tree()
            .wrap_err_with(|| format!("Failed to get tree for commit {}", commit.id()))?;
        let parent_tree = match parent {
            Some(pid) => {
                match self.repo.find_commit(pid) {
                    Ok(parent) => parent
                        .tree()
                        .wrap_err_with(|| format!("Failed to get tree for parent commit {pid}"))?,
                    // This could be a shallow clone where the parent commit is missing.
                    Err(_) => self.repo.empty_tree(),
                }
            }
            None => self.repo.empty_tree(),
        };

        let mut changes = parent_tree
            .changes()
            .wrap_err("Failed to create tree changes iterator")?;
        changes.options(|o| {
            o.track_rewrites(None);
        });

        // Use partial borrows so the closure can capture individual fields instead of &mut self
        let rules = &mut self.rules;
        let repo = self.repo;
        let diff_cache = &mut self.diff_cache;

        let outcome =
            changes.for_each_to_obtain_tree_with_cache(&commit_tree, diff_cache, |change| {
                // Can only cancel the top-level diff processing if all rules agree to cancel.
                // But we want to stop feeding changes into Rules that have already cancelled.
                let mut all_disinterested = true;
                for (idx, rule) in rules.iter_mut().enumerate() {
                    if diff_active[idx] {
                        let action = rule.on_diff_change(commit, repo, &change)?;
                        if let gix::object::tree::diff::Action::Break(()) = action {
                            diff_active[idx] = false;
                        } else {
                            all_disinterested = false;
                        }
                    }
                }

                if all_disinterested {
                    Ok::<_, eyre::Report>(gix::object::tree::diff::Action::Break(()))
                } else {
                    Ok::<_, eyre::Report>(gix::object::tree::diff::Action::Continue(()))
                }
            });

        match outcome {
            Ok(_) => {}
            Err(gix::object::tree::diff::for_each::Error::Diff(
                gix::diff::tree_with_rewrites::Error::Diff(gix::diff::tree::Error::Cancelled),
            )) => {
                // It's not an error for on_diff_change to cancel processing! That's actually desirable,
                // because it means we can short circuit processing.
            }
            Err(e) => {
                return Err(e).wrap_err_with(|| format!("Failed to diff commit {}", commit.id()));
            }
        }

        let mut achievements = Vec::new();
        for rule in &mut self.rules {
            if rule.is_interested_in_diffs() {
                achievements.extend(rule.on_diff_end(commit, self.repo));
            }
        }
        Ok(achievements)
    }
}

#[cfg(test)]
mod tests {
    use herostratus_tests::fixtures::repository;

    use super::*;
    use crate::rules::test_rules_old::{AlwaysFail, ParticipationTrophy, ParticipationTrophy2};

    fn default_mailmap() -> MailmapResolver {
        MailmapResolver::new(gix::mailmap::Snapshot::default(), None, None).unwrap()
    }

    #[test]
    fn test_process_commit_no_rules() {
        let temp_repo = repository::Builder::new()
            .commit("Initial commit")
            .build()
            .unwrap();
        let oid = crate::git::rev::parse("HEAD", &temp_repo.repo).unwrap();

        let mut engine = RuleEngine::new(
            &temp_repo.repo,
            Vec::new(),
            HashSet::new(),
            default_mailmap(),
        )
        .unwrap();
        let achievements = engine.process_commit(oid).unwrap();
        assert!(achievements.is_empty());
        assert_eq!(engine.num_commits_processed(), 1);
    }

    #[test]
    fn test_process_commit_no_matches() {
        let temp_repo = repository::Builder::new()
            .commit("Initial commit")
            .build()
            .unwrap();
        let oid = crate::git::rev::parse("HEAD", &temp_repo.repo).unwrap();

        let rules: Vec<Box<dyn RulePlugin>> = vec![Box::new(AlwaysFail)];
        let mut engine =
            RuleEngine::new(&temp_repo.repo, rules, HashSet::new(), default_mailmap()).unwrap();
        let achievements = engine.process_commit(oid).unwrap();
        assert!(achievements.is_empty());
    }

    #[test]
    fn test_process_commit_with_match() {
        let temp_repo = repository::Builder::new()
            .commit("Initial commit")
            .build()
            .unwrap();
        let oid = crate::git::rev::parse("HEAD", &temp_repo.repo).unwrap();

        let rules: Vec<Box<dyn RulePlugin>> =
            vec![Box::new(AlwaysFail), Box::new(ParticipationTrophy)];
        let mut engine =
            RuleEngine::new(&temp_repo.repo, rules, HashSet::new(), default_mailmap()).unwrap();
        let achievements = engine.process_commit(oid).unwrap();
        assert_eq!(achievements.len(), 1);
        assert_eq!(achievements[0].commit, oid);
    }

    #[test]
    fn test_finalize_collects_achievements() {
        let temp_repo = repository::Builder::new()
            .commit("Initial commit")
            .build()
            .unwrap();
        let oid = crate::git::rev::parse("HEAD", &temp_repo.repo).unwrap();

        let rules: Vec<Box<dyn RulePlugin>> = vec![Box::new(ParticipationTrophy2::default())];
        let mut engine =
            RuleEngine::new(&temp_repo.repo, rules, HashSet::new(), default_mailmap()).unwrap();

        // process_commit yields nothing from ParticipationTrophy2
        let achievements = engine.process_commit(oid).unwrap();
        assert!(achievements.is_empty());

        // finalize yields the achievement
        let achievements = engine.finalize();
        assert_eq!(achievements.len(), 1);
    }

    #[test]
    fn test_config_disabled_filters_process() {
        let temp_repo = repository::Builder::new()
            .commit("Initial commit")
            .build()
            .unwrap();
        let oid = crate::git::rev::parse("HEAD", &temp_repo.repo).unwrap();

        let rules: Vec<Box<dyn RulePlugin>> = vec![Box::new(ParticipationTrophy)];
        let mut engine = RuleEngine::new(
            &temp_repo.repo,
            rules,
            HashSet::from([2]),
            default_mailmap(),
        )
        .unwrap();

        // Achievement is generated but filtered out by config_disabled
        let achievements = engine.process_commit(oid).unwrap();
        assert!(achievements.is_empty());
    }

    #[test]
    fn test_suppressed_filters_process_but_not_finalize() {
        let temp_repo = repository::Builder::new()
            .commit("Initial commit")
            .build()
            .unwrap();
        let oid = crate::git::rev::parse("HEAD", &temp_repo.repo).unwrap();

        let rules: Vec<Box<dyn RulePlugin>> = vec![Box::new(ParticipationTrophy2::default())];
        let mut engine =
            RuleEngine::new(&temp_repo.repo, rules, HashSet::new(), default_mailmap()).unwrap();

        // Process a commit so ParticipationTrophy2 has something to finalize
        let achievements = engine.process_commit(oid).unwrap();
        assert!(achievements.is_empty());

        engine.suppress_descriptors(&[3]);

        // finalize still lets suppressed achievements through
        let achievements = engine.finalize();
        assert_eq!(achievements.len(), 1);
    }

    #[test]
    fn test_get_enabled_rule_ids() {
        let temp_repo = repository::Builder::new()
            .commit("Initial commit")
            .build()
            .unwrap();

        let rules: Vec<Box<dyn RulePlugin>> = vec![Box::new(ParticipationTrophy)];
        let mut engine =
            RuleEngine::new(&temp_repo.repo, rules, HashSet::new(), default_mailmap()).unwrap();

        assert_eq!(engine.get_enabled_rule_ids(), vec![2]);

        // config_disabled removes from enabled
        engine.config_disabled.insert(2);
        assert!(engine.get_enabled_rule_ids().is_empty());
    }

    #[test]
    fn test_process_commit_resolves_author() {
        let temp_repo = repository::Builder::new()
            .commit("Initial commit")
            .build()
            .unwrap();
        let oid = crate::git::rev::parse("HEAD", &temp_repo.repo).unwrap();

        let rules: Vec<Box<dyn RulePlugin>> = vec![Box::new(ParticipationTrophy)];
        let mut engine =
            RuleEngine::new(&temp_repo.repo, rules, HashSet::new(), default_mailmap()).unwrap();
        let achievements = engine.process_commit(oid).unwrap();
        assert_eq!(achievements.len(), 1);
        assert_eq!(achievements[0].author_name, "Herostratus");
        assert_eq!(achievements[0].author_email, "Herostratus@example.com");
    }

    #[test]
    fn test_process_commit_resolves_author_with_mailmap() {
        let temp_repo = repository::Builder::new()
            .commit("Initial commit")
            .build()
            .unwrap();
        let oid = crate::git::rev::parse("HEAD", &temp_repo.repo).unwrap();

        let mailmap_dir = tempfile::tempdir().unwrap();
        let mailmap_path = mailmap_dir.path().join("mailmap");
        std::fs::write(
            &mailmap_path,
            "Canonical Name <canonical@example.com> Herostratus <Herostratus@example.com>\n",
        )
        .unwrap();

        let mailmap =
            MailmapResolver::new(gix::mailmap::Snapshot::default(), Some(&mailmap_path), None)
                .unwrap();

        let rules: Vec<Box<dyn RulePlugin>> = vec![Box::new(ParticipationTrophy)];
        let mut engine = RuleEngine::new(&temp_repo.repo, rules, HashSet::new(), mailmap).unwrap();
        let achievements = engine.process_commit(oid).unwrap();
        assert_eq!(achievements.len(), 1);
        assert_eq!(achievements[0].author_name, "Canonical Name");
        assert_eq!(achievements[0].author_email, "canonical@example.com");
    }

    #[test]
    fn test_finalize_resolves_author() {
        let temp_repo = repository::Builder::new()
            .commit("Initial commit")
            .build()
            .unwrap();
        let oid = crate::git::rev::parse("HEAD", &temp_repo.repo).unwrap();

        let rules: Vec<Box<dyn RulePlugin>> = vec![Box::new(ParticipationTrophy2::default())];
        let mut engine =
            RuleEngine::new(&temp_repo.repo, rules, HashSet::new(), default_mailmap()).unwrap();

        let achievements = engine.process_commit(oid).unwrap();
        assert!(achievements.is_empty());

        let achievements = engine.finalize();
        assert_eq!(achievements.len(), 1);
        assert_eq!(achievements[0].author_name, "Herostratus");
        assert_eq!(achievements[0].author_email, "Herostratus@example.com");
    }
}
