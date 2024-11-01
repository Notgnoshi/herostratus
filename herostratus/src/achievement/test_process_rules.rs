use crate::achievement::{grant_with_rules, Rule};
use crate::test::fixtures;

#[test]
fn test_no_rules() {
    let temp_repo = fixtures::repository::simplest().unwrap();
    let rules = Vec::new();
    let achievements = grant_with_rules("HEAD", &temp_repo.repo, rules).unwrap();
    let achievements: Vec<_> = achievements.collect();
    assert!(achievements.is_empty());
}

#[test]
fn test_iterator_no_matches() {
    let temp_repo = fixtures::repository::simplest().unwrap();
    let rules = vec![Box::new(fixtures::rule::AlwaysFail) as Box<dyn Rule>];
    let achievements = grant_with_rules("HEAD", &temp_repo.repo, rules).unwrap();
    let achievements: Vec<_> = achievements.collect();
    assert!(achievements.is_empty());
}

#[test]
fn test_iterator_all_matches() {
    let temp_repo = fixtures::repository::simplest().unwrap();

    let rules = vec![
        Box::new(fixtures::rule::AlwaysFail) as Box<dyn Rule>,
        Box::new(fixtures::rule::ParticipationTrophy) as Box<dyn Rule>,
    ];
    let achievements = grant_with_rules("HEAD", &temp_repo.repo, rules).unwrap();
    let achievements: Vec<_> = achievements.collect();
    assert_eq!(achievements.len(), 1);
}

#[test]
fn test_awards_on_finalize() {
    let temp_repo = fixtures::repository::simplest().unwrap();

    let rules = vec![Box::new(fixtures::rule::ParticipationTrophy2) as Box<dyn Rule>];
    let achievements = grant_with_rules("HEAD", &temp_repo.repo, rules).unwrap();
    let achievements: Vec<_> = achievements.collect();
    assert_eq!(achievements.len(), 1);
}
