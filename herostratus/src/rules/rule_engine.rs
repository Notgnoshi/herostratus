use crate::achievement::{Grant, Meta};
use crate::observer::{CommitContext, Observation};
use crate::rules::rule_plugin::RulePlugin;

/// A grant paired with the rule's [Meta] that produced it.
///
/// The orchestration layer uses `meta.human_id` and `meta.kind` for variation
/// enforcement via the AchievementLog.
pub(crate) struct RuleOutput {
    pub meta: Meta,
    pub grant: Grant,
}

/// Dispatches observations to [RulePlugin]s and collects grants.
///
/// The RuleEngine is a mechanical dispatcher -- it runs rules, collects grants, and returns them.
/// It does not interpret grants, enforce variation semantics, or write to the AchievementLog.
///
/// The orchestration layer drives the engine by calling [on_commit_start](Self::on_commit_start),
/// [on_observation](Self::on_observation), and [on_commit_complete](Self::on_commit_complete) as
/// it matches on the channel receiver.
pub(crate) struct RuleEngine {
    rules: Vec<Box<dyn RulePlugin>>,
    current_ctx: Option<crate::observer::CommitContext>,
    pending: Vec<RuleOutput>,
}

impl RuleEngine {
    pub fn new(rules: Vec<Box<dyn RulePlugin>>) -> Self {
        Self {
            rules,
            current_ctx: None,
            pending: Vec::new(),
        }
    }

    /// Begin a new commit
    pub fn on_commit_start(&mut self, ctx: CommitContext) {
        self.current_ctx = Some(ctx);
        let ctx = self.current_ctx.as_ref().unwrap();
        for rule in &mut self.rules {
            if let Err(e) = rule.commit_start(ctx) {
                tracing::warn!(rule = rule.meta().human_id, "commit_start failed: {e}");
            }
        }
    }

    /// Dispatch a single observation to all rules
    pub fn on_observation(&mut self, obs: &Observation) {
        let ctx = self
            .current_ctx
            .as_ref()
            .expect("on_observation before on_commit_start");
        for rule in &mut self.rules {
            match rule.process(ctx, obs) {
                Ok(Some(grant)) => {
                    self.pending.push(RuleOutput {
                        meta: rule.meta().clone(),
                        grant,
                    });
                }
                Ok(None) => {}
                Err(e) => {
                    tracing::warn!(rule = rule.meta().human_id, "process failed: {e}");
                }
            }
        }
    }

    /// Complete the current commit
    pub fn on_commit_complete(&mut self) -> Vec<RuleOutput> {
        let ctx = self
            .current_ctx
            .as_ref()
            .expect("on_commit_complete before on_commit_start");
        for rule in &mut self.rules {
            match rule.commit_complete(ctx) {
                Ok(Some(grant)) => {
                    self.pending.push(RuleOutput {
                        meta: rule.meta().clone(),
                        grant,
                    });
                }
                Ok(None) => {}
                Err(e) => {
                    tracing::warn!(rule = rule.meta().human_id, "commit_complete failed: {e}");
                }
            }
        }
        self.current_ctx = None;
        self.collect_pending()
    }

    /// Call finalize on all rules and return their grants.
    pub fn finalize(&mut self) -> Vec<RuleOutput> {
        for rule in &mut self.rules {
            match rule.finalize() {
                Ok(Some(grant)) => {
                    self.pending.push(RuleOutput {
                        meta: rule.meta().clone(),
                        grant,
                    });
                }
                Ok(None) => {}
                Err(e) => {
                    tracing::warn!(rule = rule.meta().human_id, "finalize failed: {e}");
                }
            }
        }
        self.collect_pending()
    }

    /// Finalize the specified rules, remove them from the engine, and return their grants.
    ///
    /// Used at checkpoint boundaries: when the checkpoint system determines that certain rules are
    /// satisfied, the orchestration layer retires them in one step, flushing any cached state into
    /// final grants and then dropping the rules entirely.
    #[cfg_attr(not(test), expect(dead_code))]
    pub fn retire(&mut self, rule_ids: &[usize]) -> Vec<RuleOutput> {
        for rule in &mut self.rules {
            if !rule_ids.contains(&rule.meta().id) {
                continue;
            }
            match rule.finalize() {
                Ok(Some(grant)) => {
                    self.pending.push(RuleOutput {
                        meta: rule.meta().clone(),
                        grant,
                    });
                }
                Ok(None) => {}
                Err(e) => {
                    tracing::warn!(rule = rule.meta().human_id, "finalize failed: {e}");
                }
            }
        }
        let outputs = self.collect_pending();

        self.rules.retain(|r| !rule_ids.contains(&r.meta().id));

        outputs
    }

    /// Load persisted caches into rules.
    ///
    /// The `load` closure receives a rule's `human_id` and returns its
    /// serialized cache data. Returning `Value::Null` (or the result of
    /// loading a non-existent file) causes the rule to use its default cache.
    pub fn init_caches(
        &mut self,
        mut load: impl FnMut(&str) -> eyre::Result<serde_json::Value>,
    ) -> eyre::Result<()> {
        for rule in &mut self.rules {
            if !rule.has_cache() {
                continue;
            }
            let data = load(rule.meta().human_id)?;
            rule.init_cache(data)?;
        }
        Ok(())
    }

    /// Extract caches from rules for persistence.
    ///
    /// The `save` closure receives a rule's `human_id` and its serialized
    /// cache data.
    pub fn fini_caches(
        &self,
        mut save: impl FnMut(&str, serde_json::Value) -> eyre::Result<()>,
    ) -> eyre::Result<()> {
        for rule in &self.rules {
            if !rule.has_cache() {
                continue;
            }
            let data = rule.fini_cache()?;
            save(rule.meta().human_id, data)?;
        }
        Ok(())
    }

    /// Borrow the rules.
    #[cfg_attr(not(test), expect(dead_code))]
    pub fn rules(&self) -> &[Box<dyn RulePlugin>] {
        &self.rules
    }

    /// Consume the engine and return the rules.
    #[expect(dead_code)]
    pub fn into_rules(self) -> Vec<Box<dyn RulePlugin>> {
        self.rules
    }

    /// Sort buffered grants by rule ID for deterministic output, then drain.
    fn collect_pending(&mut self) -> Vec<RuleOutput> {
        self.pending.sort_by_key(|ro| ro.meta.id);
        self.pending.drain(..).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::observer::Observation;
    use crate::rules::test_rules::{CountingRule, GrantOnDummy};

    fn ctx() -> CommitContext {
        CommitContext {
            oid: gix::ObjectId::null(gix::hash::Kind::Sha1),
            author_name: "Test".to_string(),
            author_email: "test@example.com".to_string(),
        }
    }

    fn commit_cycle(engine: &mut RuleEngine, observations: &[Observation]) -> Vec<RuleOutput> {
        engine.on_commit_start(ctx());
        for obs in observations {
            engine.on_observation(obs);
        }
        engine.on_commit_complete()
    }

    #[test]
    fn grants_on_matching_observation() {
        let rules: Vec<Box<dyn RulePlugin>> = vec![Box::new(GrantOnDummy::new(100))];
        let mut engine = RuleEngine::new(rules);
        let outputs = commit_cycle(&mut engine, &[Observation::Dummy]);
        assert_eq!(outputs.len(), 1);
        assert_eq!(outputs[0].meta.id, 100);
        assert_eq!(outputs[0].grant.author_email, "test@example.com");
    }

    #[test]
    fn ignores_non_matching_observation() {
        let rules: Vec<Box<dyn RulePlugin>> = vec![Box::new(GrantOnDummy::new(100))];
        let mut engine = RuleEngine::new(rules);
        let outputs = commit_cycle(&mut engine, &[Observation::SubjectLength { length: 42 }]);
        assert!(outputs.is_empty());
    }

    #[test]
    fn grants_sorted_by_rule_id() {
        // Insert rules with higher ID first to verify sorting
        let rules: Vec<Box<dyn RulePlugin>> = vec![
            Box::new(GrantOnDummy::new(99)),
            Box::new(GrantOnDummy::new(10)),
            Box::new(GrantOnDummy::new(50)),
        ];
        let mut engine = RuleEngine::new(rules);
        let outputs = commit_cycle(&mut engine, &[Observation::Dummy]);
        assert_eq!(outputs.len(), 3);
        assert_eq!(outputs[0].meta.id, 10);
        assert_eq!(outputs[1].meta.id, 50);
        assert_eq!(outputs[2].meta.id, 99);
    }

    #[test]
    fn retire_finalizes_and_removes() {
        let rules: Vec<Box<dyn RulePlugin>> = vec![
            Box::new(CountingRule::new(10)),
            Box::new(CountingRule::new(20)),
            Box::new(CountingRule::new(30)),
        ];
        let mut engine = RuleEngine::new(rules);
        commit_cycle(&mut engine, &[Observation::Dummy]);

        let outputs = engine.retire(&[10, 30]);
        assert_eq!(outputs.len(), 2);
        assert_eq!(outputs[0].meta.id, 10);
        assert_eq!(outputs[1].meta.id, 30);

        assert_eq!(engine.rules().len(), 1);
        assert_eq!(engine.rules()[0].meta().id, 20);
    }

    #[test]
    fn init_caches_loads_for_cacheable_rules() {
        let rules: Vec<Box<dyn RulePlugin>> = vec![
            Box::new(GrantOnDummy::new(1)),  // Cache = (), has_cache() == false
            Box::new(CountingRule::new(10)), // Cache = usize, has_cache() == true
        ];
        let mut engine = RuleEngine::new(rules);

        let mut called_ids = Vec::new();
        engine
            .init_caches(|human_id| {
                called_ids.push(human_id.to_string());
                Ok(serde_json::Value::from(42))
            })
            .unwrap();

        // Only CountingRule should have its cache loaded
        assert_eq!(called_ids, vec!["counting-rule"]);
    }

    #[test]
    fn fini_caches_extracts_from_cacheable_rules() {
        let rules: Vec<Box<dyn RulePlugin>> = vec![
            Box::new(GrantOnDummy::new(1)),
            Box::new(CountingRule::new(10)),
        ];
        let mut engine = RuleEngine::new(rules);

        // Process a commit so CountingRule has count=1
        commit_cycle(&mut engine, &[Observation::Dummy]);

        let mut saved = Vec::new();
        engine
            .fini_caches(|human_id, data| {
                saved.push((human_id.to_string(), data));
                Ok(())
            })
            .unwrap();

        assert_eq!(saved.len(), 1);
        assert_eq!(saved[0].0, "counting-rule");
        assert_eq!(saved[0].1, serde_json::Value::from(1));
    }

    #[test]
    fn finalize_collects_from_all_rules() {
        let rules: Vec<Box<dyn RulePlugin>> = vec![
            Box::new(CountingRule::new(10)),
            Box::new(CountingRule::new(20)),
        ];
        let mut engine = RuleEngine::new(rules);
        commit_cycle(&mut engine, &[Observation::Dummy]);

        let outputs = engine.finalize();
        assert_eq!(outputs.len(), 2);
        assert_eq!(outputs[0].meta.id, 10);
        assert_eq!(outputs[1].meta.id, 20);
    }
}
