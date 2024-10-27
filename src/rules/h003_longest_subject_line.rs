use crate::achievement::{Achievement, Rule, RuleFactory};

pub struct LongestSubjectLine {
    length_threshold: usize,
    longest_so_far: Option<(git2::Oid, usize)>,
}

fn longest_subject_line() -> Box<dyn Rule> {
    Box::new(LongestSubjectLine {
        // I've seen linter rules at 50, 72, and 80 columns. Use 72 chars as the limit, so we know
        // we're granting this achievement to something egregious.
        //
        // TODO: Make this threshold configurable (#58)
        length_threshold: 72,
        longest_so_far: None,
    })
}
inventory::submit!(RuleFactory::new(longest_subject_line));

fn subject_length(commit: &git2::Commit) -> usize {
    match commit.summary() {
        Some(subject) => subject.len(),
        None => 0,
    }
}

impl Rule for LongestSubjectLine {
    fn id(&self) -> usize {
        3
    }
    fn human_id(&self) -> &'static str {
        "longest-subject-line"
    }
    fn name(&self) -> &'static str {
        "50 characters was more of a suggestion anyways"
    }
    fn description(&self) -> &'static str {
        "The longest subject line"
    }

    fn process(&mut self, commit: &git2::Commit, _repo: &git2::Repository) -> Option<Achievement> {
        let length = subject_length(commit);
        if length > self.length_threshold {
            match self.longest_so_far {
                Some((_, longest_length)) => {
                    if length > longest_length {
                        self.longest_so_far = Some((commit.id(), length));
                    }
                }
                None => self.longest_so_far = Some((commit.id(), length)),
            }
        }
        None
    }

    fn finalize(&mut self, _repo: &git2::Repository) -> Vec<Achievement> {
        match self.longest_so_far {
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
    use crate::achievement::grant_with_rules;
    use crate::test::fixtures;

    #[test]
    fn test_all_below_threshold() {
        let repo = fixtures::repository::with_empty_commits(&["0123456789", "1234567890"]).unwrap();
        let rules = vec![longest_subject_line()];
        let achievements = grant_with_rules("HEAD", &repo.repo, rules).unwrap();
        let achievements: Vec<_> = achievements.collect();
        assert!(achievements.is_empty());
    }

    #[test]
    fn test_has_long_subject() {
        let repo = fixtures::repository::with_empty_commits(&[
            "1234",
            "0123456789012345678901234567890123456789012345678901234567890123456789012345678", // 79
            "0123456789012345678901234567890123456789012345678901234567890123456789012345",    // 76
        ])
        .unwrap();
        let rules = vec![longest_subject_line()];
        let achievements = grant_with_rules("HEAD", &repo.repo, rules).unwrap();
        let achievements: Vec<_> = achievements.collect();
        assert_eq!(achievements.len(), 1);

        let oid = achievements[0].commit;
        let commit = repo.repo.find_commit(oid).unwrap();
        assert_eq!(
            commit.summary(),
            Some("0123456789012345678901234567890123456789012345678901234567890123456789012345678")
        );
    }
}
