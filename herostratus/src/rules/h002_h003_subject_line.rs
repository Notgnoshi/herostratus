use crate::achievement::{Achievement, AchievementDescriptor, Rule, RuleFactory};
use crate::config::RulesConfig;

/// The shortest subject line in a branch
pub struct SubjectLineLength {
    descriptors: [AchievementDescriptor; 2],
    h2_config: H002Config,
    h3_config: H003Config,
    shortest_length: usize,
    longest_length: usize,
    shortest_so_far: Option<gix::ObjectId>,
    longest_so_far: Option<gix::ObjectId>,
}

impl Default for SubjectLineLength {
    fn default() -> Self {
        Self {
            descriptors: [
                AchievementDescriptor {
                    enabled: true,
                    id: 2,
                    human_id: "shortest-subject-line",
                    name: "Brevity is the soul of wit",
                    description: "The shortest subject line",
                },
                AchievementDescriptor {
                    enabled: true,
                    id: 3,
                    human_id: "longest-subject-line",
                    name: "50 characters was more of a suggestion anyways",
                    description: "The longest subject line",
                },
            ],
            h2_config: H002Config::default(),
            h3_config: H003Config::default(),
            shortest_length: usize::MAX,
            longest_length: usize::MIN,
            shortest_so_far: None,
            longest_so_far: None,
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

fn subject_line_factory(config: &RulesConfig) -> Box<dyn Rule> {
    Box::new(SubjectLineLength {
        h2_config: config.h2_shortest_subject_line.clone().unwrap_or_default(),
        h3_config: config.h3_longest_subject_line.clone().unwrap_or_default(),
        ..Default::default()
    })
}
inventory::submit!(RuleFactory::new(subject_line_factory));

fn subject_length(commit: &gix::Commit) -> usize {
    let Ok(msg) = commit.message() else {
        return 0;
    };
    // number of bytes, not number of characters, but that's fine for our purposes
    msg.title.len()
}

impl Rule for SubjectLineLength {
    fn get_descriptors(&self) -> &[AchievementDescriptor] {
        &self.descriptors
    }
    fn get_descriptors_mut(&mut self) -> &mut [AchievementDescriptor] {
        &mut self.descriptors
    }
    fn init_cache(&mut self, cache: &crate::cache::EntryCache) {
        if let Some(length) = cache.shortest_subject_line_length {
            self.shortest_length = length;
        }
        if let Some(length) = cache.longest_subject_line_length {
            self.longest_length = length;
        }
    }
    fn fini_cache(&mut self, cache: &mut crate::cache::EntryCache) {
        cache.shortest_subject_line_length = Some(self.shortest_length);
        cache.longest_subject_line_length = Some(self.longest_length);
    }

    fn process(&mut self, commit: &gix::Commit, _repo: &gix::Repository) -> Vec<Achievement> {
        let length = subject_length(commit);
        if length < self.h2_config.length_threshold && length < self.shortest_length {
            self.shortest_length = length;
            self.shortest_so_far = Some(commit.id);
        }
        if length > self.h3_config.length_threshold && length > self.longest_length {
            self.longest_length = length;
            self.longest_so_far = Some(commit.id);
        }

        Vec::new()
    }

    fn finalize(&mut self, _repo: &gix::Repository) -> Vec<Achievement> {
        let mut achievements = Vec::new();

        // shortest subject line
        if self.descriptors[0].enabled
            && let Some(oid) = self.shortest_so_far
        {
            achievements.push(Achievement {
                name: self.descriptors[0].name,
                commit: oid,
            });
        }

        // longest subject line
        if self.descriptors[1].enabled
            && let Some(oid) = self.longest_so_far
        {
            achievements.push(Achievement {
                name: self.descriptors[1].name,
                commit: oid,
            });
        }

        achievements
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
        let rules = vec![subject_line_factory(&config)];
        let mut cache = crate::cache::EntryCache::default();
        let achievements = grant_with_rules("HEAD", &repo.repo, &mut cache, None, rules).unwrap();
        let achievements: Vec<_> = achievements.collect();
        assert!(achievements.is_empty());
    }

    #[test]
    fn test_has_short_subject() {
        let repo =
            fixtures::repository::with_empty_commits(&["0123456789", "1234", "1234567", "12345"])
                .unwrap();
        let rules = vec![Box::new(SubjectLineLength::default()) as Box<dyn Rule>];
        let mut cache = crate::cache::EntryCache::default();
        let achievements = grant_with_rules("HEAD", &repo.repo, &mut cache, None, rules).unwrap();
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

        let rules1 = vec![Box::new(SubjectLineLength::default()) as Box<dyn Rule>];
        // grant_with_rules() consumes the rules Vec, so there _can't_ be any state held between
        // processing any two repositories
        let rules2 = vec![Box::new(SubjectLineLength::default()) as Box<dyn Rule>];

        let mut cache1 = crate::cache::EntryCache::default();
        let achievements1 =
            grant_with_rules("HEAD", &repo1.repo, &mut cache1, None, rules1).unwrap();
        let mut cache2 = crate::cache::EntryCache::default();
        let achievements2 =
            grant_with_rules("HEAD", &repo2.repo, &mut cache2, None, rules2).unwrap();
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
        let rules = vec![subject_line_factory(&config)];
        let mut cache = crate::cache::EntryCache::default();
        let achievements = grant_with_rules("HEAD", &repo.repo, &mut cache, None, rules).unwrap();
        let achievements: Vec<_> = achievements.collect();
        assert_eq!(achievements.len(), 2); // two achievements: one for the shortest and longest

        assert_eq!(achievements[0].name, "Brevity is the soul of wit");
        let oid = achievements[0].commit;
        let commit = repo.repo.find_commit(oid).unwrap();
        let summary = commit.message().unwrap().title;
        assert_eq!(summary, "1234");

        assert_eq!(
            achievements[1].name,
            "50 characters was more of a suggestion anyways"
        );
        let oid = achievements[1].commit;
        let commit = repo.repo.find_commit(oid).unwrap();
        let summary = commit.message().unwrap().title;
        assert_eq!(summary, "1234567890");
    }

    #[test]
    fn test_shortest_on_first_run() {
        let mut cache = crate::cache::EntryCache::default();

        let config = RulesConfig {
            h2_shortest_subject_line: Some(H002Config {
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
        let rules = vec![subject_line_factory(&config)];
        let achievements = grant_with_rules("HEAD", &repo.repo, &mut cache, None, rules).unwrap();
        let achievements: Vec<_> = achievements.collect();
        assert_eq!(achievements.len(), 1);

        // Add another commit with a subject line shorter than the threshold, but longer than the
        // shortest so far
        fixtures::repository::add_empty_commit(&repo.repo, "123456").unwrap();

        let rules = vec![subject_line_factory(&config)];
        let achievements = grant_with_rules("HEAD", &repo.repo, &mut cache, None, rules).unwrap();
        let achievements: Vec<_> = achievements.collect();
        assert!(achievements.is_empty());
    }

    #[test]
    fn test_shortest_on_second_run() {
        let mut cache = crate::cache::EntryCache::default();

        let config = RulesConfig {
            h2_shortest_subject_line: Some(H002Config {
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
        let rules = vec![subject_line_factory(&config)];
        let achievements = grant_with_rules("HEAD", &repo.repo, &mut cache, None, rules).unwrap();
        let achievements: Vec<_> = achievements.collect();
        assert_eq!(achievements.len(), 1);

        // Add another commit with a subject line shorter than the shortest so far
        let new_shortest = fixtures::repository::add_empty_commit(&repo.repo, "123").unwrap();

        let rules = vec![subject_line_factory(&config)];
        let achievements = grant_with_rules("HEAD", &repo.repo, &mut cache, None, rules).unwrap();
        let achievements: Vec<_> = achievements.collect();
        assert_eq!(achievements.len(), 1);
        assert_eq!(achievements[0].commit, new_shortest);
    }
}
