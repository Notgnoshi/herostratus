use std::collections::HashMap;

use super::users::User;
use crate::achievement::{
    AchievementEventKind, AchievementLogEvent, AchievementRow, RepositoryRow,
};

/// All aggregated data needed to render the site.
pub struct SiteData {
    pub achievements: Vec<AchievementContext>,
    pub repositories: Vec<RepoContext>,
    pub users: Vec<UserContext>,
    pub recent_activity: Vec<ActivityEntry>,
}

/// Per-achievement aggregated data.
#[derive(Debug, serde::Serialize)]
pub struct AchievementContext {
    pub id: usize,
    pub human_id: String,
    pub name: String,
    pub description: String,
    pub kind: String,
    pub holders: Vec<HolderEntry>,
    pub total_grants: usize,
    pub unique_holders: usize,
    pub history: Vec<ActivityEntry>,
}

/// A user who holds an achievement.
#[derive(Debug, serde::Serialize)]
pub struct HolderEntry {
    pub user_name: String,
    pub user_slug: String,
    pub repo_name: String,
    pub commit: String,
    pub commit_url_prefix: String,
    pub timestamp: String,
}

/// Per-repository aggregated data.
#[derive(Debug, serde::Serialize)]
pub struct RepoContext {
    pub name: String,
    pub url: String,
    pub commit_url_prefix: String,
    pub reference: String,
    pub commits_checked: u64,
    pub last_checked: String,
    pub events: Vec<ActivityEntry>,
    pub achievement_summary: Vec<AchievementSummaryEntry>,
    pub total_achievements: usize,
    pub unique_achievers: usize,
}

/// Per-user aggregated data.
#[derive(Debug, serde::Serialize)]
pub struct UserContext {
    pub name: String,
    pub email: String,
    pub slug: String,
    pub total_achievements: usize,
    pub active_achievements: usize,
    pub repos_contributed_to: usize,
    pub achievements_by_repo: Vec<UserRepoAchievements>,
    pub timeline: Vec<ActivityEntry>,
}

/// A user's achievements in a single repo.
#[derive(Debug, serde::Serialize)]
pub struct UserRepoAchievements {
    pub repo_name: String,
    pub commit_url_prefix: String,
    pub achievements: Vec<UserAchievementEntry>,
}

/// A single achievement held by a user.
#[derive(Debug, serde::Serialize)]
pub struct UserAchievementEntry {
    pub achievement_name: String,
    pub achievement_human_id: String,
    pub description: String,
    pub commit: String,
    pub timestamp: String,
}

/// An entry in an achievement summary table.
#[derive(Debug, serde::Serialize)]
pub struct AchievementSummaryEntry {
    pub achievement_name: String,
    pub achievement_human_id: String,
    pub grant_count: usize,
}

/// A single event for display in timelines and recent activity.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ActivityEntry {
    pub timestamp: String,
    pub event: String,
    pub achievement_name: String,
    pub achievement_human_id: String,
    pub user_name: String,
    pub user_slug: String,
    pub repo_name: String,
    pub commit: String,
    pub commit_url_prefix: String,
}

/// Build all aggregated site data from loaded CSVs and derived users.
pub fn aggregate(
    achievements: &[AchievementRow],
    repositories: &[RepositoryRow],
    events: &HashMap<String, Vec<AchievementLogEvent>>,
    users: &[User],
) -> SiteData {
    let user_by_email: HashMap<&str, &User> = users.iter().map(|u| (u.email.as_str(), u)).collect();
    let achievement_by_id: HashMap<&str, &AchievementRow> = achievements
        .iter()
        .map(|a| (a.human_id.as_str(), a))
        .collect();
    let prefix_by_repo: HashMap<&str, &str> = repositories
        .iter()
        .map(|r| (r.name.as_str(), r.commit_url_prefix.as_str()))
        .collect();

    // Build activity entries from all events
    let mut all_activity: Vec<ActivityEntry> = Vec::new();
    for (repo_name, repo_events) in events {
        for event in repo_events {
            let user = user_by_email.get(event.user_email.as_str());
            let achievement = achievement_by_id.get(event.achievement_id.as_str());
            all_activity.push(ActivityEntry {
                timestamp: event.timestamp.to_rfc3339(),
                event: match event.event {
                    AchievementEventKind::Grant => "grant".to_string(),
                    AchievementEventKind::Revoke => "revoke".to_string(),
                },
                achievement_name: achievement
                    .map(|a| a.name.clone())
                    .unwrap_or_else(|| event.achievement_id.clone()),
                achievement_human_id: event.achievement_id.clone(),
                user_name: user
                    .map(|u| u.name.clone())
                    .unwrap_or_else(|| event.user_name.clone()),
                user_slug: user.map(|u| u.slug.clone()).unwrap_or_default(),
                repo_name: repo_name.clone(),
                commit: event.commit.to_string(),
                commit_url_prefix: prefix_by_repo
                    .get(repo_name.as_str())
                    .unwrap_or(&"")
                    .to_string(),
            });
        }
    }
    all_activity.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));

    let recent_activity: Vec<ActivityEntry> = all_activity.iter().take(20).cloned().collect();

    // Compute active grants (grants not subsequently revoked) per repo
    let active_grants = compute_active_grants(events);

    // Per-achievement contexts
    let achievement_contexts = build_achievement_contexts(
        achievements,
        &all_activity,
        &active_grants,
        &user_by_email,
        &prefix_by_repo,
    );

    // Per-repo contexts
    let repo_contexts = build_repo_contexts(
        repositories,
        events,
        &all_activity,
        &active_grants,
        &achievement_by_id,
    );

    // Per-user contexts
    let user_contexts = build_user_contexts(
        users,
        events,
        &all_activity,
        &active_grants,
        &achievement_by_id,
        &prefix_by_repo,
    );

    SiteData {
        achievements: achievement_contexts,
        repositories: repo_contexts,
        users: user_contexts,
        recent_activity,
    }
}

/// An active grant: a grant event that has not been subsequently revoked.
struct ActiveGrant<'a> {
    event: &'a AchievementLogEvent,
    repo_name: &'a str,
}

/// Compute the set of active grants across all repos.
///
/// An active grant is a grant event for which there is no subsequent revoke event with the same
/// achievement_id and user_email in the same repo.
fn compute_active_grants<'a>(
    events: &'a HashMap<String, Vec<AchievementLogEvent>>,
) -> Vec<ActiveGrant<'a>> {
    let mut active = Vec::new();

    for (repo_name, repo_events) in events {
        // Track which (achievement_id, user_email) pairs are currently active
        let mut repo_active: Vec<&AchievementLogEvent> = Vec::new();
        for event in repo_events {
            match event.event {
                AchievementEventKind::Grant => repo_active.push(event),
                AchievementEventKind::Revoke => {
                    repo_active.retain(|g| {
                        !(g.achievement_id == event.achievement_id
                            && g.user_email == event.user_email)
                    });
                }
            }
        }
        for grant in repo_active {
            active.push(ActiveGrant {
                event: grant,
                repo_name,
            });
        }
    }

    active
}

fn build_achievement_contexts(
    achievements: &[AchievementRow],
    all_activity: &[ActivityEntry],
    active_grants: &[ActiveGrant<'_>],
    user_by_email: &HashMap<&str, &User>,
    prefix_by_repo: &HashMap<&str, &str>,
) -> Vec<AchievementContext> {
    achievements
        .iter()
        .map(|a| {
            let holders: Vec<HolderEntry> = active_grants
                .iter()
                .filter(|g| g.event.achievement_id == a.human_id)
                .map(|g| {
                    let user = user_by_email.get(g.event.user_email.as_str());
                    HolderEntry {
                        user_name: user
                            .map(|u| u.name.clone())
                            .unwrap_or_else(|| g.event.user_name.clone()),
                        user_slug: user.map(|u| u.slug.clone()).unwrap_or_default(),
                        repo_name: g.repo_name.to_string(),
                        commit: g.event.commit.to_string(),
                        commit_url_prefix: prefix_by_repo
                            .get(g.repo_name)
                            .unwrap_or(&"")
                            .to_string(),
                        timestamp: g.event.timestamp.to_rfc3339(),
                    }
                })
                .collect();

            let history: Vec<ActivityEntry> = all_activity
                .iter()
                .filter(|e| e.achievement_human_id == a.human_id)
                .cloned()
                .collect();

            let total_grants = history.iter().filter(|e| e.event == "grant").count();
            let mut unique_emails: Vec<&str> =
                holders.iter().map(|h| h.user_slug.as_str()).collect();
            unique_emails.sort();
            unique_emails.dedup();
            let unique_holders = unique_emails.len();

            AchievementContext {
                id: a.id,
                human_id: a.human_id.clone(),
                name: a.name.clone(),
                description: a.description.clone(),
                kind: a.kind.clone(),
                holders,
                total_grants,
                unique_holders,
                history,
            }
        })
        .collect()
}

fn build_repo_contexts(
    repositories: &[RepositoryRow],
    events: &HashMap<String, Vec<AchievementLogEvent>>,
    all_activity: &[ActivityEntry],
    active_grants: &[ActiveGrant<'_>],
    achievement_by_id: &HashMap<&str, &AchievementRow>,
) -> Vec<RepoContext> {
    repositories
        .iter()
        .map(|repo| {
            let repo_events: Vec<ActivityEntry> = all_activity
                .iter()
                .filter(|e| e.repo_name == repo.name)
                .cloned()
                .collect();

            let repo_active: Vec<&ActiveGrant<'_>> = active_grants
                .iter()
                .filter(|g| g.repo_name == repo.name)
                .collect();

            // Achievement summary: count grants per achievement
            let mut achv_counts: HashMap<&str, usize> = HashMap::new();
            if let Some(repo_evts) = events.get(&repo.name) {
                for event in repo_evts {
                    if event.event == AchievementEventKind::Grant {
                        *achv_counts.entry(&event.achievement_id).or_default() += 1;
                    }
                }
            }
            let mut achievement_summary: Vec<AchievementSummaryEntry> = achv_counts
                .into_iter()
                .map(|(id, count)| {
                    let achievement = achievement_by_id.get(id);
                    AchievementSummaryEntry {
                        achievement_name: achievement
                            .map(|a| a.name.clone())
                            .unwrap_or_else(|| id.to_string()),
                        achievement_human_id: id.to_string(),
                        grant_count: count,
                    }
                })
                .collect();
            achievement_summary.sort_by(|a, b| b.grant_count.cmp(&a.grant_count));

            let mut unique_achievers: Vec<&str> = repo_active
                .iter()
                .map(|g| g.event.user_email.as_str())
                .collect();
            unique_achievers.sort();
            unique_achievers.dedup();

            RepoContext {
                name: repo.name.clone(),
                url: repo.url.clone(),
                commit_url_prefix: repo.commit_url_prefix.clone(),
                reference: repo.reference.clone(),
                commits_checked: repo.commits_checked,
                last_checked: repo.last_checked.clone(),
                total_achievements: repo_active.len(),
                unique_achievers: unique_achievers.len(),
                events: repo_events,
                achievement_summary,
            }
        })
        .collect()
}

fn build_user_contexts(
    users: &[User],
    events: &HashMap<String, Vec<AchievementLogEvent>>,
    all_activity: &[ActivityEntry],
    active_grants: &[ActiveGrant<'_>],
    achievement_by_id: &HashMap<&str, &AchievementRow>,
    prefix_by_repo: &HashMap<&str, &str>,
) -> Vec<UserContext> {
    users
        .iter()
        .map(|user| {
            let user_active: Vec<&ActiveGrant<'_>> = active_grants
                .iter()
                .filter(|g| g.event.user_email == user.email)
                .collect();

            // Group active achievements by repo
            let mut by_repo: HashMap<&str, Vec<UserAchievementEntry>> = HashMap::new();
            for grant in &user_active {
                let achievement = achievement_by_id.get(grant.event.achievement_id.as_str());
                by_repo
                    .entry(grant.repo_name)
                    .or_default()
                    .push(UserAchievementEntry {
                        achievement_name: achievement
                            .map(|a| a.name.clone())
                            .unwrap_or_else(|| grant.event.achievement_id.clone()),
                        achievement_human_id: grant.event.achievement_id.clone(),
                        description: achievement
                            .map(|a| a.description.clone())
                            .unwrap_or_default(),
                        commit: grant.event.commit.to_string(),
                        timestamp: grant.event.timestamp.to_rfc3339(),
                    });
            }
            let mut achievements_by_repo: Vec<UserRepoAchievements> = by_repo
                .into_iter()
                .map(|(repo_name, achievements)| UserRepoAchievements {
                    repo_name: repo_name.to_string(),
                    commit_url_prefix: prefix_by_repo.get(repo_name).unwrap_or(&"").to_string(),
                    achievements,
                })
                .collect();
            achievements_by_repo.sort_by(|a, b| a.repo_name.cmp(&b.repo_name));

            let timeline: Vec<ActivityEntry> = all_activity
                .iter()
                .filter(|e| e.user_slug == user.slug)
                .cloned()
                .collect();

            // Total achievements = all grants ever
            let total_achievements: usize = events
                .values()
                .flat_map(|evts| evts.iter())
                .filter(|e| e.user_email == user.email && e.event == AchievementEventKind::Grant)
                .count();

            let repos_contributed_to = achievements_by_repo.len();

            UserContext {
                name: user.name.clone(),
                email: user.email.clone(),
                slug: user.slug.clone(),
                total_achievements,
                active_achievements: user_active.len(),
                repos_contributed_to,
                achievements_by_repo,
                timeline,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use chrono::TimeZone;

    use super::*;
    use crate::achievement::{
        AchievementEventKind, AchievementLogEvent, AchievementRow, RepositoryRow,
    };

    fn make_event(
        email: &str,
        name: &str,
        achievement_id: &str,
        kind: AchievementEventKind,
        secs: i64,
    ) -> AchievementLogEvent {
        AchievementLogEvent {
            timestamp: chrono::Utc.timestamp_opt(secs, 0).unwrap(),
            event: kind,
            achievement_id: achievement_id.to_string(),
            commit: gix::ObjectId::from_bytes_or_panic(&[0xAA; 20]),
            user_name: name.to_string(),
            user_email: email.to_string(),
        }
    }

    fn test_achievement(human_id: &str, name: &str) -> AchievementRow {
        AchievementRow {
            id: 1,
            human_id: human_id.to_string(),
            name: name.to_string(),
            description: "test".to_string(),
            kind: "per-user".to_string(),
        }
    }

    fn test_repo(name: &str) -> RepositoryRow {
        RepositoryRow {
            name: name.to_string(),
            url: format!("https://example.com/{name}.git"),
            commit_url_prefix: String::new(),
            reference: "main".to_string(),
            commits_checked: 10,
            last_checked: "2026-01-01T00:00:00Z".to_string(),
        }
    }

    fn test_user(email: &str, name: &str, slug: &str) -> User {
        User {
            email: email.to_string(),
            name: name.to_string(),
            slug: slug.to_string(),
        }
    }

    #[test]
    fn aggregate_computes_active_grants() {
        let achievements = vec![test_achievement("fixup", "Leftovers")];
        let repositories = vec![test_repo("repo")];
        let events = HashMap::from([(
            "repo".to_string(),
            vec![
                make_event(
                    "alice@example.com",
                    "Alice",
                    "fixup",
                    AchievementEventKind::Grant,
                    100,
                ),
                make_event(
                    "bob@example.com",
                    "Bob",
                    "fixup",
                    AchievementEventKind::Grant,
                    200,
                ),
                make_event(
                    "alice@example.com",
                    "Alice",
                    "fixup",
                    AchievementEventKind::Revoke,
                    300,
                ),
            ],
        )]);
        let users = vec![
            test_user("alice@example.com", "Alice", "alice"),
            test_user("bob@example.com", "Bob", "bob"),
        ];

        let site = aggregate(&achievements, &repositories, &events, &users);

        // Alice was revoked, only Bob holds it
        assert_eq!(site.achievements[0].holders.len(), 1);
        assert_eq!(site.achievements[0].holders[0].user_name, "Bob");
        assert_eq!(site.achievements[0].total_grants, 2);
        assert_eq!(site.achievements[0].unique_holders, 1);
    }

    #[test]
    fn aggregate_user_achievements_grouped_by_repo() {
        let achievements = vec![test_achievement("fixup", "Leftovers")];
        let repositories = vec![test_repo("repo-a"), test_repo("repo-b")];
        let events = HashMap::from([
            (
                "repo-a".to_string(),
                vec![make_event(
                    "alice@example.com",
                    "Alice",
                    "fixup",
                    AchievementEventKind::Grant,
                    100,
                )],
            ),
            (
                "repo-b".to_string(),
                vec![make_event(
                    "alice@example.com",
                    "Alice",
                    "fixup",
                    AchievementEventKind::Grant,
                    200,
                )],
            ),
        ]);
        let users = vec![test_user("alice@example.com", "Alice", "alice")];

        let site = aggregate(&achievements, &repositories, &events, &users);

        let alice = &site.users[0];
        assert_eq!(alice.achievements_by_repo.len(), 2);
        assert_eq!(alice.active_achievements, 2);
        assert_eq!(alice.repos_contributed_to, 2);
    }

    #[test]
    fn aggregate_recent_activity_sorted_reverse_chronological() {
        let achievements = vec![test_achievement("fixup", "Leftovers")];
        let repositories = vec![test_repo("repo")];
        let events = HashMap::from([(
            "repo".to_string(),
            vec![
                make_event(
                    "alice@example.com",
                    "Alice",
                    "fixup",
                    AchievementEventKind::Grant,
                    100,
                ),
                make_event(
                    "bob@example.com",
                    "Bob",
                    "fixup",
                    AchievementEventKind::Grant,
                    300,
                ),
                make_event(
                    "carol@example.com",
                    "Carol",
                    "fixup",
                    AchievementEventKind::Grant,
                    200,
                ),
            ],
        )]);
        let users = vec![
            test_user("alice@example.com", "Alice", "alice"),
            test_user("bob@example.com", "Bob", "bob"),
            test_user("carol@example.com", "Carol", "carol"),
        ];

        let site = aggregate(&achievements, &repositories, &events, &users);

        assert_eq!(site.recent_activity.len(), 3);
        assert_eq!(site.recent_activity[0].user_name, "Bob");
        assert_eq!(site.recent_activity[1].user_name, "Carol");
        assert_eq!(site.recent_activity[2].user_name, "Alice");
    }
}
