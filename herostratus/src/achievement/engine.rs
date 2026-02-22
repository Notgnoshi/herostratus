use crate::achievement::Achievement;
use crate::rules::RulePlugin;

/// The rule execution engine. Owns the rules, handles diff dispatch, and tracks commit processing.
pub(crate) struct RuleEngine<'repo> {
    repo: &'repo gix::Repository,
    rules: Vec<Box<dyn RulePlugin>>,

    // This cache is unbounded and needs to be reset periodically to avoid infinite memory growth.
    // Don't reset it every commit, because each commit needs to lookup itself and its parent(s).
    // But we shouldn't *never* reset it, because then we'd end up holding the whole history in
    // memory. So we reset it every N commits processed.
    diff_cache: gix::diff::blob::Platform,
    num_commits_processed: u64,
}

impl<'repo> RuleEngine<'repo> {
    pub fn new(repo: &'repo gix::Repository, rules: Vec<Box<dyn RulePlugin>>) -> Self {
        let diff_cache = repo
            .diff_resource_cache_for_tree_diff()
            .expect("Failed to create diff cache");
        Self {
            repo,
            rules,
            diff_cache,
            num_commits_processed: 0,
        }
    }

    /// Apply all enabled rules to a single commit (process + diff)
    pub fn process_commit(&mut self, oid: gix::ObjectId) -> Vec<Achievement> {
        let commit = self
            .repo
            .find_commit(oid)
            .expect("Failed to find commit from Oids iterator");
        self.num_commits_processed += 1;

        let mut achievements = Vec::new();
        for rule in &mut self.rules {
            achievements.extend(rule.process(&commit, self.repo));
        }

        achievements.extend(self.diff_commit(&commit));

        const CLEAR_CACHE_EVERY_N: u64 = 50; // SWAG: Scientific Wild Ass Guess
        if self
            .num_commits_processed
            .is_multiple_of(CLEAR_CACHE_EVERY_N)
        {
            self.diff_cache.clear_resource_cache_keep_allocation();
        }

        if achievements.len() > 1 {
            tracing::debug!(
                "Generated {} achievements for commit {}",
                achievements.len(),
                commit.id()
            );
        }

        achievements
    }

    /// Call finalize() on all rules, returning accumulated achievements
    pub fn finalize(&mut self) -> Vec<Achievement> {
        tracing::debug!("Finalizing rules ...");
        let mut achievements = Vec::new();
        for rule in &mut self.rules {
            achievements.extend(rule.finalize(self.repo));
        }
        achievements
    }

    pub fn get_enabled_rule_ids(&self) -> Vec<usize> {
        let descriptors = self.rules.iter().flat_map(|r| r.get_descriptors());
        descriptors
            .filter_map(|d| d.enabled.then_some(d.id))
            .collect()
    }

    pub fn disable_rule_by_id(&mut self, id: usize) {
        for rule in &mut self.rules {
            rule.disable_by_id(id);
        }
    }

    pub fn enable_rule_by_id(&mut self, id: usize) {
        for rule in &mut self.rules {
            rule.enable_by_id(id);
        }
    }

    pub fn retain_rules(&mut self, f: impl FnMut(&Box<dyn RulePlugin>) -> bool) {
        self.rules.retain(f);
    }

    pub fn rules(&self) -> &[Box<dyn RulePlugin>] {
        &self.rules
    }

    pub fn rules_mut(&mut self) -> &mut [Box<dyn RulePlugin>] {
        &mut self.rules
    }

    pub fn num_commits_processed(&self) -> u64 {
        self.num_commits_processed
    }

    pub fn repo(&self) -> &'repo gix::Repository {
        self.repo
    }

    fn diff_commit(&mut self, commit: &gix::Commit) -> Vec<Achievement> {
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
            return achievements;
        }

        let commit_tree = commit.tree().unwrap();
        let parent_tree = match parent {
            Some(pid) => {
                match self.repo.find_commit(pid) {
                    Ok(parent) => parent.tree().unwrap(),
                    // This could be a shallow clone where the parent commit is missing.
                    Err(_) => self.repo.empty_tree(),
                }
            }
            None => self.repo.empty_tree(),
        };

        let mut changes = parent_tree.changes().unwrap();
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
                        if let gix::object::tree::diff::Action::Cancel = action {
                            diff_active[idx] = false;
                        } else {
                            all_disinterested = false;
                        }
                    }
                }

                if all_disinterested {
                    Ok::<_, eyre::Report>(gix::object::tree::diff::Action::Cancel)
                } else {
                    Ok::<_, eyre::Report>(gix::object::tree::diff::Action::Continue)
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
                panic!("Failed to diff commit {}: {e:?}", commit.id());
            }
        }

        let mut achievements = Vec::new();
        for rule in &mut self.rules {
            if rule.is_interested_in_diffs() {
                achievements.extend(rule.on_diff_end(commit, self.repo));
            }
        }
        achievements
    }
}

#[cfg(test)]
mod tests {
    use herostratus_tests::fixtures;

    use super::*;
    use crate::rules::test_rules::{AlwaysFail, ParticipationTrophy, ParticipationTrophy2};

    #[test]
    fn test_process_commit_no_rules() {
        let temp_repo = fixtures::repository::simplest().unwrap();
        let oid = crate::git::rev::parse("HEAD", &temp_repo.repo).unwrap();

        let mut engine = RuleEngine::new(&temp_repo.repo, Vec::new());
        let achievements = engine.process_commit(oid);
        assert!(achievements.is_empty());
        assert_eq!(engine.num_commits_processed(), 1);
    }

    #[test]
    fn test_process_commit_no_matches() {
        let temp_repo = fixtures::repository::simplest().unwrap();
        let oid = crate::git::rev::parse("HEAD", &temp_repo.repo).unwrap();

        let rules: Vec<Box<dyn RulePlugin>> = vec![Box::new(AlwaysFail::default())];
        let mut engine = RuleEngine::new(&temp_repo.repo, rules);
        let achievements = engine.process_commit(oid);
        assert!(achievements.is_empty());
    }

    #[test]
    fn test_process_commit_with_match() {
        let temp_repo = fixtures::repository::simplest().unwrap();
        let oid = crate::git::rev::parse("HEAD", &temp_repo.repo).unwrap();

        let rules: Vec<Box<dyn RulePlugin>> = vec![
            Box::new(AlwaysFail::default()),
            Box::new(ParticipationTrophy::default()),
        ];
        let mut engine = RuleEngine::new(&temp_repo.repo, rules);
        let achievements = engine.process_commit(oid);
        assert_eq!(achievements.len(), 1);
        assert_eq!(achievements[0].commit, oid);
    }

    #[test]
    fn test_finalize_collects_achievements() {
        let temp_repo = fixtures::repository::simplest().unwrap();
        let oid = crate::git::rev::parse("HEAD", &temp_repo.repo).unwrap();

        let rules: Vec<Box<dyn RulePlugin>> = vec![Box::new(ParticipationTrophy2::default())];
        let mut engine = RuleEngine::new(&temp_repo.repo, rules);

        // process_commit yields nothing from ParticipationTrophy2
        let achievements = engine.process_commit(oid);
        assert!(achievements.is_empty());

        // finalize yields the achievement
        let achievements = engine.finalize();
        assert_eq!(achievements.len(), 1);
    }

    #[test]
    fn test_disable_enable_rule_by_id() {
        let temp_repo = fixtures::repository::simplest().unwrap();

        let rules: Vec<Box<dyn RulePlugin>> = vec![Box::new(ParticipationTrophy::default())];
        let mut engine = RuleEngine::new(&temp_repo.repo, rules);

        assert_eq!(engine.get_enabled_rule_ids(), vec![2]);

        engine.disable_rule_by_id(2);
        assert!(engine.get_enabled_rule_ids().is_empty());

        engine.enable_rule_by_id(2);
        assert_eq!(engine.get_enabled_rule_ids(), vec![2]);
    }

    #[test]
    fn test_retain_rules() {
        let temp_repo = fixtures::repository::simplest().unwrap();

        let rules: Vec<Box<dyn RulePlugin>> = vec![
            Box::new(AlwaysFail::default()),
            Box::new(ParticipationTrophy::default()),
        ];
        let mut engine = RuleEngine::new(&temp_repo.repo, rules);
        assert_eq!(engine.rules().len(), 2);

        engine.retain_rules(|r| r.name() != "AlwaysFail");
        assert_eq!(engine.rules().len(), 1);
        assert_eq!(engine.rules()[0].name(), "ParticipationTrophy");
    }
}
