use std::collections::HashSet;
use std::time::{Duration, Instant};

use crate::achievement::Achievement;
use crate::git::mailmap::MailmapResolver;
use crate::observer::observer_factory::builtin_observers;
use crate::observer::{ObserverData, ObserverEngine};
use crate::rules::rule_plugin::RulePlugin;
use crate::rules::{RuleEngine, RuleOutput};

/// Drives the [ObserverEngine] and [RuleEngine] together, streaming [Achievements] via a callback.
struct Pipeline<'repo> {
    observer_engine: ObserverEngine<'repo>,
    rule_engine: RuleEngine,
    // Future fields:
    // checkpoint_strategy: CheckpointStrategy,
    // achievement_log: AchievementLog,
}

/// Statistics from a completed pipeline run.
#[cfg_attr(not(test), expect(dead_code))]
struct PipelineStats {
    num_commits_processed: u64,
    num_achievements: u64,
    #[cfg_attr(test, expect(dead_code))]
    elapsed: Duration,
}

#[cfg_attr(not(test), expect(dead_code))]
impl<'repo> Pipeline<'repo> {
    /// Build a pipeline, wiring observers to rules via their observation dependencies.
    ///
    /// Only instantiates observers whose `emits()` discriminant is consumed by at least one rule.
    pub fn new(
        repo: &'repo gix::Repository,
        rules: Vec<Box<dyn RulePlugin>>,
        mailmap: MailmapResolver,
    ) -> eyre::Result<Self> {
        let needed: HashSet<_> = rules.iter().flat_map(|r| r.consumes()).copied().collect();
        let observers: Vec<_> = builtin_observers()
            .into_iter()
            .filter(|obs| needed.contains(&obs.emits()))
            .collect();

        let observer_engine = ObserverEngine::new(repo, observers, mailmap)?;
        let rule_engine = RuleEngine::new(rules);

        Ok(Self {
            observer_engine,
            rule_engine,
        })
    }

    /// Process all commits and stream achievements to the callback.
    ///
    /// Consumes the pipeline since it is a one-shot operation.
    pub fn run(
        mut self,
        oids: impl IntoIterator<Item = gix::ObjectId>,
        mut on_achievement: impl FnMut(Achievement),
    ) -> eyre::Result<PipelineStats> {
        let start = Instant::now();
        let mut num_commits: u64 = 0;
        let mut num_achievements: u64 = 0;

        // EXTENSION POINT: rule engine init_cache

        for oid in oids {
            // TODO: This runs the git diff directly in this call graph. For performance's sake, we
            // may want to run the ObserverEngine on a separate thread, or possibly run the diffs
            // on a dedicated thread within the ObserverEngine. There might be pipelining benefits
            // to keep the diff computation unblocked by the rule processing. But this would be
            // really tricky to implement when we consider the CheckpointStrategy, which requires
            // disabling Observers and Rules when we hit a checkpoint. That doesn't preclude
            // parallelism, but it does make it trickier.
            let data = self.observer_engine.process_commit(oid)?;
            num_commits += 1;
            for msg in data {
                match msg {
                    ObserverData::CommitStart(ctx) => {
                        self.rule_engine.on_commit_start(ctx);
                    }
                    ObserverData::Observation(obs) => {
                        self.rule_engine.on_observation(&obs);
                    }
                    ObserverData::CommitComplete => {
                        for output in self.rule_engine.on_commit_complete() {
                            // EXTENSION POINT: achievement_log variation enforcement
                            Self::emit(output, &mut on_achievement, &mut num_achievements);
                        }
                        // EXTENSION POINT: checkpoint strategy
                    }
                }
            }
        }

        for output in self.rule_engine.finalize() {
            // EXTENSION POINT: achievement_log variation enforcement
            Self::emit(output, &mut on_achievement, &mut num_achievements);
        }

        // EXTENSION POINT: rule engine fini_cache
        // EXTENSION POINT: meta-achievements

        Ok(PipelineStats {
            num_commits_processed: num_commits,
            num_achievements,
            elapsed: start.elapsed(),
        })
    }

    /// Convert a RuleOutput into an Achievement and deliver it via the callback.
    ///
    /// Future: the AchievementLog will filter/transform RuleOutputs before converting.
    fn emit(
        output: RuleOutput,
        on_achievement: &mut impl FnMut(Achievement),
        num_achievements: &mut u64,
    ) {
        let achievement = Achievement {
            descriptor_id: output.meta.id,
            name: output.meta.name,
            commit: output.grant.commit,
            author_name: output.grant.author_name,
            author_email: output.grant.author_email,
        };
        on_achievement(achievement);
        *num_achievements += 1;
    }
}

#[cfg(test)]
mod tests {
    use herostratus_tests::fixtures::repository;

    use super::*;
    use crate::config::RulesConfig;
    use crate::rules::rule_plugin::builtin_rules;

    fn default_mailmap() -> MailmapResolver {
        MailmapResolver::new(gix::mailmap::Snapshot::default(), None, None).unwrap()
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

        let pipeline = Pipeline::new(&temp_repo.repo, Vec::new(), default_mailmap()).unwrap();

        let mut achievements = Vec::new();
        let stats = pipeline.run(oids, |a| achievements.push(a)).unwrap();

        assert!(achievements.is_empty());
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

        let pipeline = Pipeline::new(&temp_repo.repo, rules, default_mailmap()).unwrap();

        let mut achievements = Vec::new();
        let stats = pipeline
            .run(std::iter::once(oid), |a| achievements.push(a))
            .unwrap();

        // H001 (fixup) should fire on the per-commit path
        let fixup_achievements: Vec<_> = achievements
            .iter()
            .filter(|a| a.descriptor_id == 1)
            .collect();
        assert!(
            !fixup_achievements.is_empty(),
            "expected H001 fixup achievement, got: {achievements:?}"
        );
        assert_eq!(stats.num_commits_processed, 1);
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
        let pipeline = Pipeline::new(&temp_repo.repo, rules, default_mailmap()).unwrap();

        let mut achievements = Vec::new();
        let stats = pipeline.run(oids, |a| achievements.push(a)).unwrap();

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
}
