use std::time::Instant;

use eyre::WrapErr;

use crate::achievement::{Achievement, LoggedRule, Rule};
use crate::config::Config;

/// An iterator of [Achievement]s
pub struct Achievements<'repo, Oids>
where
    Oids: Iterator<Item = git2::Oid>,
{
    repo: &'repo git2::Repository,
    oids: Oids,
    rules: Vec<Box<dyn Rule>>,

    current_commit: Option<git2::Commit<'repo>>,
    next_rule: usize,

    finalized: Option<std::vec::IntoIter<Achievement>>,

    start_processing: Option<Instant>,
    num_commits_processed: u64,
    num_achievements_generated: u64,
}

impl<Oids> Achievements<'_, Oids>
where
    Oids: Iterator<Item = git2::Oid>,
{
    // Returning None indicates rule processing is finished
    fn get_next_achievement_online(&mut self) -> Option<Achievement> {
        if self.rules.is_empty() {
            return None;
        }

        let mut retval = None;
        // At least one rule is processed each iteration
        while retval.is_none() {
            // Roll over to the next commit if we've finished processing this one
            if self.next_rule >= self.rules.len() {
                self.next_rule = 0;
                let oid = self.oids.next()?;
                // I think the only way this could happen if the commit was garbage collected
                // during traversal, which is pretty unlikely?
                let commit = self
                    .repo
                    .find_commit(oid)
                    .expect("Failed to find commit in repository");
                self.current_commit = Some(commit);
                self.num_commits_processed += 1;
            }

            let Some(commit) = &self.current_commit else {
                unreachable!();
            };

            // Process the rules on this commit, stopping after the first achievement
            retval = self.rules[self.next_rule].process_log(commit, self.repo);
            self.next_rule += 1;
        }
        retval
    }

    fn finalize_all_rules(&mut self) {
        tracing::debug!("Finalizing rules ...");
        let mut achievements = Vec::new();
        for rule in &mut self.rules {
            let mut temp = rule.finalize_log(self.repo);
            achievements.append(&mut temp);
        }
        self.finalized = Some(achievements.into_iter());
    }

    fn get_next_achievement_finalized(&mut self) -> Option<Achievement> {
        let finalized = self.finalized.as_mut()?;
        finalized.next()
    }
}

impl<Oids> Iterator for Achievements<'_, Oids>
where
    Oids: Iterator<Item = git2::Oid>,
{
    type Item = Achievement;

    fn next(&mut self) -> Option<Self::Item> {
        if self.start_processing.is_none() {
            self.start_processing = Some(Instant::now());
        }

        // Get all of the achievements from processing the rules online
        if let Some(achievement) = self.get_next_achievement_online() {
            self.num_achievements_generated += 1;
            return Some(achievement);
        }

        // Once done processing all of the rules, collect any achievements that the rules stored.
        if self.finalized.is_none() {
            self.finalize_all_rules();
        }
        if let Some(achievement) = self.get_next_achievement_finalized() {
            self.num_achievements_generated += 1;
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
pub fn process_rules<Oids>(
    oids: Oids,
    repo: &git2::Repository,
    rules: Vec<Box<dyn Rule>>,
) -> Achievements<Oids>
where
    Oids: Iterator<Item = git2::Oid>,
{
    Achievements {
        repo,
        oids,
        rules,
        current_commit: None,
        next_rule: usize::MAX,
        finalized: None,
        start_processing: None,
        num_commits_processed: 0,
        num_achievements_generated: 0,
    }
}

pub fn grant<'repo>(
    config: Option<&Config>,
    reference: &str,
    repo: &'repo git2::Repository,
) -> eyre::Result<impl Iterator<Item = Achievement> + 'repo> {
    grant_with_rules(reference, repo, crate::rules::builtin_rules(config))
}

pub fn grant_with_rules<'repo>(
    reference: &str,
    repo: &'repo git2::Repository,
    rules: Vec<Box<dyn Rule>>,
) -> eyre::Result<impl Iterator<Item = Achievement> + 'repo> {
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
    Ok(process_rules(oids, repo, rules))
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
        let achievements = grant_with_rules("HEAD", &temp_repo.repo, rules).unwrap();
        let achievements: Vec<_> = achievements.collect();
        assert!(achievements.is_empty());
    }

    #[test]
    fn test_iterator_no_matches() {
        let temp_repo = fixtures::repository::simplest().unwrap();
        let rules = vec![Box::new(AlwaysFail) as Box<dyn Rule>];
        let achievements = grant_with_rules("HEAD", &temp_repo.repo, rules).unwrap();
        let achievements: Vec<_> = achievements.collect();
        assert!(achievements.is_empty());
    }

    #[test]
    fn test_iterator_all_matches() {
        let temp_repo = fixtures::repository::simplest().unwrap();

        let rules = vec![
            Box::new(AlwaysFail) as Box<dyn Rule>,
            Box::new(ParticipationTrophy) as Box<dyn Rule>,
        ];
        let achievements = grant_with_rules("HEAD", &temp_repo.repo, rules).unwrap();
        let achievements: Vec<_> = achievements.collect();
        assert_eq!(achievements.len(), 1);
    }

    #[test]
    fn test_awards_on_finalize() {
        let temp_repo = fixtures::repository::simplest().unwrap();

        let rules = vec![Box::new(ParticipationTrophy2) as Box<dyn Rule>];
        let achievements = grant_with_rules("HEAD", &temp_repo.repo, rules).unwrap();
        let achievements: Vec<_> = achievements.collect();
        assert_eq!(achievements.len(), 1);
    }
}
