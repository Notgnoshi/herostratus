use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use eyre::WrapErr;

use super::achievement_log::AchievementLog;
use super::pipeline_checkpoint::{CheckpointAction, Continuation, PipelineCheckpoint};
use crate::achievement::{Achievement, AchievementEvent};
use crate::cache::{CheckpointCache, RuleCache};
use crate::config::Config;
use crate::git::mailmap::MailmapResolver;
use crate::observer::{ObserverData, ObserverEngine, builtin_observers};
use crate::rules::{RuleEngine, RuleOutput, RulePlugin};

pub struct GrantStats {
    pub num_commits_processed: u64,
    pub num_achievements_generated: u64,
    pub elapsed: Duration,
}

#[allow(clippy::too_many_arguments)]
#[tracing::instrument(target = "perf", skip(repo, on_event))]
pub fn grant(
    config: Option<&Config>,
    reference: &str,
    repo: &mut gix::Repository,
    depth: Option<usize>,
    data_dir: Option<&Path>,
    name: &str,
    repo_config: Option<&crate::config::RepositoryConfig>,
    on_event: impl FnMut(AchievementEvent),
) -> eyre::Result<GrantStats> {
    let default_rc = crate::config::RulesConfig::default();
    let rules_config = config.and_then(|c| c.rules.as_ref()).unwrap_or(&default_rc);
    let rules = crate::rules::builtin_rules(rules_config);

    let global_mailmap = config.and_then(|c| c.mailmap_file.as_deref());
    let repo_mailmap = config
        .and_then(|c| c.repositories.get(name))
        .and_then(|rc| rc.mailmap_file.as_deref());

    let snapshot = repo.open_mailmap();
    let mailmap = MailmapResolver::new(snapshot, global_mailmap, repo_mailmap)?;

    run_grant(
        reference,
        repo,
        depth,
        data_dir,
        name,
        rules,
        mailmap,
        repo_config,
        on_event,
    )
}

#[allow(clippy::too_many_arguments)]
fn run_grant(
    reference: &str,
    repo: &mut gix::Repository,
    depth: Option<usize>,
    data_dir: Option<&Path>,
    name: &str,
    rules: Vec<Box<dyn RulePlugin>>,
    mailmap: MailmapResolver,
    repo_config: Option<&crate::config::RepositoryConfig>,
    on_event: impl FnMut(AchievementEvent),
) -> eyre::Result<GrantStats> {
    let rev = crate::git::rev::parse(reference, repo)
        .wrap_err(format!("Failed to rev-parse: {reference:?}"))?;

    if let Some(data_dir) = data_dir {
        super::export::write_achievements_csv(data_dir, &rules)?;
    }

    // Build the pipeline first (briefly borrows &repo, but ObserverEngine clones into owned
    // storage, so the borrow does not persist after construction).
    let pipeline = Pipeline::new(repo, rules, mailmap, data_dir, name)?;

    // Choose the iteration strategy based on whether we have a repo config and the repo is
    // shallow. DeepeningRevWalk transparently fetches more history as needed; for non-shallow
    // repos (or stateless mode without a config) we use the plain rev::walk.
    if let Some(rc) = repo_config
        && repo.is_shallow()
    {
        let batch_size = crate::git::clone::DEFAULT_SHALLOW_DEPTH;
        let oids = crate::git::deepen::DeepeningRevWalk::new(rev, repo, rc.clone(), batch_size)?;
        if let Some(depth) = depth {
            let stats = pipeline.run(oids.take(depth), on_event)?;
            Ok(map_stats(stats))
        } else {
            let stats = pipeline.run(oids, on_event)?;
            Ok(map_stats(stats))
        }
    } else {
        let oids = crate::git::rev::walk(rev, repo)
            .wrap_err(format!("Failed to rev-walk rev: {rev:?}"))?;
        // Wrap Ok values and skip errors (existing behavior for stateless mode)
        let oids = oids.filter_map(|o| match o {
            Ok(o) => Some(Ok(o)),
            Err(e) => {
                tracing::error!("Skipping OID: {e:?}");
                None
            }
        });
        if let Some(depth) = depth {
            let stats = pipeline.run(oids.take(depth), on_event)?;
            Ok(map_stats(stats))
        } else {
            let stats = pipeline.run(oids, on_event)?;
            Ok(map_stats(stats))
        }
    }
}

fn map_stats(stats: PipelineStats) -> GrantStats {
    GrantStats {
        num_commits_processed: stats.num_commits_processed,
        num_achievements_generated: stats.num_achievements,
        elapsed: stats.elapsed,
    }
}

/// Drives the [ObserverEngine] and [RuleEngine] together, streaming [Achievement]s via a callback.
struct Pipeline {
    observer_engine: ObserverEngine,
    rule_engine: RuleEngine,
    achievement_log: AchievementLog,
    checkpoint: PipelineCheckpoint,
    data_dir: Option<PathBuf>,
    repo_name: String,
}

struct CommitResult {
    num_achievements: u64,
    early_exit: bool,
}

/// Statistics from a completed pipeline run.
struct PipelineStats {
    num_commits_processed: u64,
    num_achievements: u64,
    elapsed: Duration,
}

impl Pipeline {
    /// Build a pipeline, wiring observers to rules via their observation dependencies.
    ///
    /// Only instantiates observers whose `emits()` discriminant is consumed by at least one rule.
    ///
    /// When `data_dir` is provided, rule caches are loaded before processing and saved afterward.
    /// Pass `None` for stateless operation (no persistence).
    pub fn new(
        repo: &gix::Repository,
        rules: Vec<Box<dyn RulePlugin>>,
        mailmap: MailmapResolver,
        data_dir: Option<&Path>,
        repo_name: &str,
    ) -> eyre::Result<Self> {
        let needed: HashSet<_> = rules.iter().flat_map(|r| r.consumes()).copied().collect();
        let observers: Vec<_> = builtin_observers()
            .into_iter()
            .filter(|obs| needed.contains(&obs.emits()))
            .collect();

        let observer_engine = ObserverEngine::new(repo, observers, mailmap)?;
        let rule_engine = RuleEngine::new(rules);

        let log_path = data_dir.map(|d| {
            d.join("export")
                .join("events")
                .join(format!("{repo_name}.csv"))
        });
        let achievement_log = AchievementLog::load(log_path.as_deref())?;

        let checkpoint = if let Some(dir) = data_dir {
            CheckpointCache::from_data_dir(dir, repo_name)?
        } else {
            CheckpointCache::in_memory()
        };
        let checkpoint = PipelineCheckpoint::new(checkpoint);

        Ok(Self {
            observer_engine,
            rule_engine,
            achievement_log,
            checkpoint,
            data_dir: data_dir.map(Path::to_path_buf),
            repo_name: repo_name.to_string(),
        })
    }

    /// Process all commits and stream achievements to the callback.
    ///
    /// Consumes the pipeline since it is a one-shot operation.
    #[tracing::instrument(target = "perf", skip_all)]
    pub fn run(
        mut self,
        oids: impl IntoIterator<Item = eyre::Result<gix::ObjectId>>,
        mut on_event: impl FnMut(AchievementEvent),
    ) -> eyre::Result<PipelineStats> {
        let start = Instant::now();
        let mut num_commits: u64 = 0;
        let mut num_achievements: u64 = 0;

        if let Some(data_dir) = &self.data_dir {
            let repo_name = &self.repo_name;
            self.rule_engine.init_caches(|human_id| {
                let cache = RuleCache::from_rule_name(data_dir, repo_name, human_id)?;
                Ok(cache.data)
            })?;
        }

        for oid in oids {
            let oid = oid?;
            let result = self.on_commit(oid, &mut on_event)?;
            num_achievements += result.num_achievements;
            if result.early_exit {
                break;
            }
            num_commits += 1;
        }

        tracing::debug!("Finalizing rules ...");
        let outputs = self.rule_engine.finalize();
        num_achievements += self.emit(outputs, &mut on_event);

        tracing::debug!("Evaluating meta-achievements ...");
        let meta_outputs = super::meta_achievements::evaluate(&self.achievement_log);
        num_achievements += self.emit(meta_outputs, &mut on_event);

        self.checkpoint
            .save_checkpoint(self.rule_engine.active_rules())?;

        if let Some(data_dir) = &self.data_dir {
            let repo_name = &self.repo_name;
            self.rule_engine.fini_caches(|human_id, data| {
                let cache = RuleCache::new_for_rule(data_dir, repo_name, human_id, data);
                cache.save()
            })?;
        }

        self.achievement_log.save()?;

        let elapsed = start.elapsed();
        tracing::info!(
            "Generated {num_achievements} achievements after processing {num_commits} commits in {elapsed:?}"
        );

        Ok(PipelineStats {
            num_commits_processed: num_commits,
            num_achievements,
            elapsed,
        })
    }

    /// Process a single commit: checkpoint decision, observer dispatch, and rule evaluation.
    ///
    /// Returns whether the loop should exit early and how many achievements were emitted.
    #[tracing::instrument(target = "perf", name = "Pipeline::on_commit", skip_all)]
    fn on_commit(
        &mut self,
        oid: gix::ObjectId,
        on_event: &mut impl FnMut(AchievementEvent),
    ) -> eyre::Result<CommitResult> {
        let mut num_achievements = 0;

        match self.checkpoint.on_commit(oid) {
            Continuation::Process => {}
            Continuation::ReachedCheckpoint => {
                match self.checkpoint.resolve(&self.rule_engine.active_rules()) {
                    CheckpointAction::EarlyExit => {
                        return Ok(CommitResult {
                            num_achievements: 0,
                            early_exit: true,
                        });
                    }
                    CheckpointAction::Retire { rule_ids } => {
                        let data_dir = &self.data_dir;
                        let repo_name = &self.repo_name;
                        let outputs = self.rule_engine.retire(&rule_ids, |human_id, data| {
                            save_rule_cache(data_dir, repo_name, human_id, data)
                        })?;
                        num_achievements += self.emit(outputs, on_event);
                        self.observer_engine
                            .retire_all_except(&self.rule_engine.consumed());
                    }
                }
            }
        }

        // TODO: This runs the git diff directly in this call graph. For performance's sake, we
        // may want to run the ObserverEngine on a separate thread, or possibly run the diffs
        // on a dedicated thread within the ObserverEngine. There might be pipelining benefits
        // to keep the diff computation unblocked by the rule processing. But this would be
        // really tricky to implement when we consider the checkpoint, which requires retiring
        // Observers and Rules when we hit a checkpoint. That doesn't preclude parallelism,
        // but it does make it trickier.
        let data = self.observer_engine.process_commit(oid)?;
        let _guard = tracing::info_span!(target: "perf", "RuleEngine::on_commit").entered();
        for msg in data {
            match msg {
                ObserverData::CommitStart(ctx) => {
                    self.rule_engine.on_commit_start(ctx);
                }
                ObserverData::Observation(obs) => {
                    self.rule_engine.on_observation(&obs);
                }
                ObserverData::CommitComplete => {
                    let outputs = self.rule_engine.on_commit_complete();
                    num_achievements += self.emit(outputs, on_event);
                }
            }
        }

        Ok(CommitResult {
            num_achievements,
            early_exit: false,
        })
    }

    /// Resolve rule outputs through the achievement log and emit events via the callback.
    ///
    /// Revocations are emitted before the corresponding grant.
    fn emit(
        &mut self,
        outputs: Vec<RuleOutput>,
        on_event: &mut impl FnMut(AchievementEvent),
    ) -> u64 {
        let mut count = 0;
        for output in outputs {
            let name_override = output.grant.name_override.clone();
            let description_override = output.grant.description_override.clone();
            if let Some(resolution) = self.achievement_log.resolve(&output.meta, output.grant) {
                if let Some(ref revoke) = resolution.revoke {
                    let achievement = Achievement {
                        descriptor_id: output.meta.id,
                        human_id: output.meta.human_id,
                        name: output.meta.name.to_string(),
                        description: output.meta.description.to_string(),
                        commit: revoke.commit,
                        user_name: revoke.user_name.clone(),
                        user_email: revoke.user_email.clone(),
                    };
                    tracing::info!(
                        "revoked achievement: {:?} from {}",
                        achievement.name,
                        achievement.user_email
                    );
                    on_event(AchievementEvent::Revoke(achievement));
                }

                let achievement = Achievement {
                    descriptor_id: output.meta.id,
                    human_id: output.meta.human_id,
                    name: name_override.unwrap_or_else(|| output.meta.name.to_string()),
                    description: description_override
                        .unwrap_or_else(|| output.meta.description.to_string()),
                    commit: resolution.grant.commit,
                    user_name: resolution.grant.user_name,
                    user_email: resolution.grant.user_email,
                };
                tracing::info!(
                    "granted achievement: {:?} to {:?} for commit {}",
                    achievement.name,
                    achievement.user_name,
                    achievement.commit
                );
                on_event(AchievementEvent::Grant(achievement));
                count += 1;
            }
        }
        count
    }
}

fn save_rule_cache(
    data_dir: &Option<PathBuf>,
    repo_name: &str,
    human_id: &str,
    data: serde_json::Value,
) -> eyre::Result<()> {
    if let Some(data_dir) = data_dir {
        let cache = RuleCache::new_for_rule(data_dir, repo_name, human_id, data);
        cache.save()?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use herostratus_tests::fixtures::repository;

    use super::*;
    use crate::config::RulesConfig;
    use crate::rules::builtin_rules;

    fn default_mailmap() -> MailmapResolver {
        MailmapResolver::new(gix::mailmap::Snapshot::default(), None, None).unwrap()
    }

    /// Extract only the granted [Achievement]s from a list of events.
    fn grants(events: &[AchievementEvent]) -> Vec<&Achievement> {
        events
            .iter()
            .filter_map(|e| match e {
                AchievementEvent::Grant(a) => Some(a),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn commits_but_no_rules() {
        let temp_repo = repository::Builder::new()
            .commit("first")
            .commit("second")
            .build()
            .unwrap();

        let head = crate::git::rev::parse("HEAD", &temp_repo.repo).unwrap();
        let oids: Vec<_> = crate::git::rev::walk(head, &temp_repo.repo)
            .unwrap()
            .map(|r| r.unwrap())
            .collect();

        let pipeline =
            Pipeline::new(&temp_repo.repo, Vec::new(), default_mailmap(), None, "").unwrap();

        let mut events = Vec::new();
        let stats = pipeline
            .run(oids.into_iter().map(Ok), |e| events.push(e))
            .unwrap();

        assert!(events.is_empty());
        assert_eq!(stats.num_commits_processed, 2);
        assert_eq!(stats.num_achievements, 0);
    }

    #[test]
    fn end_to_end_fixup() {
        let temp_repo = repository::Builder::new()
            .commit("fixup! something")
            .build()
            .unwrap();

        let oid = crate::git::rev::parse("HEAD", &temp_repo.repo).unwrap();
        let rules = builtin_rules(&RulesConfig::default());

        let pipeline = Pipeline::new(&temp_repo.repo, rules, default_mailmap(), None, "").unwrap();

        let mut events = Vec::new();
        let stats = pipeline
            .run(std::iter::once(Ok(oid)), |e| events.push(e))
            .unwrap();

        // H001 (fixup) should fire on the per-commit path
        let achievements = grants(&events);
        let fixup_achievements: Vec<_> = achievements
            .iter()
            .filter(|a| a.descriptor_id == 1)
            .collect();
        assert!(
            !fixup_achievements.is_empty(),
            "expected H001 fixup achievement, got: {events:?}"
        );
        assert_eq!(stats.num_commits_processed, 1);
    }

    #[test]
    fn caches_are_persisted_and_loaded() {
        let data_dir = tempfile::tempdir().unwrap();

        // Run 1: commit with a short subject (2 chars, below H002 threshold of 10).
        // ShortestSubject should grant and save its cache with shortest_length=Some(2).
        let temp_repo = repository::Builder::new().commit("Hi").build().unwrap();
        let oid = crate::git::rev::parse("HEAD", &temp_repo.repo).unwrap();
        let rules = builtin_rules(&RulesConfig::default());

        let pipeline = Pipeline::new(
            &temp_repo.repo,
            rules,
            default_mailmap(),
            Some(data_dir.path()),
            "test-repo",
        )
        .unwrap();

        let mut events = Vec::new();
        pipeline
            .run(std::iter::once(Ok(oid)), |e| events.push(e))
            .unwrap();

        let h002_granted = grants(&events).iter().any(|a| a.descriptor_id == 2);
        assert!(h002_granted, "expected H002 on first run");

        // Verify the cache file was written
        let cache_path = data_dir
            .path()
            .join("cache/test-repo/rule_shortest-subject-line.json");
        assert!(cache_path.exists(), "cache file should exist after run");

        // Run 2: commit with subject "Hello" (5 chars, still below threshold 10).
        // With the loaded cache (shortest_length=2), 5 >= 2 so no new record -- no grant.
        let temp_repo2 = repository::Builder::new().commit("Hello").build().unwrap();
        let oid2 = crate::git::rev::parse("HEAD", &temp_repo2.repo).unwrap();
        let rules2 = builtin_rules(&RulesConfig::default());

        let pipeline2 = Pipeline::new(
            &temp_repo2.repo,
            rules2,
            default_mailmap(),
            Some(data_dir.path()),
            "test-repo",
        )
        .unwrap();

        let mut events2 = Vec::new();
        pipeline2
            .run(std::iter::once(Ok(oid2)), |e| events2.push(e))
            .unwrap();

        let h002_granted_again = grants(&events2).iter().any(|a| a.descriptor_id == 2);
        assert!(
            !h002_granted_again,
            "expected no H002 on second run (cache should suppress it)"
        );
    }

    #[test]
    fn achievement_log_deduplicates_per_user_across_runs() {
        let data_dir = tempfile::tempdir().unwrap();

        // Run 1: fixup commit triggers H001 (PerUser { recurrent: false })
        let temp_repo = repository::Builder::new()
            .commit("fixup! something")
            .build()
            .unwrap();
        let oid = crate::git::rev::parse("HEAD", &temp_repo.repo).unwrap();
        let rules = builtin_rules(&RulesConfig::default());

        let pipeline = Pipeline::new(
            &temp_repo.repo,
            rules,
            default_mailmap(),
            Some(data_dir.path()),
            "test-repo",
        )
        .unwrap();

        let mut events = Vec::new();
        pipeline
            .run(std::iter::once(Ok(oid)), |e| events.push(e))
            .unwrap();

        let h001_count = grants(&events)
            .iter()
            .filter(|a| a.descriptor_id == 1)
            .count();
        assert_eq!(h001_count, 1, "expected H001 on first run");

        // Run 2: another fixup commit by the same author. The achievement log should suppress it.
        let temp_repo2 = repository::Builder::new()
            .commit("fixup! another thing")
            .build()
            .unwrap();
        let oid2 = crate::git::rev::parse("HEAD", &temp_repo2.repo).unwrap();
        let rules2 = builtin_rules(&RulesConfig::default());

        let pipeline2 = Pipeline::new(
            &temp_repo2.repo,
            rules2,
            default_mailmap(),
            Some(data_dir.path()),
            "test-repo",
        )
        .unwrap();

        let mut events2 = Vec::new();
        pipeline2
            .run(std::iter::once(Ok(oid2)), |e| events2.push(e))
            .unwrap();

        let h001_count2 = grants(&events2)
            .iter()
            .filter(|a| a.descriptor_id == 1)
            .count();
        assert_eq!(
            h001_count2, 0,
            "expected no H001 on second run (achievement log should deduplicate)"
        );
    }

    /// End-to-end test exercising all four AchievementKind variants via profanity rules.
    ///
    /// - H7 (Global{revocable:false}): first profanity in the repo (oldest commit)
    /// - H8 (PerUser{recurrent:false}): one grant per author
    /// - H9 (PerUser{recurrent:true}): grants at threshold milestone
    /// - H10 (Global{revocable:true}): most profane author at finalize
    #[test]
    fn all_profanity_achievement_kinds() {
        // 6 profane commits: 5 from Alice, 1 from Bob.
        // Commit order (oldest to newest): 1(Bob), 2(Alice), 3(Alice), 4(Alice), 5(Alice), 6(Alice)
        // Walk order is newest-first: 6, 5, 4, 3, 2, 1.
        // Bob authored the oldest profane commit, so H7 should go to Bob.
        let temp_repo = repository::Builder::new()
            .commit("shit happens")
            .author("Bob", "bob@example.com")
            .commit("damn this code")
            .author("Alice", "alice@example.com")
            .commit("hell yeah")
            .author("Alice", "alice@example.com")
            .commit("piss off")
            .author("Alice", "alice@example.com")
            .commit("fucking bugs")
            .author("Alice", "alice@example.com")
            .commit("what a damn mess")
            .author("Alice", "alice@example.com")
            .build()
            .unwrap();

        let head = crate::git::rev::parse("HEAD", &temp_repo.repo).unwrap();
        let oids: Vec<_> = crate::git::rev::walk(head, &temp_repo.repo)
            .unwrap()
            .map(|r| r.unwrap())
            .collect();

        let rules = builtin_rules(&RulesConfig::default());
        let pipeline = Pipeline::new(&temp_repo.repo, rules, default_mailmap(), None, "").unwrap();

        let mut events = Vec::new();
        let stats = pipeline
            .run(oids.into_iter().map(Ok), |e| events.push(e))
            .unwrap();
        let achievements = grants(&events);

        // H7 (Global{revocable:false}): Bob is the actual first swearer (oldest commit)
        let h7: Vec<_> = achievements
            .iter()
            .filter(|a| a.descriptor_id == 7)
            .collect();
        assert_eq!(h7.len(), 1, "expected exactly one H7 grant: {h7:?}");
        assert_eq!(h7[0].user_email, "bob@example.com");

        // H8 (PerUser{recurrent:false}): one grant per author
        let h8: Vec<_> = achievements
            .iter()
            .filter(|a| a.descriptor_id == 8)
            .collect();
        assert_eq!(h8.len(), 2, "expected one H8 per author: {h8:?}");
        let h8_emails: HashSet<_> = h8.iter().map(|a| a.user_email.as_str()).collect();
        assert!(h8_emails.contains("alice@example.com"));
        assert!(h8_emails.contains("bob@example.com"));

        // H9 (PerUser{recurrent:true}): Alice hits threshold 5
        let h9: Vec<_> = achievements
            .iter()
            .filter(|a| a.descriptor_id == 9)
            .collect();
        assert_eq!(h9.len(), 1, "expected one H9 grant at threshold 5: {h9:?}");
        assert_eq!(h9[0].user_email, "alice@example.com");

        // H10 (Global{revocable:true}): Alice is the most profane at finalize
        let h10: Vec<_> = achievements
            .iter()
            .filter(|a| a.descriptor_id == 10)
            .collect();
        assert_eq!(h10.len(), 1, "expected one H10 grant: {h10:?}");
        assert_eq!(h10[0].user_email, "alice@example.com");

        assert_eq!(stats.num_commits_processed, 6);
    }

    /// Incremental test: two pipeline runs with shared persistence, verifying deduplication,
    /// permanence, cache interaction, and CSV round-trip across runs.
    ///
    /// Run 1: Alice swears 3 times (below H9 threshold). Gets H7, H8, H10.
    /// Run 2: Bob swears 10 times. Gets H8 (his first), H9 twice (thresholds 5 and 10),
    ///        H10 (supersedes Alice). H7 stays with Alice permanently. CSV verified.
    #[test]
    fn incremental_profanity_runs() {
        let data_dir = tempfile::tempdir().unwrap();

        // -- Run 1: Alice swears 3 times --
        let temp_repo1 = repository::Builder::new()
            .commit("damn this code")
            .author("Alice", "alice@example.com")
            .commit("shit happens")
            .author("Alice", "alice@example.com")
            .commit("hell yeah")
            .author("Alice", "alice@example.com")
            .build()
            .unwrap();

        let head1 = crate::git::rev::parse("HEAD", &temp_repo1.repo).unwrap();
        let oids1: Vec<_> = crate::git::rev::walk(head1, &temp_repo1.repo)
            .unwrap()
            .map(|r| r.unwrap())
            .collect();

        let rules1 = builtin_rules(&RulesConfig::default());
        let pipeline1 = Pipeline::new(
            &temp_repo1.repo,
            rules1,
            default_mailmap(),
            Some(data_dir.path()),
            "test-repo",
        )
        .unwrap();

        let mut events1 = Vec::new();
        pipeline1
            .run(oids1.into_iter().map(Ok), |e| events1.push(e))
            .unwrap();
        let achievements1 = grants(&events1);

        assert!(
            achievements1.iter().any(|a| a.descriptor_id == 7),
            "expected H7 in run 1"
        );
        assert!(
            achievements1.iter().any(|a| a.descriptor_id == 8),
            "expected H8 in run 1"
        );
        assert!(
            !achievements1.iter().any(|a| a.descriptor_id == 9),
            "unexpected H9 in run 1 (only 3 profanities, threshold is 5)"
        );
        assert!(
            achievements1.iter().any(|a| a.descriptor_id == 10),
            "expected H10 in run 1"
        );

        // -- Run 2: Bob swears 10 times (new repo, same data_dir for shared caches/log) --
        let temp_repo2 = repository::Builder::new()
            .commit("damn thing")
            .author("Bob", "bob@example.com")
            .commit("shit code")
            .author("Bob", "bob@example.com")
            .commit("hell no")
            .author("Bob", "bob@example.com")
            .commit("piss off")
            .author("Bob", "bob@example.com")
            .commit("fucking mess")
            .author("Bob", "bob@example.com")
            .commit("bastard bug")
            .author("Bob", "bob@example.com")
            .commit("damn it again")
            .author("Bob", "bob@example.com")
            .commit("shit sandwich")
            .author("Bob", "bob@example.com")
            .commit("hell frozen over")
            .author("Bob", "bob@example.com")
            .commit("bitch please")
            .author("Bob", "bob@example.com")
            .build()
            .unwrap();

        let head2 = crate::git::rev::parse("HEAD", &temp_repo2.repo).unwrap();
        let oids2: Vec<_> = crate::git::rev::walk(head2, &temp_repo2.repo)
            .unwrap()
            .map(|r| r.unwrap())
            .collect();

        let rules2 = builtin_rules(&RulesConfig::default());
        let pipeline2 = Pipeline::new(
            &temp_repo2.repo,
            rules2,
            default_mailmap(),
            Some(data_dir.path()),
            "test-repo",
        )
        .unwrap();

        let mut events2 = Vec::new();
        pipeline2
            .run(oids2.into_iter().map(Ok), |e| events2.push(e))
            .unwrap();
        let achievements2 = grants(&events2);

        // H7 (Global{revocable:false}): NOT granted to Bob -- Alice holds permanently
        assert!(
            !achievements2.iter().any(|a| a.descriptor_id == 7),
            "H7 should not be granted in run 2 (Alice holds permanently)"
        );

        // H8 (PerUser{recurrent:false}): Bob gets his first
        let h8: Vec<_> = achievements2
            .iter()
            .filter(|a| a.descriptor_id == 8)
            .collect();
        assert_eq!(h8.len(), 1, "expected one H8 for Bob in run 2: {h8:?}");
        assert_eq!(h8[0].user_email, "bob@example.com");

        // H9 (PerUser{recurrent:true}): Bob hits thresholds 5 and 10
        let h9: Vec<_> = achievements2
            .iter()
            .filter(|a| a.descriptor_id == 9)
            .collect();
        assert_eq!(
            h9.len(),
            2,
            "expected two H9 grants (thresholds 5 and 10): {h9:?}"
        );
        assert!(h9.iter().all(|a| a.user_email == "bob@example.com"));

        // H10 (Global{revocable:true}): Bob supersedes Alice (10 > 3)
        // Alice's H10 is revoked, then Bob gets the grant
        let revokes: Vec<_> = events2
            .iter()
            .filter_map(|e| match e {
                AchievementEvent::Revoke(a) if a.descriptor_id == 10 => Some(a),
                _ => None,
            })
            .collect();
        assert_eq!(revokes.len(), 1, "expected one H10 revocation: {revokes:?}");
        assert_eq!(revokes[0].user_email, "alice@example.com");

        let h10: Vec<_> = achievements2
            .iter()
            .filter(|a| a.descriptor_id == 10)
            .collect();
        assert_eq!(h10.len(), 1, "expected H10 for Bob in run 2: {h10:?}");
        assert_eq!(h10[0].user_email, "bob@example.com");
    }

    #[test]
    fn finalization_grants() {
        // Create commits with varying subject lengths to trigger H002 (shortest, threshold <10)
        // and H003 (longest, threshold >72).
        let temp_repo = repository::Builder::new()
            .commit("Hi")
            .commit(
                "This commit subject is deliberately very long to exceed the seventy-two \
                 character threshold for H003",
            )
            .build()
            .unwrap();

        let head = crate::git::rev::parse("HEAD", &temp_repo.repo).unwrap();
        let oids: Vec<_> = crate::git::rev::walk(head, &temp_repo.repo)
            .unwrap()
            .map(|r| r.unwrap())
            .collect();

        let rules = builtin_rules(&RulesConfig::default());
        let pipeline = Pipeline::new(&temp_repo.repo, rules, default_mailmap(), None, "").unwrap();

        let mut events = Vec::new();
        let stats = pipeline
            .run(oids.into_iter().map(Ok), |e| events.push(e))
            .unwrap();
        let achievements = grants(&events);

        let ids: Vec<_> = achievements.iter().map(|a| a.descriptor_id).collect();
        assert!(
            ids.contains(&2),
            "expected H002 (shortest subject), got: {ids:?}"
        );
        assert!(
            ids.contains(&3),
            "expected H003 (longest subject), got: {ids:?}"
        );
        assert_eq!(stats.num_commits_processed, 2);
    }

    /// Helper: build and run a pipeline over a repo, returning stats and events.
    fn run_pipeline(
        repo: &gix::Repository,
        data_dir: Option<&Path>,
        repo_name: &str,
    ) -> (PipelineStats, Vec<AchievementEvent>) {
        let head = crate::git::rev::parse("HEAD", repo).unwrap();
        let oids: Vec<_> = crate::git::rev::walk(head, repo)
            .unwrap()
            .map(|r| r.unwrap())
            .collect();
        let rules = builtin_rules(&RulesConfig::default());
        let pipeline = Pipeline::new(repo, rules, default_mailmap(), data_dir, repo_name).unwrap();
        let mut events = Vec::new();
        let stats = pipeline
            .run(oids.into_iter().map(Ok), |e| events.push(e))
            .unwrap();
        (stats, events)
    }

    #[test]
    fn checkpoint_early_exit() {
        let data_dir = tempfile::tempdir().unwrap();
        let temp_repo = repository::Builder::new()
            .commit("fixup! something")
            .commit("normal commit")
            .build()
            .unwrap();

        // Run 1: process all commits
        let (stats1, events1) = run_pipeline(&temp_repo.repo, Some(data_dir.path()), "test-repo");
        assert_eq!(stats1.num_commits_processed, 2);
        assert!(!events1.is_empty());

        // Run 2: same rules, same repo -- checkpoint should trigger early exit
        let (stats2, events2) = run_pipeline(&temp_repo.repo, Some(data_dir.path()), "test-repo");
        assert_eq!(stats2.num_commits_processed, 0);
        assert!(events2.is_empty());
    }

    #[test]
    fn checkpoint_new_commits_after_checkpoint() {
        let data_dir = tempfile::tempdir().unwrap();
        let temp_repo = repository::Builder::new()
            .commit("fixup! first")
            .build()
            .unwrap();

        // Run 1
        let (stats1, _) = run_pipeline(&temp_repo.repo, Some(data_dir.path()), "test-repo");
        assert_eq!(stats1.num_commits_processed, 1);

        // Add new commits
        temp_repo.commit("fixup! second").create().unwrap();
        temp_repo.commit("fixup! third").create().unwrap();

        // Run 2: only new commits should be processed (old checkpoint hit -> early exit for rest)
        let (stats2, _) = run_pipeline(&temp_repo.repo, Some(data_dir.path()), "test-repo");
        // Walk order: third, second, first(checkpoint). Processes third and second, then exits.
        assert_eq!(stats2.num_commits_processed, 2);
    }

    /// Helper: build and run a pipeline with a specific set of rules (filtered by ID).
    fn run_pipeline_with_rules(
        repo: &gix::Repository,
        data_dir: Option<&Path>,
        repo_name: &str,
        rule_ids: &[usize],
    ) -> (PipelineStats, Vec<AchievementEvent>) {
        let head = crate::git::rev::parse("HEAD", repo).unwrap();
        let oids: Vec<_> = crate::git::rev::walk(head, repo)
            .unwrap()
            .map(|r| r.unwrap())
            .collect();
        let rules: Vec<_> = builtin_rules(&RulesConfig::default())
            .into_iter()
            .filter(|r| rule_ids.contains(&r.meta().id))
            .collect();
        let pipeline = Pipeline::new(repo, rules, default_mailmap(), data_dir, repo_name).unwrap();
        let mut events = Vec::new();
        let stats = pipeline
            .run(oids.into_iter().map(Ok), |e| events.push(e))
            .unwrap();
        (stats, events)
    }

    #[test]
    fn checkpoint_retire_and_continue() {
        let data_dir = tempfile::tempdir().unwrap();

        // Create a repo with a fixup commit (triggers H001) and a short subject (triggers H002
        // at finalize).
        let temp_repo = repository::Builder::new()
            .commit("fixup! something")
            .commit("Hi")
            .build()
            .unwrap();

        // Run 1: only H001 (fixup)
        let (stats1, events1) =
            run_pipeline_with_rules(&temp_repo.repo, Some(data_dir.path()), "test-repo", &[1]);
        assert_eq!(stats1.num_commits_processed, 2);
        assert!(
            grants(&events1).iter().any(|a| a.descriptor_id == 1),
            "expected H001 in run 1"
        );

        // Run 2: H001 + H002 (shortest subject). The checkpoint should retire H001 and
        // continue processing all commits with just H002.
        let (stats2, events2) =
            run_pipeline_with_rules(&temp_repo.repo, Some(data_dir.path()), "test-repo", &[1, 2]);
        // All commits re-processed for the new rule H002
        assert_eq!(stats2.num_commits_processed, 2);
        // H002 should fire (shortest subject "Hi" is 2 chars, below threshold 10)
        assert!(
            grants(&events2).iter().any(|a| a.descriptor_id == 2),
            "expected H002 in run 2: {events2:?}"
        );
        // H001 should NOT fire again (it was retired, not re-processed)
        assert!(
            !grants(&events2).iter().any(|a| a.descriptor_id == 1),
            "H001 should not fire in run 2 (retired at checkpoint)"
        );
    }

    #[test]
    fn checkpoint_saves_rule_caches_on_retire() {
        let data_dir = tempfile::tempdir().unwrap();

        // Create a repo with short subjects to exercise H002 (ShortestSubject, has cache)
        let temp_repo = repository::Builder::new().commit("Hi").build().unwrap();

        // Run 1: only H002
        run_pipeline_with_rules(&temp_repo.repo, Some(data_dir.path()), "test-repo", &[2]);

        // Verify H002 cache was written
        let cache_path = data_dir
            .path()
            .join("cache/test-repo/rule_shortest-subject-line.json");
        assert!(cache_path.exists(), "cache file should exist after run 1");

        // Remove the cache file to verify that retire re-saves it
        std::fs::remove_file(&cache_path).unwrap();

        // Run 2: H002 + H001. H002 is retired at checkpoint, which should save its cache.
        run_pipeline_with_rules(&temp_repo.repo, Some(data_dir.path()), "test-repo", &[1, 2]);

        assert!(
            cache_path.exists(),
            "cache file should be re-saved when H002 is retired at checkpoint"
        );
    }

    #[test]
    fn checkpoint_not_saved_without_data_dir() {
        let temp_repo = repository::Builder::new()
            .commit("fixup! something")
            .build()
            .unwrap();

        // Run 1: no data_dir
        let (stats1, _) = run_pipeline(&temp_repo.repo, None, "test-repo");
        assert_eq!(stats1.num_commits_processed, 1);

        // Run 2: still no data_dir -- no checkpoint was saved, so all commits are processed again
        let (stats2, _) = run_pipeline(&temp_repo.repo, None, "test-repo");
        assert_eq!(stats2.num_commits_processed, 1);
    }
}
