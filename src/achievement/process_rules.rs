use crate::achievement::{Achievement, Rule};

trait LoggedRule: Rule {
    /// Wrap Rule::process in a logging helper
    fn process_log(
        &mut self,
        commit: &git2::Commit,
        repo: &git2::Repository,
    ) -> Option<Achievement> {
        let achievement = self.process(commit, repo)?;
        debug_assert_eq!(
            achievement.name,
            self.name(),
            "Achievement::name and Rule::name are expected to match"
        );
        tracing::info!("Generated achievement: {achievement:?}");
        Some(achievement)
    }

    /// Wrap Rule::finalize in a logging helper
    fn finalize_log(&mut self, repo: &git2::Repository) -> Vec<Achievement> {
        let achievements = self.finalize(repo);
        if !achievements.is_empty() {
            tracing::debug!(
                "Rule '{}' generated {} achievements",
                self.name(),
                achievements.len()
            );
            for achievement in &achievements {
                debug_assert_eq!(
                    achievement.name,
                    self.name(),
                    "Achievement::name and Rule::name are expected to match"
                );
                tracing::info!("Generated achievement: {achievement:?}");
            }
        }
        achievements
    }
}

impl<T: ?Sized + Rule> LoggedRule for T {}

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
}

impl<'repo, Oids> Achievements<'repo, Oids>
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

impl<'repo, Oids> Iterator for Achievements<'repo, Oids>
where
    Oids: Iterator<Item = git2::Oid>,
{
    type Item = Achievement;

    fn next(&mut self) -> Option<Self::Item> {
        // Get all of the achievements from processing the rules online
        if let Some(achievement) = self.get_next_achievement_online() {
            return Some(achievement);
        }

        // Once done processing all of the rules, collect any achievements that the rules stored.
        if self.finalized.is_none() {
            self.finalize_all_rules();
        }
        self.get_next_achievement_finalized()
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
    }
}
