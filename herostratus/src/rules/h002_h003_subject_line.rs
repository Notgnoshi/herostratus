use crate::achievement::{Achievement, AchievementDescriptor};
use crate::config::RulesConfig;
use crate::rules::{Rule, RuleFactory, RulePlugin};

const DESCRIPTORS: [AchievementDescriptor; 2] = [
    AchievementDescriptor {
        id: 2,
        human_id: "shortest-subject-line",
        name: "Brevity is the soul of wit",
        description: "The shortest subject line",
    },
    AchievementDescriptor {
        id: 3,
        human_id: "longest-subject-line",
        name: "50 characters was more of a suggestion anyways",
        description: "The longest subject line",
    },
];

#[derive(Clone, serde::Deserialize, serde::Serialize)]
pub struct LengthCache {
    shortest_length: usize,
    longest_length: usize,
}

impl Default for LengthCache {
    fn default() -> Self {
        Self {
            shortest_length: usize::MAX,
            longest_length: usize::MIN,
        }
    }
}

/// The shortest subject line in a branch
#[derive(Default)]
pub struct SubjectLineLength {
    h2_config: H002Config,
    h3_config: H003Config,
    cache: LengthCache,
    shortest_so_far: Option<gix::ObjectId>,
    longest_so_far: Option<gix::ObjectId>,
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

fn subject_line_factory(config: &RulesConfig) -> Box<dyn RulePlugin> {
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
    type Cache = LengthCache;

    fn init_cache(&mut self, cache: Self::Cache) {
        self.cache = cache;
    }

    fn fini_cache(&self) -> Self::Cache {
        self.cache.clone()
    }

    fn descriptors(&self) -> &[AchievementDescriptor] {
        &DESCRIPTORS
    }

    fn process(&mut self, commit: &gix::Commit, _repo: &gix::Repository) -> Vec<Achievement> {
        let length = subject_length(commit);
        if length < self.h2_config.length_threshold && length < self.cache.shortest_length {
            self.cache.shortest_length = length;
            self.shortest_so_far = Some(commit.id);
        }
        if length > self.h3_config.length_threshold && length > self.cache.longest_length {
            self.cache.longest_length = length;
            self.longest_so_far = Some(commit.id);
        }

        Vec::new()
    }

    fn finalize(&mut self, _repo: &gix::Repository) -> Vec<Achievement> {
        let mut achievements = Vec::new();

        // shortest subject line
        if let Some(oid) = self.shortest_so_far {
            achievements.push(DESCRIPTORS[0].grant(oid));
        }

        // longest subject line
        if let Some(oid) = self.longest_so_far {
            achievements.push(DESCRIPTORS[1].grant(oid));
        }

        achievements
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use herostratus_tests::fixtures::repository;

    use super::*;
    use crate::achievement::{Achievement, grant_with_rules_old as grant_with_rules};

    fn collect(
        reference: &str,
        repo: &gix::Repository,
        data_dir: Option<&Path>,
        rules: Vec<Box<dyn RulePlugin>>,
    ) -> Vec<Achievement> {
        let mut achievements = Vec::new();
        grant_with_rules(reference, repo, None, data_dir, "", rules, |a| {
            achievements.push(a);
        })
        .unwrap();
        achievements
    }

    #[test]
    fn test_all_above_threshold() {
        let config = RulesConfig {
            h2_shortest_subject_line: Some(H002Config {
                length_threshold: 7,
            }),
            ..Default::default()
        };
        let repo = repository::Builder::new()
            .commit("0123456789")
            .commit("1234567890")
            .build()
            .unwrap();
        let rules = vec![subject_line_factory(&config)];
        let achievements = collect("HEAD", &repo.repo, None, rules);
        assert!(achievements.is_empty());
    }

    #[test]
    fn test_has_short_subject() {
        let repo = repository::Builder::new()
            .commit("0123456789")
            .commit("1234")
            .commit("1234567")
            .commit("12345")
            .build()
            .unwrap();
        let rules = vec![Box::new(SubjectLineLength::default()) as Box<dyn RulePlugin>];
        let achievements = collect("HEAD", &repo.repo, None, rules);
        assert_eq!(achievements.len(), 1);

        let oid = achievements[0].commit;
        let commit = repo.repo.find_commit(oid).unwrap();
        let summary = commit.message().unwrap().title;
        assert_eq!(summary, "1234");
    }

    #[test]
    fn test_resets_state_between_repositories() {
        let repo1 = repository::Builder::new()
            .commit("0123456789")
            .commit("1234567")
            .commit("234")
            .build()
            .unwrap();
        let repo2 = repository::Builder::new()
            .commit("1234567890")
            .commit("2345671")
            .commit("1234")
            .build()
            .unwrap();

        let rules1 = vec![Box::new(SubjectLineLength::default()) as Box<dyn RulePlugin>];
        // grant_with_rules() consumes the rules Vec, so there _can't_ be any state held between
        // processing any two repositories
        let rules2 = vec![Box::new(SubjectLineLength::default()) as Box<dyn RulePlugin>];

        let achievements1 = collect("HEAD", &repo1.repo, None, rules1);
        let achievements2 = collect("HEAD", &repo2.repo, None, rules2);
        assert_eq!(achievements1.len(), 1);
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
        let repo = repository::Builder::new()
            .commit("1234")
            .commit("1234567890") // 10
            .commit("123456789") // 9
            .build()
            .unwrap();
        let rules = vec![subject_line_factory(&config)];
        let achievements = collect("HEAD", &repo.repo, None, rules);
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
        let config = RulesConfig {
            h2_shortest_subject_line: Some(H002Config {
                length_threshold: 8,
            }),
            ..Default::default()
        };
        let repo = repository::Builder::new()
            .commit("1234")
            .commit("1234567890") // 10
            .commit("123456789") // 9
            .build()
            .unwrap();
        let rules = vec![subject_line_factory(&config)];
        let achievements = collect("HEAD", &repo.repo, Some(repo.path()), rules);
        assert_eq!(achievements.len(), 1);

        // Add another commit with a subject line shorter than the threshold, but longer than the
        // shortest so far
        repo.commit("123456").create().unwrap();

        let rules = vec![subject_line_factory(&config)];
        let achievements = collect("HEAD", &repo.repo, Some(repo.path()), rules);
        assert!(achievements.is_empty());
    }

    #[test]
    fn test_shortest_on_second_run() {
        let config = RulesConfig {
            h2_shortest_subject_line: Some(H002Config {
                length_threshold: 8,
            }),
            ..Default::default()
        };
        let repo = repository::Builder::new()
            .commit("1234")
            .commit("1234567890") // 10
            .commit("123456789") // 9
            .build()
            .unwrap();
        let rules = vec![subject_line_factory(&config)];
        let achievements = collect("HEAD", &repo.repo, Some(repo.path()), rules);
        assert_eq!(achievements.len(), 1);

        // Add another commit with a subject line shorter than the shortest so far
        let new_shortest = repo.commit("123").create().unwrap();

        let rules = vec![subject_line_factory(&config)];
        let achievements = collect("HEAD", &repo.repo, Some(repo.path()), rules);
        assert_eq!(achievements.len(), 1);
        assert_eq!(achievements[0].commit, new_shortest);
    }
}
