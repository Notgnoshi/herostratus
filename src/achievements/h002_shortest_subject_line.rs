use crate::achievement::{Achievement, Rule};

/// The shortest subject line in a branch
#[derive(Default)]
pub struct ShortestSubjectLine {
    shortest_so_far: Option<(git2::Oid, usize)>,
}

fn subject_length(commit: &git2::Commit) -> usize {
    match commit.summary() {
        Some(subject) => subject.len(),
        None => 0,
    }
}

/// Only consider commits below a certain size to maximize the signal-to-noise ratio for this rule
#[inline]
fn short_enough_to_care(length: usize) -> bool {
    // TODO: There might be some good heuristics using number of words too?
    length < 10
}

impl Rule for ShortestSubjectLine {
    fn id(&self) -> usize {
        2
    }
    fn human_id(&self) -> &'static str {
        "shortest-subject-line"
    }
    fn name(&self) -> &'static str {
        "I bet you have the loudest keyboard"
    }
    fn description(&self) -> &'static str {
        "The shortest subject line"
    }
    fn process(&mut self, commit: &git2::Commit, _repo: &git2::Repository) -> Option<Achievement> {
        let length = subject_length(commit);
        if short_enough_to_care(length) {
            match self.shortest_so_far {
                Some((_, shortest_length)) => {
                    if length < shortest_length {
                        self.shortest_so_far = Some((commit.id(), length));
                    }
                }
                None => self.shortest_so_far = Some((commit.id(), length)),
            }
        }

        None
    }

    fn finalize(&mut self, _repo: &git2::Repository) -> Vec<Achievement> {
        match self.shortest_so_far {
            Some((oid, _)) => vec![Achievement {
                name: self.name(),
                commit: oid,
            }],
            None => Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::achievement::{grant_with_rules, Rule};
    use crate::test::fixtures;

    #[test]
    fn test_all_above_threshold() {
        let repo = fixtures::repository::with_empty_commits(&["0123456789", "1234567890"]).unwrap();
        let rules = vec![Box::new(ShortestSubjectLine::default()) as Box<dyn Rule>];
        let achievements = grant_with_rules("HEAD", &repo.repo, rules).unwrap();
        let achievements: Vec<_> = achievements.collect();
        assert!(achievements.is_empty());
    }

    #[test]
    fn test_has_short_subject() {
        let repo =
            fixtures::repository::with_empty_commits(&["0123456789", "1234567", "1234"]).unwrap();
        let rules = vec![Box::new(ShortestSubjectLine::default()) as Box<dyn Rule>];
        let achievements = grant_with_rules("HEAD", &repo.repo, rules).unwrap();
        let achievements: Vec<_> = achievements.collect();
        assert_eq!(achievements.len(), 1);

        let oid = achievements[0].commit;
        let commit = repo.repo.find_commit(oid).unwrap();
        assert_eq!(commit.summary(), Some("1234"));
    }

    #[test]
    fn test_resets_state_between_repositories() {
        let repo1 =
            fixtures::repository::with_empty_commits(&["0123456789", "1234567", "234"]).unwrap();
        let repo2 =
            fixtures::repository::with_empty_commits(&["1234567890", "2345671", "1234"]).unwrap();

        let rules1 = vec![Box::new(ShortestSubjectLine::default()) as Box<dyn Rule>];
        // grant_with_rules() consumes the rules Vec, so there _can't_ be any state held between
        // processing any two repositories
        let rules2 = vec![Box::new(ShortestSubjectLine::default()) as Box<dyn Rule>];

        let achievements1 = grant_with_rules("HEAD", &repo1.repo, rules1).unwrap();
        let achievements2 = grant_with_rules("HEAD", &repo2.repo, rules2).unwrap();
        let achievements1: Vec<_> = achievements1.collect();
        assert_eq!(achievements1.len(), 1);
        let achievements2: Vec<_> = achievements2.collect();
        assert_eq!(achievements2.len(), 1);

        let oid = achievements1[0].commit;
        let commit = repo1.repo.find_commit(oid).unwrap();
        assert_eq!(commit.summary(), Some("234"));

        let oid = achievements2[0].commit;
        let commit = repo2.repo.find_commit(oid).unwrap();
        assert_eq!(commit.summary(), Some("1234"));
    }
}
