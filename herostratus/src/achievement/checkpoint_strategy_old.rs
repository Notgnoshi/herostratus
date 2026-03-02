use crate::cache::CheckpointCache;

/// What the pipeline should do when it encounters a commit
pub(crate) enum Continuation {
    /// Haven't hit checkpoint yet -- process this commit normally
    Process,
    /// Hit the checkpoint and no new rules exist -- just finalize and stop
    EarlyExit,
    /// Hit the checkpoint but new rules were added -- suppress old rules and continue
    SuppressAndContinue { rule_ids_to_suppress: Vec<usize> },
}

/// Pure decision-making logic for checkpoint-based early exit and rule suppression.
///
/// This struct does not own or mutate the engine -- it returns directives via [`Continuation`]
/// that the caller applies.
pub(crate) struct CheckpointStrategy {
    checkpoint: CheckpointCache,
    first_commit: Option<gix::ObjectId>,
}

impl CheckpointStrategy {
    pub fn new(checkpoint: CheckpointCache) -> Self {
        Self {
            checkpoint,
            first_commit: None,
        }
    }

    /// Evaluate what to do when we encounter this commit.
    ///
    /// `current_enabled_ids`: the rule IDs currently enabled in the engine.
    pub fn on_commit(&mut self, oid: gix::ObjectId, current_enabled_ids: &[usize]) -> Continuation {
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

        // We've hit a commit we've already processed. Do we need to keep going with any new
        // rules that were added since the last time we ran?
        //
        // CASE 1: No new rules were added since the last time we ran; we can finalize and stop
        //         processing new commits.
        //
        // CASE 2: New rules were added since the last time we ran; we need to suppress the old
        //         rules and continue processing commits with just the new rules.
        tracing::debug!("Reached last processed commit {oid}");

        // Figure out which rule IDs to suppress (those that were already processed)
        let rule_ids_to_suppress: Vec<usize> = self
            .checkpoint
            .data
            .rules
            .iter()
            .filter(|id| current_enabled_ids.contains(id))
            .copied()
            .collect();

        // After suppressing, will there be any enabled rules left?
        let has_remaining = current_enabled_ids
            .iter()
            .any(|id| !self.checkpoint.data.rules.contains(id));

        if !has_remaining {
            tracing::info!(
                "No new rules added since last run; finalizing achievements and exiting early ..."
            );
            Continuation::EarlyExit
        } else {
            Continuation::SuppressAndContinue {
                rule_ids_to_suppress,
            }
        }
    }

    /// The first commit encountered (for checkpoint saving)
    #[cfg(test)]
    pub fn first_commit(&self) -> Option<gix::ObjectId> {
        self.first_commit
    }

    /// Save the checkpoint with the given enabled rule IDs.
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
    fn test_no_checkpoint_always_process() {
        let cache = CheckpointCache::in_memory();
        let mut strategy = CheckpointStrategy::new(cache);

        let oid1 = make_oid(1);
        let oid2 = make_oid(2);
        assert!(matches!(
            strategy.on_commit(oid1, &[1, 2]),
            Continuation::Process
        ));
        assert!(matches!(
            strategy.on_commit(oid2, &[1, 2]),
            Continuation::Process
        ));
    }

    #[test]
    fn test_commits_before_checkpoint_process() {
        let checkpoint_oid = make_oid(99);
        let cache = checkpoint_with(checkpoint_oid, vec![1, 2]);
        let mut strategy = CheckpointStrategy::new(cache);

        // Different OID from checkpoint -- should process
        let oid = make_oid(1);
        assert!(matches!(
            strategy.on_commit(oid, &[1, 2]),
            Continuation::Process
        ));
    }

    #[test]
    fn test_hit_checkpoint_no_new_rules_early_exit() {
        let checkpoint_oid = make_oid(42);
        let cache = checkpoint_with(checkpoint_oid, vec![1, 2]);
        let mut strategy = CheckpointStrategy::new(cache);

        // Hit the checkpoint with the same rules that were already processed
        let result = strategy.on_commit(checkpoint_oid, &[1, 2]);
        assert!(matches!(result, Continuation::EarlyExit));
    }

    #[test]
    fn test_hit_checkpoint_new_rules_suppress_and_continue() {
        let checkpoint_oid = make_oid(42);
        let cache = checkpoint_with(checkpoint_oid, vec![1, 2]);
        let mut strategy = CheckpointStrategy::new(cache);

        // Hit the checkpoint with rule 3 being new
        let result = strategy.on_commit(checkpoint_oid, &[1, 2, 3]);
        match result {
            Continuation::SuppressAndContinue {
                rule_ids_to_suppress,
            } => {
                assert_eq!(rule_ids_to_suppress, vec![1, 2]);
            }
            _ => panic!("Expected SuppressAndContinue"),
        }
    }

    #[test]
    fn test_first_commit_tracked() {
        let cache = CheckpointCache::in_memory();
        let mut strategy = CheckpointStrategy::new(cache);

        assert!(strategy.first_commit().is_none());

        let oid1 = make_oid(1);
        let oid2 = make_oid(2);
        strategy.on_commit(oid1, &[]);
        strategy.on_commit(oid2, &[]);

        assert_eq!(strategy.first_commit(), Some(oid1));
    }

    #[test]
    fn test_save_checkpoint() {
        let cache = CheckpointCache::in_memory();
        let mut strategy = CheckpointStrategy::new(cache);

        let oid = make_oid(1);
        strategy.on_commit(oid, &[1, 2]);

        // save_checkpoint updates internal state; verify via first_commit
        strategy.save_checkpoint(vec![1, 2]).unwrap();
        assert_eq!(strategy.first_commit(), Some(oid));
    }
}
