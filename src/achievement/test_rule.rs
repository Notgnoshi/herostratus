use crate::achievement::{process_rules, Achievement, Rule};
use crate::git::test::fixtures;

pub struct AlwaysFail;
impl Rule for AlwaysFail {
    fn name(&self) -> &'static str {
        "AlwaysFail"
    }
    fn process(&mut self, _commit: &git2::Commit, _repo: &git2::Repository) -> Option<Achievement> {
        None
    }
}

pub struct ParticipationTrophy;
impl Rule for ParticipationTrophy {
    fn name(&self) -> &'static str {
        "Participation Trophy"
    }
    fn process(&mut self, commit: &git2::Commit, _repo: &git2::Repository) -> Option<Achievement> {
        tracing::debug!("Granting {:?} a participation trophy", commit.id());
        Some(Achievement { name: self.name() })
    }
}

pub struct ParticipationTrophy2;
impl Rule for ParticipationTrophy2 {
    fn name(&self) -> &'static str {
        "Participation Trophy 2"
    }
    fn process(&mut self, _commit: &git2::Commit, _repo: &git2::Repository) -> Option<Achievement> {
        None
    }

    fn finalize(&mut self, _repo: &git2::Repository) -> Vec<Achievement> {
        tracing::debug!("Finalizing ParticipationTrophy2");
        vec![Achievement { name: self.name() }]
    }
}

#[test]
fn test_no_rules() {
    // TODO: Cut down on the copy-pasta
    let temp_repo = fixtures::repository::simplest().unwrap();
    let rev = crate::git::rev_parse("HEAD", &temp_repo.repo).unwrap();
    let oids = crate::git::rev_walk(rev, &temp_repo.repo).unwrap();
    let oids = oids.filter_map(|o| match o {
        Ok(o) => Some(o),
        Err(e) => {
            tracing::error!("Skipping OID: {e:?}");
            None
        }
    });

    let rules = Vec::new();
    let achievements = process_rules(oids, &temp_repo.repo, rules);
    let achievements: Vec<_> = achievements.collect();
    assert!(achievements.is_empty());
}

#[test]
fn test_iterator_no_matches() {
    let temp_repo = fixtures::repository::simplest().unwrap();
    let rev = crate::git::rev_parse("HEAD", &temp_repo.repo).unwrap();
    let oids = crate::git::rev_walk(rev, &temp_repo.repo).unwrap();
    let oids = oids.filter_map(|o| match o {
        Ok(o) => Some(o),
        Err(e) => {
            tracing::error!("Skipping OID: {e:?}");
            None
        }
    });

    let rules = vec![Box::new(AlwaysFail) as Box<dyn Rule>];
    let achievements = process_rules(oids, &temp_repo.repo, rules);
    let achievements: Vec<_> = achievements.collect();
    assert!(achievements.is_empty());
}

#[test]
fn test_iterator_all_matches() {
    let temp_repo = fixtures::repository::simplest().unwrap();
    let rev = crate::git::rev_parse("HEAD", &temp_repo.repo).unwrap();
    let oids = crate::git::rev_walk(rev, &temp_repo.repo).unwrap();
    let oids = oids.filter_map(|o| match o {
        Ok(o) => Some(o),
        Err(e) => {
            tracing::error!("Skipping OID: {e:?}");
            None
        }
    });

    let rules = vec![
        Box::new(AlwaysFail) as Box<dyn Rule>,
        Box::new(ParticipationTrophy) as Box<dyn Rule>,
    ];
    let achievements = process_rules(oids, &temp_repo.repo, rules);
    let achievements: Vec<_> = achievements.collect();
    assert_eq!(achievements.len(), 1);
}

#[test]
fn test_awards_on_finalize() {
    let temp_repo = fixtures::repository::simplest().unwrap();
    let rev = crate::git::rev_parse("HEAD", &temp_repo.repo).unwrap();
    let oids = crate::git::rev_walk(rev, &temp_repo.repo).unwrap();
    let oids = oids.filter_map(|o| match o {
        Ok(o) => Some(o),
        Err(e) => {
            tracing::error!("Skipping OID: {e:?}");
            None
        }
    });

    let rules = vec![Box::new(ParticipationTrophy2) as Box<dyn Rule>];
    let achievements = process_rules(oids, &temp_repo.repo, rules);
    let achievements: Vec<_> = achievements.collect();
    assert_eq!(achievements.len(), 1);
}
