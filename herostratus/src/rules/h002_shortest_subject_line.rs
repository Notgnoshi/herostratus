use crate::achievement::{Achievement, AchievementDescriptor, Rule, RuleFactory};
use crate::config::RulesConfig;

/// The shortest subject line in a branch
pub struct ShortestSubjectLine {
    descriptors: [AchievementDescriptor; 1],
    config: H002Config,
    shortest_so_far: Option<(gix::ObjectId, usize)>,
}

impl Default for ShortestSubjectLine {
    fn default() -> Self {
        Self {
            descriptors: [AchievementDescriptor {
                enabled: true,
                id: 2,
                human_id: "shortest-subject-line",
                name: "Brevity is the soul of wit",
                description: "The shortest subject line",
            }],
            config: H002Config::default(),
            shortest_so_far: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct H002Config {
    pub length_threshold: usize,
}

impl Default for H002Config {
    fn default() -> Self {
        Self {
            length_threshold: 10,
        }
    }
}

fn shortest_subject_line(config: &RulesConfig) -> Box<dyn Rule> {
    Box::new(ShortestSubjectLine {
        config: config.h2_shortest_subject_line.clone().unwrap_or_default(),
        ..Default::default()
    })
}
inventory::submit!(RuleFactory::new(shortest_subject_line));

fn subject_length(commit: &gix::Commit) -> usize {
    let Ok(msg) = commit.message() else {
        return 0;
    };
    // number of bytes, not number of characters, but that's fine for our purposes
    msg.title.len()
}

impl Rule for ShortestSubjectLine {
    fn get_descriptors(&self) -> &[AchievementDescriptor] {
        &self.descriptors
    }
    fn get_descriptors_mut(&mut self) -> &mut [AchievementDescriptor] {
        &mut self.descriptors
    }
    fn process(&mut self, commit: &gix::Commit, _repo: &gix::Repository) -> Option<Achievement> {
        let length = subject_length(commit);
        if length < self.config.length_threshold {
            match self.shortest_so_far {
                Some((_, shortest_length)) => {
                    if length < shortest_length {
                        self.shortest_so_far = Some((commit.id, length));
                    }
                }
                None => self.shortest_so_far = Some((commit.id, length)),
            }
        }

        None
    }

    fn finalize(&mut self, _repo: &gix::Repository) -> Vec<Achievement> {
        match self.shortest_so_far {
            // TODO: use AchievementDescriptor as source-of-truth for name
            Some((oid, _)) => vec![Achievement {
                name: "Brevity is the soul of wit",
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
    fn test_all_above_threshold() {
        let config = RulesConfig {
            h2_shortest_subject_line: Some(H002Config {
                length_threshold: 7,
            }),
            ..Default::default()
        };
        let repo = fixtures::repository::with_empty_commits(&["0123456789", "1234567890"]).unwrap();
        let rules = vec![shortest_subject_line(&config)];
        let achievements = grant_with_rules("HEAD", &repo.repo, None, rules).unwrap();
        let achievements: Vec<_> = achievements.collect();
        assert!(achievements.is_empty());
    }

    #[test]
    fn test_has_short_subject() {
        let repo =
            fixtures::repository::with_empty_commits(&["0123456789", "1234", "1234567", "12345"])
                .unwrap();
        let rules = vec![Box::new(ShortestSubjectLine::default()) as Box<dyn Rule>];
        let achievements = grant_with_rules("HEAD", &repo.repo, None, rules).unwrap();
        let achievements: Vec<_> = achievements.collect();
        assert_eq!(achievements.len(), 1);

        let oid = achievements[0].commit;
        let commit = repo.repo.find_commit(oid).unwrap();
        let summary = commit.message().unwrap().title;
        assert_eq!(summary, "1234");
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

        let achievements1 = grant_with_rules("HEAD", &repo1.repo, None, rules1).unwrap();
        let achievements2 = grant_with_rules("HEAD", &repo2.repo, None, rules2).unwrap();
        let achievements1: Vec<_> = achievements1.collect();
        assert_eq!(achievements1.len(), 1);
        let achievements2: Vec<_> = achievements2.collect();
        assert_eq!(achievements2.len(), 1);

        let oid = achievements1[0].commit;
        let commit = repo1.repo.find_commit(oid).unwrap();
        let summary = commit.message().unwrap().title;
        assert_eq!(summary, "234");

        let oid = achievements2[0].commit;
        let commit = repo2.repo.find_commit(oid).unwrap();
        let summary = commit.message().unwrap().title;
        assert_eq!(summary, "1234");
    }
}
