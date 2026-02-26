use std::collections::HashSet;
use std::path::Path;
use std::time::{Duration, Instant};

use eyre::WrapErr;

use crate::achievement::Achievement;
use crate::achievement::checkpoint_strategy::{CheckpointStrategy, Continuation};
use crate::achievement::engine::RuleEngine;
use crate::config::Config;
use crate::rules::RulePlugin;

pub struct GrantStats {
    pub num_commits_processed: u64,
    pub num_achievements_generated: u64,
    pub elapsed: Duration,
}

pub fn grant(
    config: Option<&Config>,
    reference: &str,
    repo: &gix::Repository,
    depth: Option<usize>,
    data_dir: Option<&Path>,
    name: &str,
    on_achievement: impl FnMut(Achievement),
) -> eyre::Result<GrantStats> {
    let (rules, config_disabled) = crate::rules::builtin_rules(config);
    let global_mailmap = config.and_then(|c| c.mailmap_file.as_deref());
    let repo_mailmap = config
        .and_then(|c| c.repositories.get(name))
        .and_then(|rc| rc.mailmap_file.as_deref());
    grant_with_rules_and_disabled(
        reference,
        repo,
        depth,
        data_dir,
        name,
        rules,
        config_disabled,
        global_mailmap,
        repo_mailmap,
        on_achievement,
    )
}

pub fn grant_with_rules(
    reference: &str,
    repo: &gix::Repository,
    depth: Option<usize>,
    data_dir: Option<&Path>,
    name: &str,
    rules: Vec<Box<dyn RulePlugin>>,
    on_achievement: impl FnMut(Achievement),
) -> eyre::Result<GrantStats> {
    grant_with_rules_and_disabled(
        reference,
        repo,
        depth,
        data_dir,
        name,
        rules,
        HashSet::new(),
        None,
        None,
        on_achievement,
    )
}

#[allow(clippy::too_many_arguments)]
fn grant_with_rules_and_disabled(
    reference: &str,
    repo: &gix::Repository,
    depth: Option<usize>,
    data_dir: Option<&Path>,
    name: &str,
    rules: Vec<Box<dyn RulePlugin>>,
    config_disabled: HashSet<usize>,
    global_mailmap: Option<&Path>,
    repo_mailmap: Option<&Path>,
    on_achievement: impl FnMut(Achievement),
) -> eyre::Result<GrantStats> {
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
        run_pipeline(
            oids.take(depth),
            repo,
            data_dir,
            name,
            rules,
            config_disabled,
            global_mailmap,
            repo_mailmap,
            on_achievement,
        )
    } else {
        run_pipeline(
            oids,
            repo,
            data_dir,
            name,
            rules,
            config_disabled,
            global_mailmap,
            repo_mailmap,
            on_achievement,
        )
    }
}

#[allow(clippy::too_many_arguments)]
fn run_pipeline(
    oids: impl Iterator<Item = gix::ObjectId>,
    repo: &gix::Repository,
    data_dir: Option<&Path>,
    name: &str,
    mut rules: Vec<Box<dyn RulePlugin>>,
    config_disabled: HashSet<usize>,
    global_mailmap: Option<&Path>,
    repo_mailmap: Option<&Path>,
    mut on_achievement: impl FnMut(Achievement),
) -> eyre::Result<GrantStats> {
    let start = Instant::now();

    // Load caches
    let checkpoint = load_checkpoint(data_dir, name)?;
    load_rule_caches(&mut rules, data_dir, name)?;

    let snapshot = repo.open_mailmap();
    let mailmap =
        crate::git::mailmap::MailmapResolver::new(snapshot, global_mailmap, repo_mailmap)?;
    let mut engine = RuleEngine::new(repo, rules, config_disabled, mailmap)?;
    let mut strategy = CheckpointStrategy::new(checkpoint);
    let mut num_achievements: u64 = 0;

    let mut emit = |a: Achievement, count: &mut u64| {
        *count += 1;
        tracing::info!("granted achievement: {:?} for commit {}", a.name, a.commit);
        on_achievement(a);
    };

    // Main processing loop
    for oid in oids {
        let directive = strategy.on_commit(oid, &engine.get_enabled_rule_ids());

        match directive {
            Continuation::Process => {
                for a in engine.process_commit(oid)? {
                    emit(a, &mut num_achievements);
                }
            }
            Continuation::EarlyExit => {
                break;
            }
            Continuation::SuppressAndContinue {
                rule_ids_to_suppress,
            } => {
                let early = apply_suppress_and_continue(&mut engine, &rule_ids_to_suppress);
                for a in early {
                    emit(a, &mut num_achievements);
                }
                // Process the checkpoint commit with the remaining (new) rules
                for a in engine.process_commit(oid)? {
                    emit(a, &mut num_achievements);
                }
            }
        }
    }

    // Finalization
    for a in engine.finalize() {
        emit(a, &mut num_achievements);
    }

    save_caches(&engine, &mut strategy, data_dir, name)?;

    let elapsed = start.elapsed();
    tracing::info!(
        "Generated {num_achievements} achievements after processing {} commits in {elapsed:?}",
        engine.num_commits_processed()
    );

    Ok(GrantStats {
        num_commits_processed: engine.num_commits_processed(),
        num_achievements_generated: num_achievements,
        elapsed,
    })
}

fn load_checkpoint(
    data_dir: Option<&Path>,
    name: &str,
) -> eyre::Result<crate::cache::CheckpointCache> {
    if let Some(dir) = data_dir {
        crate::cache::CheckpointCache::from_data_dir(dir, name)
    } else {
        Ok(crate::cache::CheckpointCache::in_memory())
    }
}

fn load_rule_caches(
    rules: &mut [Box<dyn RulePlugin>],
    data_dir: Option<&Path>,
    name: &str,
) -> eyre::Result<()> {
    for rule in rules {
        if !rule.has_cache() {
            continue;
        }

        let cache = if let Some(dir) = data_dir {
            crate::cache::RuleCache::from_rule_name(dir, name, rule.name())
                .wrap_err(format!("Failed to load cache for rule '{}'", rule.name()))?
        } else {
            crate::cache::RuleCache::in_memory()
        };
        rule.init_cache(cache.data).wrap_err(format!(
            "Failed to initialize cache for rule '{}'",
            rule.name()
        ))?;
    }
    Ok(())
}

fn save_caches(
    engine: &RuleEngine,
    strategy: &mut CheckpointStrategy,
    data_dir: Option<&Path>,
    name: &str,
) -> eyre::Result<()> {
    strategy.save_checkpoint(engine.get_enabled_rule_ids())?;

    let Some(dir) = data_dir else {
        return Ok(());
    };
    for rule in engine.rules() {
        if !rule.has_cache() {
            continue;
        }
        let erased_cache = rule.fini_cache()?;
        let rule_cache =
            crate::cache::RuleCache::new_for_rule(dir, name, rule.name(), erased_cache);
        rule_cache.save()?;
    }

    Ok(())
}

/// Apply the SuppressAndContinue directive: suppress old descriptors, early-finalize
/// fully-inactive rules, and remove them from the engine.
fn apply_suppress_and_continue(
    engine: &mut RuleEngine,
    rule_ids_to_suppress: &[usize],
) -> Vec<Achievement> {
    engine.suppress_descriptors(rule_ids_to_suppress);
    let achievements = engine.finalize_inactive_rules();
    engine.retain_active_rules();
    achievements
}

#[cfg(test)]
mod tests {
    use herostratus_tests::fixtures;

    use super::*;
    use crate::achievement::AchievementDescriptor;
    use crate::rules::test_rules::{
        AlwaysFail, FlexibleRule, ParticipationTrophy, ParticipationTrophy2,
    };

    /// Helper to collect achievements via callback into a Vec
    fn collect_achievements(
        reference: &str,
        repo: &gix::Repository,
        data_dir: Option<&Path>,
        rules: Vec<Box<dyn RulePlugin>>,
    ) -> (Vec<Achievement>, GrantStats) {
        let mut achievements = Vec::new();
        let stats = grant_with_rules(reference, repo, None, data_dir, "", rules, |a| {
            achievements.push(a);
        })
        .unwrap();
        (achievements, stats)
    }

    #[test]
    fn test_no_rules() {
        let temp_repo = fixtures::repository::simplest().unwrap();
        let (achievements, _) = collect_achievements("HEAD", &temp_repo.repo, None, Vec::new());
        assert!(achievements.is_empty());
    }

    #[test]
    fn test_no_matches() {
        let temp_repo = fixtures::repository::simplest().unwrap();
        let rules = vec![Box::new(AlwaysFail) as Box<dyn RulePlugin>];
        let (achievements, _) = collect_achievements("HEAD", &temp_repo.repo, None, rules);
        assert!(achievements.is_empty());
    }

    #[test]
    fn test_all_matches() {
        let temp_repo = fixtures::repository::simplest().unwrap();
        let rules = vec![
            Box::new(AlwaysFail) as Box<dyn RulePlugin>,
            Box::new(ParticipationTrophy) as Box<dyn RulePlugin>,
        ];
        let (achievements, _) = collect_achievements("HEAD", &temp_repo.repo, None, rules);
        assert_eq!(achievements.len(), 1);
    }

    #[test]
    fn test_awards_on_finalize() {
        let temp_repo = fixtures::repository::simplest().unwrap();
        let rules = vec![Box::new(ParticipationTrophy2::default()) as Box<dyn RulePlugin>];
        let (achievements, stats) = collect_achievements("HEAD", &temp_repo.repo, None, rules);
        assert_eq!(achievements.len(), 1);
        assert_eq!(stats.num_achievements_generated, 1);
        assert_eq!(stats.num_commits_processed, 1);
    }

    #[test]
    fn test_early_exit_no_new_rules() {
        let temp_repo = fixtures::repository::simplest().unwrap();

        let rules = vec![
            Box::new(AlwaysFail) as Box<dyn RulePlugin>,
            Box::new(ParticipationTrophy) as Box<dyn RulePlugin>,
        ];
        let (achievements, _) =
            collect_achievements("HEAD", &temp_repo.repo, Some(temp_repo.path()), rules);
        assert_eq!(achievements.len(), 1);

        // Run the same rules again on the same repo; should early exit without granting any new
        // achievements.
        let rules = vec![
            Box::new(AlwaysFail) as Box<dyn RulePlugin>,
            Box::new(ParticipationTrophy) as Box<dyn RulePlugin>,
        ];
        let (achievements, _) =
            collect_achievements("HEAD", &temp_repo.repo, Some(temp_repo.path()), rules);
        assert!(achievements.is_empty());

        // Add a new commit to the repo; will generate a single new achievement
        let new_commit =
            fixtures::repository::add_empty_commit(&temp_repo.repo, "new-commit").unwrap();
        let rules = vec![
            Box::new(AlwaysFail) as Box<dyn RulePlugin>,
            Box::new(ParticipationTrophy) as Box<dyn RulePlugin>,
        ];
        let (achievements, _) =
            collect_achievements("HEAD", &temp_repo.repo, Some(temp_repo.path()), rules);
        assert_eq!(achievements.len(), 1);
        assert_eq!(achievements[0].commit, new_commit);
    }

    #[test]
    fn test_continue_processing_with_new_rules() {
        let temp_repo = fixtures::repository::simplest().unwrap();
        let first_commit = crate::git::rev::parse("HEAD", &temp_repo.repo).unwrap();

        let rules = vec![
            Box::new(AlwaysFail) as Box<dyn RulePlugin>,          // 1
            Box::new(ParticipationTrophy) as Box<dyn RulePlugin>, // 2
        ];
        let (achievements, _) =
            collect_achievements("HEAD", &temp_repo.repo, Some(temp_repo.path()), rules);
        assert_eq!(achievements.len(), 1);

        // Add a new commit to the repo
        let second_commit =
            fixtures::repository::add_empty_commit(&temp_repo.repo, "new-commit").unwrap();

        // Add a new rule; the new rule should process all commits; the old rules should only
        // process the newly added commit.
        let rules = vec![
            Box::new(AlwaysFail) as Box<dyn RulePlugin>, // 1
            Box::new(FlexibleRule {
                descriptors: vec![AchievementDescriptor {
                    id: 2,
                    human_id: "participation-trophy",
                    name: "first instance",
                    description: "This rule always grants an achievement",
                }],
            }) as Box<dyn RulePlugin>,
            Box::new(FlexibleRule {
                descriptors: vec![AchievementDescriptor {
                    id: 3,
                    human_id: "participation-trophy-2",
                    name: "second instance",
                    description: "This rule always grants an achievement",
                }],
            }) as Box<dyn RulePlugin>,
        ];

        let (achievements, _) =
            collect_achievements("HEAD", &temp_repo.repo, Some(temp_repo.path()), rules);
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

        // First run: FlexibleRule only has descriptor id=2
        let rules = vec![
            Box::new(AlwaysFail) as Box<dyn RulePlugin>, // 1
            Box::new(FlexibleRule {
                descriptors: vec![AchievementDescriptor {
                    id: 2,
                    human_id: "rule1",
                    name: "rule1",
                    description: "first rule descriptor",
                }],
            }) as Box<dyn RulePlugin>,
        ];
        let (granted, _) =
            collect_achievements("HEAD", &temp_repo.repo, Some(temp_repo.path()), rules);
        assert_eq!(granted.len(), 1);
        assert_eq!(granted[0].commit, first_commit);
        assert_eq!(granted[0].name, "rule1");

        // Add a new commit, and a new AchievementDescriptor to the existing RulePlugin implementation
        let second_commit =
            fixtures::repository::add_empty_commit(&temp_repo.repo, "new-commit").unwrap();
        // Second run: FlexibleRule now has both descriptor id=2 and id=3
        let rules = vec![
            Box::new(AlwaysFail) as Box<dyn RulePlugin>, // 1
            Box::new(FlexibleRule {
                descriptors: vec![
                    AchievementDescriptor {
                        id: 2,
                        human_id: "rule1",
                        name: "rule1",
                        description: "first rule descriptor",
                    },
                    AchievementDescriptor {
                        id: 3,
                        human_id: "rule2",
                        name: "rule2",
                        description: "second rule descriptor",
                    },
                ],
            }) as Box<dyn RulePlugin>,
        ];
        let (achievements, _) =
            collect_achievements("HEAD", &temp_repo.repo, Some(temp_repo.path()), rules);

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
