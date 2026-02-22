use std::path::{Path, PathBuf};
use std::time::Instant;

use eyre::WrapErr;

use crate::achievement::Achievement;
use crate::achievement::engine::RuleEngine;
use crate::config::Config;
use crate::rules::RulePlugin;

/// An iterator of [Achievement]s
pub struct Achievements<'repo, Oids>
where
    Oids: Iterator<Item = gix::ObjectId>,
{
    data_dir: Option<PathBuf>,
    repo_name: String,
    oids: Oids,
    engine: RuleEngine<'repo>,

    accumulated: std::vec::IntoIter<Achievement>,
    has_finalized: bool,

    checkpoint: crate::cache::CheckpointCache,
    first_commit: Option<gix::ObjectId>,
    // These are rules that were already processed on the previous run and need to be skipped on
    // this run once we get to the last processed commit.
    suppressed_rules: Vec<usize>,

    pub start_processing: Option<Instant>,
    pub num_commits_processed: u64,
    pub num_achievements_generated: u64,
}

impl<Oids> Achievements<'_, Oids>
where
    Oids: Iterator<Item = gix::ObjectId>,
{
    fn try_handle_early_exit(&mut self, oid: gix::ObjectId) -> (Vec<Achievement>, bool) {
        if self.first_commit.is_none() {
            self.first_commit = Some(oid);
        }
        if self.checkpoint.data.commit.is_none() {
            // If there's nothing in the cache, we continue processing and don't early-exit
            return (Vec::new(), true);
        }

        let last_oid = self.checkpoint.data.commit.expect("Checked above");
        if oid == last_oid {
            // We've hit a commit we've already processed. Do we need to keep going with any new
            // rules that were added since the last time we ran?
            //
            // CASE 1: No new rules were added since the last time we ran; we can finalize and stop
            //         processing new commits.
            //
            // CASE 2: New rules were added since the last time we ran; we need to suppress the old
            //         rules and continue processing commits with just the new rules.
            //
            // PATHOLOGICAL CASE: From the last time we ran, an existing `RulePlugin` gained a new
            //                    `AchievementDescriptor`. Because of this edge case, we keep track
            //                    of any rules we had to suppressed, disable them here, and then
            //                    when we finalize, we can re-enable the suppressed rules
            //                    (otherwise, finalization would skip over the disabled rules).
            tracing::debug!("Reached last processed commit {oid}");

            // Disable any rules that were already processed from the last run
            for id in &self.checkpoint.data.rules {
                for rule in self.engine.rules_mut() {
                    for desc in rule.get_descriptors() {
                        if desc.id == *id {
                            self.suppressed_rules.push(*id);
                        }
                    }
                    rule.disable_by_id(*id);
                }
            }

            // Do we have any enable rules to keep processing?
            let continue_processing = !self.engine.get_enabled_rule_ids().is_empty();
            if !continue_processing {
                tracing::info!(
                    "No new rules added since last run; finalizing achievements and exiting early ..."
                );
                self.has_finalized = true;
                return (self.get_finalized_achievements(), continue_processing);
            }

            // As an optimization, if there are any rules we can skip entirely, finalize them now
            // and remove them from the list of rules to process so we can reduce how many rules
            // need to handle the commits that were already processed.
            let mut achievements = Vec::new();
            let repo = self.engine.repo();
            for rule in self.engine.rules_mut() {
                let is_disabled = rule.get_descriptors().iter().all(|d| !d.enabled);
                if is_disabled {
                    let names: Vec<_> = rule.get_descriptors().iter().map(|d| d.human_id).collect();
                    let rule_name = names.join(",");

                    tracing::debug!(
                        "{rule_name:?} doesn't have any new achievements to process; finalizing ..."
                    );
                    // Need to re-enable it temporarily so that finalizing it works as expected.
                    for suppressed in &self.suppressed_rules {
                        // TODO: The debug logs from this enable/disable are noisy and confusing to
                        // follow!
                        rule.enable_by_id(*suppressed);
                    }
                    let new = rule.finalize(repo);
                    if !new.is_empty() {
                        achievements.extend(new);
                    }
                    // Mark it as disabled again, so we can filter out any rules that are fully
                    // disabled.
                    for suppressed in &self.suppressed_rules {
                        rule.disable_by_id(*suppressed);
                    }
                } else {
                    let names: Vec<_> = rule
                        .get_descriptors()
                        .iter()
                        .filter_map(|d| d.enabled.then_some(d.human_id))
                        .collect();
                    let rule_name = names.join(",");
                    tracing::warn!(
                        "Continuing to process new rule {rule_name:?} on already-processed commits"
                    );
                }
            }

            // Remove any rules that are fully disabled and finalized.
            self.engine
                .retain_rules(|r| !r.get_descriptors().iter().all(|d| !d.enabled));

            (achievements, continue_processing)
        } else {
            // We've not processed this commit yet, so keep going
            (Vec::new(), true)
        }
    }

    fn process_commit(&mut self, oid: gix::ObjectId) -> Vec<Achievement> {
        let (mut achievements, continue_processing) = self.try_handle_early_exit(oid);
        if !continue_processing {
            return achievements;
        }

        let new = self.engine.process_commit(oid);
        self.num_commits_processed = self.engine.num_commits_processed();
        if !new.is_empty() {
            achievements.extend(new);
        }

        achievements
    }

    fn process_commits_until_first_achievement(&mut self) {
        while let Some(oid) = self.oids.next()
            && !self.has_finalized
        {
            let achievements = self.process_commit(oid);
            if !achievements.is_empty() {
                self.accumulated = achievements.into_iter();
                break;
            }
        }
    }

    // Returning None indicates rule processing is finished
    fn get_next_achievement_online(&mut self) -> Option<Achievement> {
        if self.engine.is_empty() {
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

    fn get_finalized_achievements(&mut self) -> Vec<Achievement> {
        tracing::debug!("Re-enabling suppressed rules: {:?}", self.suppressed_rules);
        for id in &self.suppressed_rules {
            self.engine.enable_rule_by_id(*id);
        }

        self.engine.finalize()
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
            self.accumulated = self.get_finalized_achievements().into_iter();
            self.has_finalized = true;
        }
        if let Some(achievement) = self.get_next_accumulated() {
            return Some(achievement);
        }

        // If we get to here, we've finished generating achievements, and it's time to log summary
        // stats. Use .take() so that the stats are only logged once, even if .next() is repeatedly
        // called at the end.
        if let Some(start_timestamp) = self.start_processing.take() {
            self.checkpoint.data.rules = self.engine.get_enabled_rule_ids();
            self.checkpoint.data.commit = self.first_commit;
            // TODO: TOO MANY UNWRAPS
            self.checkpoint.save().expect("Failed to save checkpoint");

            for rule in self.engine.rules() {
                if self.data_dir.is_none() {
                    break;
                }
                if !rule.has_cache() {
                    continue;
                }
                let erased_cache = rule.fini_cache().expect("Failed to finalize rule cache");
                let rule_cache = crate::cache::RuleCache::new_for_rule(
                    self.data_dir.as_ref().unwrap(),
                    &self.repo_name,
                    rule.name(),
                    erased_cache,
                );
                rule_cache.save().expect("Failed to save RuleCache to disk");
            }

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
    data_dir: Option<&Path>,
    name: &str,
    mut rules: Vec<Box<dyn RulePlugin>>,
) -> Achievements<'repo, Oids>
where
    Oids: Iterator<Item = gix::ObjectId>,
{
    let data_dir = data_dir.map(|d| d.to_path_buf());

    let checkpoint = if let Some(dir) = &data_dir {
        crate::cache::CheckpointCache::from_data_dir(dir, name)
            .expect("Failed to load CheckpointCache from disk")
    } else {
        crate::cache::CheckpointCache::in_memory()
    };

    for rule in &mut rules {
        if !rule.has_cache() {
            continue;
        }

        let cache = if let Some(dir) = &data_dir {
            crate::cache::RuleCache::from_rule_name(dir, name, rule.name())
                .wrap_err(format!("Failed to load cache for rule '{}'", rule.name()))
                .unwrap()
        } else {
            crate::cache::RuleCache::in_memory()
        };
        rule.init_cache(cache.data)
            .wrap_err(format!(
                "Failed to initialize cache for rule '{}'",
                rule.name()
            ))
            .unwrap()
    }

    Achievements {
        data_dir,
        repo_name: name.to_string(),
        oids,
        engine: RuleEngine::new(repo, rules),
        accumulated: Vec::new().into_iter(),
        has_finalized: false,
        checkpoint,
        first_commit: None,
        suppressed_rules: Vec::new(),
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
    data_dir: Option<&Path>,
    name: &str,
) -> eyre::Result<Achievements<'repo, Box<dyn Iterator<Item = gix::ObjectId> + 'repo>>> {
    grant_with_rules(
        reference,
        repo,
        depth,
        data_dir,
        name,
        crate::rules::builtin_rules(config),
    )
}

pub fn grant_with_rules<'repo>(
    reference: &str,
    repo: &'repo gix::Repository,
    depth: Option<usize>,
    data_dir: Option<&Path>,
    name: &str,
    rules: Vec<Box<dyn RulePlugin>>,
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
            data_dir,
            name,
            rules,
        ))
    } else {
        Ok(process_rules(Box::new(oids), repo, data_dir, name, rules))
    }
}

#[cfg(test)]
mod tests {
    use herostratus_tests::fixtures;

    use super::*;
    use crate::achievement::AchievementDescriptor;
    use crate::rules::test_rules::{
        AlwaysFail, FlexibleRule, ParticipationTrophy, ParticipationTrophy2,
    };

    #[test]
    fn test_no_rules() {
        let temp_repo = fixtures::repository::simplest().unwrap();
        let rules = Vec::new();
        let achievements =
            grant_with_rules("HEAD", &temp_repo.repo, None, None, "", rules).unwrap();
        let achievements: Vec<_> = achievements.collect();
        assert!(achievements.is_empty());
    }

    #[test]
    fn test_iterator_no_matches() {
        let temp_repo = fixtures::repository::simplest().unwrap();
        let rules = vec![Box::new(AlwaysFail::default()) as Box<dyn RulePlugin>];
        let achievements =
            grant_with_rules("HEAD", &temp_repo.repo, None, None, "", rules).unwrap();
        let achievements: Vec<_> = achievements.collect();
        assert!(achievements.is_empty());
    }

    #[test]
    fn test_iterator_all_matches() {
        let temp_repo = fixtures::repository::simplest().unwrap();

        let rules = vec![
            Box::new(AlwaysFail::default()) as Box<dyn RulePlugin>,
            Box::new(ParticipationTrophy::default()) as Box<dyn RulePlugin>,
        ];
        let achievements =
            grant_with_rules("HEAD", &temp_repo.repo, None, None, "", rules).unwrap();
        let achievements: Vec<_> = achievements.collect();
        assert_eq!(achievements.len(), 1);
    }

    #[test]
    fn test_awards_on_finalize() {
        let temp_repo = fixtures::repository::simplest().unwrap();

        let rules = vec![Box::new(ParticipationTrophy2::default()) as Box<dyn RulePlugin>];
        let mut achievements =
            grant_with_rules("HEAD", &temp_repo.repo, None, None, "", rules).unwrap();
        let mut granted = Vec::new();
        for a in &mut achievements {
            granted.push(a);
        }
        assert_eq!(granted.len(), 1);

        let rev = crate::git::rev::parse("HEAD", &temp_repo.repo).unwrap();
        assert_eq!(achievements.checkpoint.data.commit.unwrap(), rev);
        assert_eq!(achievements.checkpoint.data.rules, [3]);
    }

    #[test]
    fn test_early_exit_no_new_rules() {
        let temp_repo = fixtures::repository::simplest().unwrap();

        let rules = vec![
            Box::new(AlwaysFail::default()) as Box<dyn RulePlugin>,
            Box::new(ParticipationTrophy::default()) as Box<dyn RulePlugin>,
        ];
        let achievements = grant_with_rules(
            "HEAD",
            &temp_repo.repo,
            None,
            Some(temp_repo.path()),
            "",
            rules,
        )
        .unwrap();
        let achievements: Vec<_> = achievements.collect();
        assert_eq!(achievements.len(), 1);

        // Run the same rules again on the same repo; should early exit without granting any new
        // achievements.
        let rules = vec![
            Box::new(AlwaysFail::default()) as Box<dyn RulePlugin>,
            Box::new(ParticipationTrophy::default()) as Box<dyn RulePlugin>,
        ];
        let achievements = grant_with_rules(
            "HEAD",
            &temp_repo.repo,
            None,
            Some(temp_repo.path()),
            "",
            rules,
        )
        .unwrap();
        let achievements: Vec<_> = achievements.collect();
        assert!(achievements.is_empty());

        // Add a new commit to the repo; will generate a single new achievement
        let new_commit =
            fixtures::repository::add_empty_commit(&temp_repo.repo, "new-commit").unwrap();
        let rules = vec![
            Box::new(AlwaysFail::default()) as Box<dyn RulePlugin>,
            Box::new(ParticipationTrophy::default()) as Box<dyn RulePlugin>,
        ];
        let achievements = grant_with_rules(
            "HEAD",
            &temp_repo.repo,
            None,
            Some(temp_repo.path()),
            "",
            rules,
        )
        .unwrap();
        let achievements: Vec<_> = achievements.collect();
        assert_eq!(achievements.len(), 1);
        assert_eq!(achievements[0].commit, new_commit);
    }

    #[test]
    fn test_continue_processing_with_new_rules() {
        let temp_repo = fixtures::repository::simplest().unwrap();
        let first_commit = crate::git::rev::parse("HEAD", &temp_repo.repo).unwrap();

        let rules = vec![
            Box::new(AlwaysFail::default()) as Box<dyn RulePlugin>, // 1
            Box::new(ParticipationTrophy::default()) as Box<dyn RulePlugin>, // 2
        ];
        let achievements = grant_with_rules(
            "HEAD",
            &temp_repo.repo,
            None,
            Some(temp_repo.path()),
            "",
            rules,
        )
        .unwrap();
        let achievements: Vec<_> = achievements.collect();
        assert_eq!(achievements.len(), 1);

        // Add a new commit to the repo
        let second_commit =
            fixtures::repository::add_empty_commit(&temp_repo.repo, "new-commit").unwrap();

        // Add a new rule; the new rule should process all commits; the old rules should only
        // process the newly added commit.
        let mut rules = vec![
            Box::new(AlwaysFail::default()) as Box<dyn RulePlugin>, // 1
            Box::new(ParticipationTrophy::default()) as Box<dyn RulePlugin>, // 2
            Box::new(ParticipationTrophy::default()) as Box<dyn RulePlugin>, // 3
        ];
        rules[1].get_descriptors_mut()[0].id = 2;
        rules[1].get_descriptors_mut()[0].name = "first instance";
        rules[2].get_descriptors_mut()[0].id = 3;
        rules[2].get_descriptors_mut()[0].name = "second instance";

        let achievements = grant_with_rules(
            "HEAD",
            &temp_repo.repo,
            None,
            Some(temp_repo.path()),
            "",
            rules,
        )
        .unwrap();
        let achievements: Vec<_> = achievements.collect();
        assert_eq!(achievements.len(), 3);

        // The new commit should get achievements from both the old and new rule instances
        assert_eq!(achievements[0].commit, second_commit);
        assert_eq!(achievements[0].name, "first instance");
        assert_eq!(achievements[1].commit, second_commit);
        assert_eq!(achievements[1].name, "second instance");

        // But the first commit should only get an achievement from the new rule instance
        assert_eq!(achievements[2].commit, first_commit);
        assert_eq!(achievements[2].name, "second instance");
    }

    #[test]
    fn test_continue_processing_pathological_case() {
        let temp_repo = fixtures::repository::simplest().unwrap();
        let first_commit = crate::git::rev::parse("HEAD", &temp_repo.repo).unwrap();

        let rules = vec![
            Box::new(AlwaysFail::default()) as Box<dyn RulePlugin>, // 1
            Box::new(FlexibleRule {
                descriptors: vec![
                    AchievementDescriptor {
                        enabled: true,
                        id: 2,
                        human_id: "rule1",
                        name: "rule1",
                        description: "",
                    },
                    AchievementDescriptor {
                        enabled: false,
                        id: 3,
                        human_id: "rule2",
                        name: "rule2",
                        description: "",
                    },
                ],
            }) as Box<dyn RulePlugin>,
        ];
        let mut achievements = grant_with_rules(
            "HEAD",
            &temp_repo.repo,
            None,
            Some(temp_repo.path()),
            "",
            rules,
        )
        .unwrap();
        let mut granted = Vec::new();
        for a in &mut achievements {
            granted.push(a);
        }
        assert_eq!(achievements.checkpoint.data.rules, [1, 2]);
        assert_eq!(granted.len(), 1);
        assert_eq!(granted[0].commit, first_commit);
        assert_eq!(granted[0].name, "rule1");

        // Add a new commit, and a new AchievementDescriptor to the existing RulePlugin implementation
        let second_commit =
            fixtures::repository::add_empty_commit(&temp_repo.repo, "new-commit").unwrap();
        let rules = vec![
            Box::new(AlwaysFail::default()) as Box<dyn RulePlugin>, // 1
            Box::new(FlexibleRule {
                descriptors: vec![
                    AchievementDescriptor {
                        enabled: true,
                        id: 2,
                        human_id: "rule1",
                        name: "rule1",
                        description: "",
                    },
                    AchievementDescriptor {
                        enabled: true,
                        id: 3,
                        human_id: "rule2",
                        name: "rule2",
                        description: "",
                    },
                ],
            }) as Box<dyn RulePlugin>,
        ];
        let achievements = grant_with_rules(
            "HEAD",
            &temp_repo.repo,
            None,
            Some(temp_repo.path()),
            "",
            rules,
        )
        .unwrap();
        let achievements: Vec<_> = achievements.collect();

        // The new commit should get achievements from both the old and new rule instances
        assert_eq!(achievements[0].commit, second_commit);
        assert_eq!(achievements[0].name, "rule1");
        assert_eq!(achievements[1].commit, second_commit);
        assert_eq!(achievements[1].name, "rule2");

        // But the first commit should only get an achievement from the new rule instance
        assert_eq!(achievements[2].commit, first_commit);
        assert_eq!(achievements[2].name, "rule2");
    }
}
