use std::collections::HashMap;

use super::achievement_log::AchievementLog;
use super::grant::Grant;
use super::meta::{AchievementKind, Meta};
use crate::rules::RuleOutput;

/// Human IDs of all meta-achievements, used to exclude them from input counts.
///
/// Meta-achievements do not cascade: when counting achievements for meta-achievement evaluation,
/// grants from other meta-achievements are excluded. This prevents feedback loops across runs.
const META_ACHIEVEMENT_IDS: &[&str] = &["achievement-farmer"];

const ACHIEVEMENT_FARMER: Meta = Meta {
    id: 11,
    human_id: "achievement-farmer",
    name: "Achievement Farmer",
    description: "Farm the most achievements in the repository",
    kind: AchievementKind::Global { revocable: true },
};

/// Evaluate all meta-achievements against the full achievement log.
///
/// Meta-achievements are a single-pass post-processing step that runs after all rules have
/// finalized. They are recomputed from scratch each run and do not cascade.
///
/// Returns outputs suitable for resolution through the [AchievementLog], using the same
/// [AchievementKind] enforcement as regular rules.
#[tracing::instrument(target = "perf", skip_all)]
pub fn evaluate(log: &AchievementLog) -> Vec<RuleOutput> {
    let mut outputs = Vec::new();
    if let Some(output) = achievement_farmer(log) {
        outputs.push(output);
    }
    outputs
}

/// Grant the "Achievement Farmer" to the person with the most achievements.
///
/// Thresholds (all strict inequalities):
/// - More than 2 contributors with active achievements
/// - More than 10 active achievements total
/// - The leader must have more than 5 achievements
///
/// Ties are broken deterministically by alphabetically first email.
fn achievement_farmer(log: &AchievementLog) -> Option<RuleOutput> {
    let mut counts: HashMap<&str, usize> = HashMap::new();
    let mut names: HashMap<&str, &str> = HashMap::new();
    let mut commits: HashMap<&str, gix::ObjectId> = HashMap::new();
    let mut total: usize = 0;

    for event in log.active_grants() {
        if META_ACHIEVEMENT_IDS.contains(&event.achievement_id.as_str()) {
            continue;
        }
        total += 1;
        *counts.entry(event.user_email.as_str()).or_default() += 1;
        // Overwrite with the latest seen (events are in chronological order)
        names.insert(event.user_email.as_str(), event.user_name.as_str());
        commits.insert(event.user_email.as_str(), event.commit);
    }

    if counts.len() <= 2 {
        return None;
    }
    if total <= 10 {
        return None;
    }

    // Find the leader, breaking ties by alphabetically first email
    let (leader_email, leader_count) =
        counts
            .iter()
            .max_by(|(email_a, count_a), (email_b, count_b)| {
                count_a.cmp(count_b).then_with(|| email_b.cmp(email_a))
            })?;

    if *leader_count <= 5 {
        return None;
    }

    Some(RuleOutput {
        meta: ACHIEVEMENT_FARMER.clone(),
        grant: Grant {
            commit: commits[leader_email],
            user_name: names[leader_email].to_string(),
            user_email: leader_email.to_string(),
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::achievement::AchievementKind;

    fn add_grant(log: &mut AchievementLog, human_id: &'static str, email: &str, name: &str) {
        let meta = Meta {
            id: 0,
            human_id,
            name: "test",
            description: "test",
            kind: AchievementKind::PerUser { recurrent: true },
        };
        let grant = Grant {
            commit: gix::ObjectId::null(gix::hash::Kind::Sha1),
            user_name: name.to_string(),
            user_email: email.to_string(),
        };
        log.resolve(&meta, grant);
    }

    /// Populate a log with distinct achievement types for a given author.
    fn add_grants(log: &mut AchievementLog, email: &str, name: &str, count: usize) {
        for i in 0..count {
            // Use distinct human_ids so active_grants counts them separately
            let human_id: &'static str = Box::leak(format!("test-{i}").into_boxed_str());
            add_grant(log, human_id, email, name);
        }
    }

    #[test]
    fn too_few_contributors() {
        let mut log = AchievementLog::load(None).unwrap();
        // 2 contributors, 12 total achievements, leader has 6
        add_grants(&mut log, "alice@example.com", "Alice", 6);
        add_grants(&mut log, "bob@example.com", "Bob", 6);

        let result = achievement_farmer(&log);
        assert!(result.is_none(), "need > 2 contributors");
    }

    #[test]
    fn too_few_total_achievements() {
        let mut log = AchievementLog::load(None).unwrap();
        // 3 contributors, 10 total, leader has 6
        add_grants(&mut log, "alice@example.com", "Alice", 6);
        add_grants(&mut log, "bob@example.com", "Bob", 3);
        add_grants(&mut log, "carol@example.com", "Carol", 1);

        let result = achievement_farmer(&log);
        assert!(result.is_none(), "need > 10 total achievements");
    }

    #[test]
    fn leader_has_too_few() {
        let mut log = AchievementLog::load(None).unwrap();
        // 3 contributors, 12 total, leader has 5
        add_grants(&mut log, "alice@example.com", "Alice", 5);
        add_grants(&mut log, "bob@example.com", "Bob", 4);
        add_grants(&mut log, "carol@example.com", "Carol", 3);

        let result = achievement_farmer(&log);
        assert!(result.is_none(), "need leader to have > 5 achievements");
    }

    #[test]
    fn grants_to_leader_when_all_thresholds_met() {
        let mut log = AchievementLog::load(None).unwrap();
        // 3 contributors, 12 total, leader (Alice) has 6
        add_grants(&mut log, "alice@example.com", "Alice", 6);
        add_grants(&mut log, "bob@example.com", "Bob", 3);
        add_grants(&mut log, "carol@example.com", "Carol", 3);

        let result = achievement_farmer(&log);
        assert!(result.is_some(), "all thresholds met");
        let output = result.unwrap();
        assert_eq!(output.meta.id, 11);
        assert_eq!(output.grant.user_email, "alice@example.com");
        assert_eq!(output.grant.user_name, "Alice");
    }

    #[test]
    fn meta_achievement_grants_excluded_from_counts() {
        let mut log = AchievementLog::load(None).unwrap();
        // Alice has 5 real achievements + 1 meta-achievement grant
        add_grants(&mut log, "alice@example.com", "Alice", 5);
        add_grant(&mut log, "achievement-farmer", "alice@example.com", "Alice");
        add_grants(&mut log, "bob@example.com", "Bob", 3);
        add_grants(&mut log, "carol@example.com", "Carol", 3);
        // Total real: 11, Alice real: 5 (not > 5)

        let result = achievement_farmer(&log);
        assert!(
            result.is_none(),
            "meta-achievement grants should not count toward leader threshold"
        );
    }

    #[test]
    fn ties_broken_by_email() {
        let mut log = AchievementLog::load(None).unwrap();
        // Alice and Bob both have 6. Alice wins by email sort.
        add_grants(&mut log, "alice@example.com", "Alice", 6);
        add_grants(&mut log, "bob@example.com", "Bob", 6);
        add_grants(&mut log, "carol@example.com", "Carol", 3);

        let result = achievement_farmer(&log).unwrap();
        assert_eq!(
            result.grant.user_email, "alice@example.com",
            "ties should be broken by alphabetically first email"
        );
    }

    #[test]
    fn evaluate_returns_outputs() {
        let mut log = AchievementLog::load(None).unwrap();
        add_grants(&mut log, "alice@example.com", "Alice", 6);
        add_grants(&mut log, "bob@example.com", "Bob", 3);
        add_grants(&mut log, "carol@example.com", "Carol", 3);

        let outputs = evaluate(&log);
        assert_eq!(outputs.len(), 1);
        assert_eq!(outputs[0].meta.human_id, "achievement-farmer");
    }

    #[test]
    fn evaluate_returns_empty_when_thresholds_not_met() {
        let log = AchievementLog::load(None).unwrap();
        let outputs = evaluate(&log);
        assert!(outputs.is_empty());
    }
}
