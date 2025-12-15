use std::time::Instant;

use eyre::WrapErr;

use crate::achievement::{Achievement, LoggedRule, Rule};
use crate::config::Config;

/// An iterator of [Achievement]s
pub struct Achievements<'repo, Oids>
where
    Oids: Iterator<Item = gix::ObjectId>,
{
    repo: &'repo gix::Repository,
    oids: Oids,
    rules: Vec<Box<dyn Rule>>,

    accumulated: std::vec::IntoIter<Achievement>,
    has_finalized: bool,

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
            let new = rule.process_log(&commit, self.repo);
            if !new.is_empty() {
                achievements.extend(new);
            }
        }

        if !achievements.is_empty() {
            tracing::debug!(
                "Generated {} achievements for commit {}",
                achievements.len(),
                commit.id()
            );
        }

        achievements
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
            let mut temp = rule.finalize_log(self.repo);
            achievements.append(&mut temp);
        }
        self.accumulated = achievements.into_iter();
    }

    fn get_next_accumulated(&mut self) -> Option<Achievement> {
        self.accumulated.next()
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
                self.num_achievements_generated += 1;
                return achievement;
            }
        }

        // Once done processing all of the rules, collect any achievements that the rules stored.
        if !self.has_finalized {
            self.finalize_all_rules();
            self.has_finalized = true;
        }
        if let Some(achievement) = self.get_next_accumulated() {
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
    repo: &gix::Repository,
    rules: Vec<Box<dyn Rule>>,
) -> Achievements<'_, Oids>
where
    Oids: Iterator<Item = gix::ObjectId>,
{
    Achievements {
        repo,
        oids,
        rules,
        accumulated: Vec::new().into_iter(),
        has_finalized: false,
        start_processing: None,
        num_commits_processed: 0,
        num_achievements_generated: 0,
    }
}

pub fn grant<'repo>(
    config: Option<&Config>,
    reference: &str,
    repo: &'repo gix::Repository,
    depth: Option<usize>,
) -> eyre::Result<Achievements<'repo, Box<dyn Iterator<Item = gix::ObjectId> + 'repo>>> {
    grant_with_rules(reference, repo, depth, crate::rules::builtin_rules(config))
}

pub fn grant_with_rules<'repo>(
    reference: &str,
    repo: &'repo gix::Repository,
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
        Ok(process_rules(Box::new(oids.take(depth)), repo, rules))
    } else {
        Ok(process_rules(Box::new(oids), repo, rules))
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
        let achievements = grant_with_rules("HEAD", &temp_repo.repo, None, rules).unwrap();
        let achievements: Vec<_> = achievements.collect();
        assert!(achievements.is_empty());
    }

    #[test]
    fn test_iterator_no_matches() {
        let temp_repo = fixtures::repository::simplest().unwrap();
        let rules = vec![Box::new(AlwaysFail::default()) as Box<dyn Rule>];
        let achievements = grant_with_rules("HEAD", &temp_repo.repo, None, rules).unwrap();
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
        let achievements = grant_with_rules("HEAD", &temp_repo.repo, None, rules).unwrap();
        let achievements: Vec<_> = achievements.collect();
        assert_eq!(achievements.len(), 1);
    }

    #[test]
    fn test_awards_on_finalize() {
        let temp_repo = fixtures::repository::simplest().unwrap();

        let rules = vec![Box::new(ParticipationTrophy2::default()) as Box<dyn Rule>];
        let achievements = grant_with_rules("HEAD", &temp_repo.repo, None, rules).unwrap();
        let achievements: Vec<_> = achievements.collect();
        assert_eq!(achievements.len(), 1);
    }
}
