use crate::achievement::{Achievement, Rule, RuleFactory};
use crate::config::RulesConfig;

pub struct LongestSubjectLine {
    config: H003Config,
    longest_so_far: Option<(gix::ObjectId, usize)>,
}

#[derive(Debug, Clone, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct H003Config {
    pub length_threshold: usize,
}

impl Default for H003Config {
    fn default() -> Self {
        Self {
            length_threshold: 72,
        }
    }
}

fn longest_subject_line(config: &RulesConfig) -> Box<dyn Rule> {
    Box::new(LongestSubjectLine {
        config: config.h3_longest_subject_line.clone().unwrap_or_default(),
        longest_so_far: None,
    })
}
inventory::submit!(RuleFactory::new(longest_subject_line));

fn subject_length(commit: &gix::Commit) -> usize {
    let Ok(msg) = commit.message() else {
        return 0;
    };
    // number of bytes, not number of characters, but that's fine for our purposes
    msg.title.len()
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

    fn process(&mut self, commit: &gix::Commit, _repo: &gix::Repository) -> Option<Achievement> {
        let length = subject_length(commit);
        if length > self.config.length_threshold {
            match self.longest_so_far {
                Some((_, longest_length)) => {
                    if length > longest_length {
                        self.longest_so_far = Some((commit.id, length));
                    }
                }
                None => self.longest_so_far = Some((commit.id, length)),
            }
        }
        None
    }

    fn finalize(&mut self, _repo: &gix::Repository) -> Vec<Achievement> {
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
    use herostratus_tests::fixtures;

    use super::*;
    use crate::achievement::grant_with_rules;

    #[test]
    fn test_all_below_threshold() {
        let config = RulesConfig {
            h3_longest_subject_line: Some(H003Config {
                length_threshold: 11,
            }),
            ..Default::default()
        };
        let repo = fixtures::repository::with_empty_commits(&["0123456789", "1234567890"]).unwrap();
        let rules = vec![longest_subject_line(&config)];
        let achievements = grant_with_rules("HEAD", &repo.repo, rules).unwrap();
        let achievements: Vec<_> = achievements.collect();
        assert!(achievements.is_empty());
    }

    #[test]
    fn test_has_long_subject() {
        let config = RulesConfig {
            h3_longest_subject_line: Some(H003Config {
                length_threshold: 8,
            }),
            ..Default::default()
        };
        let repo = fixtures::repository::with_empty_commits(&[
            "1234",
            "1234567890", // 10
            "123456789",  // 9
        ])
        .unwrap();
        let rules = vec![longest_subject_line(&config)];
        let achievements = grant_with_rules("HEAD", &repo.repo, rules).unwrap();
        let achievements: Vec<_> = achievements.collect();
        assert_eq!(achievements.len(), 1);

        let oid = achievements[0].commit;
        let commit = repo.repo.find_commit(oid).unwrap();
        let summary = commit.message().unwrap().title;
        assert_eq!(summary, "1234567890");
    }
}
