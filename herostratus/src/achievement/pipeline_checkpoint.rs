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
    /// Compares the rule IDs that were active when the checkpoint was saved against
    /// `current_enabled_ids` to determine whether all rules have already been processed
    /// (early exit) or whether new rules need a full pass (retire old rules, continue).
    pub fn resolve(&self, current_enabled_ids: &[usize]) -> CheckpointAction {
        // Figure out which rule IDs to retire (those that were already processed)
        let rule_ids: Vec<usize> = self
            .checkpoint
            .data
            .rules
            .iter()
            .filter(|id| current_enabled_ids.contains(id))
            .copied()
            .collect();

        // After retiring, will there be any enabled rules left?
        let has_remaining = current_enabled_ids
            .iter()
            .any(|id| !self.checkpoint.data.rules.contains(id));

        if !has_remaining {
            tracing::info!(
                "No new rules added since last run; finalizing achievements and exiting early ..."
            );
            CheckpointAction::EarlyExit
        } else {
            CheckpointAction::Retire { rule_ids }
        }
    }

    /// Save the checkpoint with the given enabled rule IDs.
    #[tracing::instrument(target = "perf", skip_all)]
    pub fn save_checkpoint(&mut self, enabled_rule_ids: Vec<usize>) -> eyre::Result<()> {
        self.checkpoint.data.rules = enabled_rule_ids;
        self.checkpoint.data.commit = self.first_commit;
        self.checkpoint.save()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cache::Checkpoint;

    fn make_oid(byte: u8) -> gix::ObjectId {
        gix::ObjectId::from_bytes_or_panic(&[byte; 20])
    }

    fn checkpoint_with(commit: gix::ObjectId, rules: Vec<usize>) -> CheckpointCache {
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
        let cache = checkpoint_with(checkpoint_oid, vec![1, 2]);
        let mut checkpoint = PipelineCheckpoint::new(cache);

        // Different OID from checkpoint -- should process
        let oid = make_oid(1);
        assert!(matches!(checkpoint.on_commit(oid), Continuation::Process));
    }

    #[test]
    fn hit_checkpoint_no_new_rules_early_exit() {
        let checkpoint_oid = make_oid(42);
        let cache = checkpoint_with(checkpoint_oid, vec![1, 2]);
        let mut checkpoint = PipelineCheckpoint::new(cache);

        // Hit the checkpoint with the same rules that were already processed
        assert!(matches!(
            checkpoint.on_commit(checkpoint_oid),
            Continuation::ReachedCheckpoint
        ));
        assert!(matches!(
            checkpoint.resolve(&[1, 2]),
            CheckpointAction::EarlyExit
        ));
    }

    #[test]
    fn hit_checkpoint_new_rules_retire_and_continue() {
        let checkpoint_oid = make_oid(42);
        let cache = checkpoint_with(checkpoint_oid, vec![1, 2]);
        let mut checkpoint = PipelineCheckpoint::new(cache);

        // Hit the checkpoint with rule 3 being new
        assert!(matches!(
            checkpoint.on_commit(checkpoint_oid),
            Continuation::ReachedCheckpoint
        ));
        match checkpoint.resolve(&[1, 2, 3]) {
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
        checkpoint.save_checkpoint(vec![1, 2]).unwrap();

        // Verify by creating a new PipelineCheckpoint from the same cache data
        // (in-memory, so we check internal state indirectly via on_commit)
        assert_eq!(checkpoint.checkpoint.data.commit, Some(oid));
        assert_eq!(checkpoint.checkpoint.data.rules, vec![1, 2]);
    }
}
