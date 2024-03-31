use crate::achievement::{process_rules, Rule};
use crate::test::fixtures;

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

    let rules = vec![Box::new(fixtures::rule::AlwaysFail) as Box<dyn Rule>];
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
        Box::new(fixtures::rule::AlwaysFail) as Box<dyn Rule>,
        Box::new(fixtures::rule::ParticipationTrophy) as Box<dyn Rule>,
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

    let rules = vec![Box::new(fixtures::rule::ParticipationTrophy2) as Box<dyn Rule>];
    let achievements = process_rules(oids, &temp_repo.repo, rules);
    let achievements: Vec<_> = achievements.collect();
    assert_eq!(achievements.len(), 1);
}
