use crate::cache::CheckpointCache;

/// What the pipeline should do when it encounters a commit
pub enum Continuation {
    /// Haven't hit checkpoint yet -- process this commit normally
    Process,
    /// Hit the checkpoint commit -- caller must call [PipelineCheckpoint::resolve] to decide
    /// whether to early-exit or retire rules
    ReachedCheckpoint,
}

/// Decision after reaching the checkpoint commit
pub enum CheckpointAction {
    /// No new rules exist -- just finalize and stop
    EarlyExit,
    /// New rules were added -- retire old rules and continue
    Retire { rule_ids: Vec<usize> },
}

/// Pure decision-making logic for checkpoint-based early exit and rule retirement.
///
/// This struct does not own or mutate the engine -- it returns directives via [Continuation]
/// that the caller applies.
pub struct PipelineCheckpoint {
    checkpoint: CheckpointCache,
    first_commit: Option<gix::ObjectId>,
}

impl PipelineCheckpoint {
    pub fn new(checkpoint: CheckpointCache) -> Self {
        Self {
            checkpoint,
            first_commit: None,
        }
    }

    /// Evaluate what to do when we encounter this commit.
    #[tracing::instrument(target = "perf", name = "Checkpoint::on_commit", skip_all)]
    pub fn on_commit(&mut self, oid: gix::ObjectId) -> Continuation {
        if self.first_commit.is_none() {
            self.first_commit = Some(oid);
        }

        let Some(last_oid) = self.checkpoint.data.commit else {
            // If there's nothing in the cache, we continue processing and don't early-exit
            return Continuation::Process;
        };
        if oid != last_oid {
            // We've not processed this commit yet, so keep going
            return Continuation::Process;
        }

        tracing::debug!("Reached last processed commit {oid}");
        Continuation::ReachedCheckpoint
    }

    /// Decide what to do after reaching the checkpoint commit.
    ///
    /// Compares the `(rule_id, version)` pairs that were active when the checkpoint was saved
    /// against `current_enabled` to determine whether all rules have already been processed at
    /// their current versions (early exit) or whether some rules need a full pass (retire those
    /// that are unchanged, continue).
    pub fn resolve(&self, current_enabled: &[(usize, u32)]) -> CheckpointAction {
        // Unchanged = present in checkpoint at matching version
        let rule_ids: Vec<usize> = current_enabled
            .iter()
            .filter(|(id, ver)| {
                self.checkpoint
                    .data
                    .rules
                    .iter()
                    .any(|(cid, cver)| cid == id && cver == ver)
            })
            .map(|(id, _)| *id)
            .collect();

        // Anything in current_enabled that is NOT unchanged needs a full pass
        let has_remaining = current_enabled.iter().any(|(id, ver)| {
            !self
                .checkpoint
                .data
                .rules
                .iter()
                .any(|(cid, cver)| cid == id && cver == ver)
        });

        if !has_remaining {
            tracing::info!(
                "No rule changes since last run; finalizing achievements and exiting early ..."
            );
            CheckpointAction::EarlyExit
        } else {
            CheckpointAction::Retire { rule_ids }
        }
    }

    /// Save the checkpoint with the given enabled `(rule_id, version)` pairs.
    #[tracing::instrument(target = "perf", skip_all)]
    pub fn save_checkpoint(&mut self, enabled_rules: Vec<(usize, u32)>) -> eyre::Result<()> {
        self.checkpoint.data.rules = enabled_rules;
        self.checkpoint.data.commit = self.first_commit;
        self.checkpoint.save()
    }

    /// Rules present in the checkpoint at a different version than the current build.
    ///
    /// These rules have stale caches and stale grants that must be discarded before the walk.
    /// Rules that are new (not in the checkpoint at all) are not returned here; they are handled
    /// by the existing "new rule" code path in [resolve](Self::resolve).
    pub fn classify_invalidated(&self, current: &[(usize, u32)]) -> Vec<usize> {
        current
            .iter()
            .filter(|(id, ver)| {
                self.checkpoint
                    .data
                    .rules
                    .iter()
                    .any(|(cid, cver)| cid == id && cver != ver)
            })
            .map(|(id, _)| *id)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cache::Checkpoint;

    fn make_oid(byte: u8) -> gix::ObjectId {
        gix::ObjectId::from_bytes_or_panic(&[byte; 20])
    }

    fn checkpoint_with(commit: gix::ObjectId, rules: Vec<(usize, u32)>) -> CheckpointCache {
        let mut cache = CheckpointCache::in_memory();
        cache.data = Checkpoint {
            commit: Some(commit),
            rules,
        };
        cache
    }

    #[test]
    fn no_checkpoint_always_process() {
        let cache = CheckpointCache::in_memory();
        let mut checkpoint = PipelineCheckpoint::new(cache);

        let oid1 = make_oid(1);
        let oid2 = make_oid(2);
        assert!(matches!(checkpoint.on_commit(oid1), Continuation::Process));
        assert!(matches!(checkpoint.on_commit(oid2), Continuation::Process));
    }

    #[test]
    fn commits_before_checkpoint_process() {
        let checkpoint_oid = make_oid(99);
        let cache = checkpoint_with(checkpoint_oid, vec![(1, 1), (2, 1)]);
        let mut checkpoint = PipelineCheckpoint::new(cache);

        // Different OID from checkpoint -- should process
        let oid = make_oid(1);
        assert!(matches!(checkpoint.on_commit(oid), Continuation::Process));
    }

    #[test]
    fn hit_checkpoint_no_new_rules_early_exit() {
        let checkpoint_oid = make_oid(42);
        let cache = checkpoint_with(checkpoint_oid, vec![(1, 1), (2, 1)]);
        let mut checkpoint = PipelineCheckpoint::new(cache);

        // Hit the checkpoint with the same rules that were already processed
        assert!(matches!(
            checkpoint.on_commit(checkpoint_oid),
            Continuation::ReachedCheckpoint
        ));
        assert!(matches!(
            checkpoint.resolve(&[(1, 1), (2, 1)]),
            CheckpointAction::EarlyExit
        ));
    }

    #[test]
    fn hit_checkpoint_new_rules_retire_and_continue() {
        let checkpoint_oid = make_oid(42);
        let cache = checkpoint_with(checkpoint_oid, vec![(1, 1), (2, 1)]);
        let mut checkpoint = PipelineCheckpoint::new(cache);

        // Hit the checkpoint with rule 3 being new
        assert!(matches!(
            checkpoint.on_commit(checkpoint_oid),
            Continuation::ReachedCheckpoint
        ));
        match checkpoint.resolve(&[(1, 1), (2, 1), (3, 1)]) {
            CheckpointAction::Retire { rule_ids } => {
                assert_eq!(rule_ids, vec![1, 2]);
            }
            _ => panic!("Expected Retire"),
        }
    }

    #[test]
    fn save_checkpoint_records_first_commit_and_rules() {
        let cache = CheckpointCache::in_memory();
        let mut checkpoint = PipelineCheckpoint::new(cache);

        let oid = make_oid(1);
        checkpoint.on_commit(oid);
        checkpoint.save_checkpoint(vec![(1, 1), (2, 1)]).unwrap();

        // Verify by creating a new PipelineCheckpoint from the same cache data
        // (in-memory, so we check internal state indirectly via on_commit)
        assert_eq!(checkpoint.checkpoint.data.commit, Some(oid));
        assert_eq!(checkpoint.checkpoint.data.rules, vec![(1, 1), (2, 1)]);
    }

    #[test]
    fn classify_invalidated_returns_rules_with_mismatched_version() {
        let checkpoint_oid = make_oid(42);
        let cache = checkpoint_with(checkpoint_oid, vec![(1, 1), (2, 1), (3, 2)]);
        let checkpoint = PipelineCheckpoint::new(cache);

        // Rule 1 unchanged, rule 2 bumped to v2 (invalidated), rule 3 unchanged at v2,
        // rule 4 is new (not in checkpoint -- should NOT be in invalidated).
        let current = &[(1, 1), (2, 2), (3, 2), (4, 1)];
        let invalidated = checkpoint.classify_invalidated(current);
        assert_eq!(invalidated, vec![2]);
    }

    #[test]
    fn classify_invalidated_empty_when_no_mismatches() {
        let checkpoint_oid = make_oid(42);
        let cache = checkpoint_with(checkpoint_oid, vec![(1, 1), (2, 1)]);
        let checkpoint = PipelineCheckpoint::new(cache);

        let invalidated = checkpoint.classify_invalidated(&[(1, 1), (2, 1), (3, 1)]);
        assert!(invalidated.is_empty());
    }
}
