use std::time::Instant;

use eyre::WrapErr;

use crate::achievement::{Achievement, Rule};
use crate::config::Config;

/// An iterator of [Achievement]s
pub struct Achievements<'repo, Oids>
where
    Oids: Iterator<Item = gix::ObjectId>,
{
    repo: &'repo gix::Repository,
    oids: Oids,
    rules: Vec<Box<dyn Rule>>,
    // INVARIANT: 1-1 index correspondence with `rules`
    rule_diff_interest: Vec<bool>,

    accumulated: std::vec::IntoIter<Achievement>,
    has_finalized: bool,

    // This cache is unbounded and needs to be reset periodically to avoid infinite memory growth.
    // Don't reset it every commit, because each commit needs to lookup itself and its parent(s).
    // But we shouldn't *never* reset it, because then we'd end up holding the whole history in
    // memory. So we reset it every N commits processed.
    diff_cache: gix::diff::blob::Platform,
    // This cache is to enable short-circuiting processing commits that have already been processed
    // on a previous run.
    #[expect(unused)]
    entry_cache: &'repo mut crate::cache::EntryCache,

    pub start_processing: Option<Instant>,
    pub num_commits_processed: u64,
    pub num_achievements_generated: u64,
}

impl<Oids> Achievements<'_, Oids>
where
    Oids: Iterator<Item = gix::ObjectId>,
{
    fn process_commit(&mut self, oid: gix::ObjectId) -> Vec<Achievement> {
        let commit = self
            .repo
            .find_commit(oid)
            .expect("Failed to find commit from Oids iterator");
        self.num_commits_processed += 1;

        let mut achievements = Vec::new();
        for rule in &mut self.rules {
            let new = rule.process(&commit, self.repo);
            if !new.is_empty() {
                achievements.extend(new);
            }
        }

        let new = self.diff_commit(&commit);
        if !new.is_empty() {
            achievements.extend(new);
        }

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

    fn on_diff_start(&mut self, commit: &gix::Commit) {
        for (idx, rule) in &mut self.rules.iter_mut().enumerate() {
            let interest = rule.is_interested_in_diffs();
            self.rule_diff_interest[idx] = interest;
            if interest {
                rule.on_diff_start(commit, self.repo);
            }
        }
    }

    fn on_diff_change(
        &mut self,
        commit: &gix::Commit,
        change: gix::object::tree::diff::Change,
    ) -> eyre::Result<gix::object::tree::diff::Action> {
        // Can only cancel the top-level diff processing if all rules agree to cancel. But we want
        // to stop feeding changes into Rules that have already decided to cancel.
        let mut all_disinterested = true;
        for (idx, rule) in &mut self.rules.iter_mut().enumerate() {
            if self.rule_diff_interest[idx] {
                let action = rule.on_diff_change(commit, self.repo, &change)?;
                if let gix::object::tree::diff::Action::Cancel = action {
                    self.rule_diff_interest[idx] = false;
                } else {
                    all_disinterested = false;
                }
            }
        }

        if all_disinterested {
            Ok(gix::object::tree::diff::Action::Cancel)
        } else {
            Ok(gix::object::tree::diff::Action::Continue)
        }
    }

    fn on_diff_end(&mut self, commit: &gix::Commit) -> Vec<Achievement> {
        let mut achievements = Vec::new();
        for rule in &mut self.rules {
            if rule.is_interested_in_diffs() {
                let new = rule.on_diff_end(commit, self.repo);
                if !new.is_empty() {
                    achievements.extend(new);
                }
            }
        }
        achievements
    }

    fn diff_commit(&mut self, commit: &gix::Commit) -> Vec<Achievement> {
        self.on_diff_start(commit);
        let mut parents = commit.parent_ids();
        let parent = parents.next();
        if parents.next().is_some() {
            // This is a merge commit (has multiple parents), and we want to skip it
            return self.on_diff_end(commit);
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

        // Swap to avoid mutably borrowing self
        let mut diff_cache = self.repo.diff_resource_cache_for_tree_diff().unwrap();
        std::mem::swap(&mut diff_cache, &mut self.diff_cache);

        let outcome =
            changes.for_each_to_obtain_tree_with_cache(&commit_tree, &mut diff_cache, |change| {
                self.on_diff_change(commit, change)
            });

        // Put the cache back
        std::mem::swap(&mut self.diff_cache, &mut diff_cache);

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

        self.on_diff_end(commit)
    }

    fn process_commits_until_first_achievement(&mut self) {
        while let Some(oid) = self.oids.next() {
            let achievements = self.process_commit(oid);
            if !achievements.is_empty() {
                self.accumulated = achievements.into_iter();
                break;
            }
        }
    }

    // Returning None indicates rule processing is finished
    fn get_next_achievement_online(&mut self) -> Option<Achievement> {
        if self.rules.is_empty() {
            return None;
        }

        // If we have accumulated achievements, consume those first
        let accumulated = self.get_next_accumulated();
        if accumulated.is_some() {
            return accumulated;
        }

        // Otherwise process commits until one of them accumulates achievements
        self.process_commits_until_first_achievement();

        // And yield the first achievement accumulated from that
        self.get_next_accumulated()
    }

    fn finalize_all_rules(&mut self) {
        tracing::debug!("Finalizing rules ...");
        let mut achievements = Vec::new();
        for rule in &mut self.rules {
            let mut temp = rule.finalize(self.repo);
            achievements.append(&mut temp);
        }
        self.accumulated = achievements.into_iter();
    }

    fn get_next_accumulated(&mut self) -> Option<Achievement> {
        if let Some(achievement) = self.accumulated.next() {
            self.num_achievements_generated += 1;
            tracing::info!(
                "granted achievement: {:?} for commit {}",
                achievement.name,
                achievement.commit
            );
            Some(achievement)
        } else {
            None
        }
    }
}

impl<Oids> Iterator for Achievements<'_, Oids>
where
    Oids: Iterator<Item = gix::ObjectId>,
{
    type Item = Achievement;

    fn next(&mut self) -> Option<Self::Item> {
        if self.start_processing.is_none() {
            self.start_processing = Some(Instant::now());
        }

        // Get all of the achievements from processing the rules online
        if !self.has_finalized {
            let achievement = self.get_next_achievement_online();
            if achievement.is_some() {
                return achievement;
            }
        }

        // Once done processing all of the rules, collect any achievements that the rules stored.
        if !self.has_finalized {
            self.finalize_all_rules();
            self.has_finalized = true;
        }
        if let Some(achievement) = self.get_next_accumulated() {
            return Some(achievement);
        }

        // If we get to here, we've finished generating achievements, and it's time to log summary
        // stats. Use .take() so that the stats are only logged once, even if .next() is repeatedly
        // called at the end.
        if let Some(start_timestamp) = self.start_processing.take() {
            tracing::info!(
                "Generated {} achievements after processing {} commits in {:?}",
                self.num_achievements_generated,
                self.num_commits_processed,
                start_timestamp.elapsed()
            );
        }

        None
    }
}

/// Process the given `oids` with the specified `rules`
///
/// Returns a lazy iterator. The rules will be processed as the iterator advances.
fn process_rules<'repo, Oids>(
    oids: Oids,
    repo: &'repo gix::Repository,
    entry_cache: &'repo mut crate::cache::EntryCache,
    rules: Vec<Box<dyn Rule>>,
) -> Achievements<'repo, Oids>
where
    Oids: Iterator<Item = gix::ObjectId>,
{
    Achievements {
        repo,
        oids,
        rule_diff_interest: rules.iter().map(|r| r.is_interested_in_diffs()).collect(),
        rules,
        accumulated: Vec::new().into_iter(),
        has_finalized: false,
        diff_cache: repo
            .diff_resource_cache_for_tree_diff()
            .expect("Failed to create diff cache"),
        entry_cache,
        start_processing: None,
        num_commits_processed: 0,
        num_achievements_generated: 0,
    }
}

pub fn grant<'repo>(
    config: Option<&Config>,
    reference: &str,
    repo: &'repo gix::Repository,
    cache: &'repo mut crate::cache::EntryCache,
    depth: Option<usize>,
) -> eyre::Result<Achievements<'repo, Box<dyn Iterator<Item = gix::ObjectId> + 'repo>>> {
    grant_with_rules(
        reference,
        repo,
        cache,
        depth,
        crate::rules::builtin_rules(config),
    )
}

pub fn grant_with_rules<'repo>(
    reference: &str,
    repo: &'repo gix::Repository,
    cache: &'repo mut crate::cache::EntryCache,
    depth: Option<usize>,
    rules: Vec<Box<dyn Rule>>,
) -> eyre::Result<Achievements<'repo, Box<dyn Iterator<Item = gix::ObjectId> + 'repo>>> {
    let rev = crate::git::rev::parse(reference, repo)
        .wrap_err(format!("Failed to rev-parse: {reference:?}"))?;
    let oids =
        crate::git::rev::walk(rev, repo).wrap_err(format!("Failed to rev-walk rev: {rev:?}"))?;

    // TODO: There should be better error handling than this
    let oids = oids.filter_map(|o| match o {
        Ok(o) => Some(o),
        Err(e) => {
            tracing::error!("Skipping OID: {e:?}");
            None
        }
    });

    if let Some(depth) = depth {
        Ok(process_rules(
            Box::new(oids.take(depth)),
            repo,
            cache,
            rules,
        ))
    } else {
        Ok(process_rules(Box::new(oids), repo, cache, rules))
    }
}

#[cfg(test)]
mod tests {
    use herostratus_tests::fixtures;

    use super::*;
    use crate::rules::test_rules::{AlwaysFail, ParticipationTrophy, ParticipationTrophy2};

    #[test]
    fn test_no_rules() {
        let temp_repo = fixtures::repository::simplest().unwrap();
        let rules = Vec::new();
        let mut cache = crate::cache::EntryCache::default();
        let achievements =
            grant_with_rules("HEAD", &temp_repo.repo, &mut cache, None, rules).unwrap();
        let achievements: Vec<_> = achievements.collect();
        assert!(achievements.is_empty());
    }

    #[test]
    fn test_iterator_no_matches() {
        let temp_repo = fixtures::repository::simplest().unwrap();
        let rules = vec![Box::new(AlwaysFail::default()) as Box<dyn Rule>];
        let mut cache = crate::cache::EntryCache::default();
        let achievements =
            grant_with_rules("HEAD", &temp_repo.repo, &mut cache, None, rules).unwrap();
        let achievements: Vec<_> = achievements.collect();
        assert!(achievements.is_empty());
    }

    #[test]
    fn test_iterator_all_matches() {
        let temp_repo = fixtures::repository::simplest().unwrap();

        let rules = vec![
            Box::new(AlwaysFail::default()) as Box<dyn Rule>,
            Box::new(ParticipationTrophy::default()) as Box<dyn Rule>,
        ];
        let mut cache = crate::cache::EntryCache::default();
        let achievements =
            grant_with_rules("HEAD", &temp_repo.repo, &mut cache, None, rules).unwrap();
        let achievements: Vec<_> = achievements.collect();
        assert_eq!(achievements.len(), 1);
    }

    #[test]
    fn test_awards_on_finalize() {
        let temp_repo = fixtures::repository::simplest().unwrap();

        let rules = vec![Box::new(ParticipationTrophy2::default()) as Box<dyn Rule>];
        let mut cache = crate::cache::EntryCache::default();
        let achievements =
            grant_with_rules("HEAD", &temp_repo.repo, &mut cache, None, rules).unwrap();
        let achievements: Vec<_> = achievements.collect();
        assert_eq!(achievements.len(), 1);
    }
}
