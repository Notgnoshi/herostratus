use std::collections::HashMap;

use crate::achievement::AchievementLogEvent;

/// A user derived from achievement events.
#[derive(Debug, serde::Serialize)]
pub struct User {
    pub email: String,
    pub name: String,
    pub slug: String,
}

/// Derive users from all achievement events across all repositories.
///
/// For each unique email, the display name is taken from the event with the latest timestamp.
/// Slugs are generated from the display name, with numeric suffixes to break collisions. Ties are
/// broken by earliest event timestamp (the first-seen user gets the bare slug).
pub fn derive_users(all_events: &HashMap<String, Vec<AchievementLogEvent>>) -> Vec<User> {
    // Collect per-email: latest name and earliest timestamp
    let mut by_email: HashMap<
        &str,
        (
            &str,
            &chrono::DateTime<chrono::Utc>,
            &chrono::DateTime<chrono::Utc>,
        ),
    > = HashMap::new();

    for events in all_events.values() {
        for event in events {
            by_email
                .entry(&event.user_email)
                .and_modify(|(name, earliest, latest)| {
                    if event.timestamp < **earliest {
                        *earliest = &event.timestamp;
                    }
                    if event.timestamp > **latest {
                        *latest = &event.timestamp;
                        *name = &event.user_name;
                    }
                })
                .or_insert((&event.user_name, &event.timestamp, &event.timestamp));
        }
    }

    // Sort by earliest event timestamp for deterministic slug assignment
    let mut entries: Vec<_> = by_email
        .into_iter()
        .map(|(email, (name, earliest, _latest))| (email, name, earliest))
        .collect();
    entries.sort_by_key(|&(_, _, earliest)| *earliest);

    // Assign slugs with collision handling
    let mut slug_counts: HashMap<String, usize> = HashMap::new();
    let mut users = Vec::with_capacity(entries.len());

    for (email, name, _) in entries {
        let base_slug = slugify(name);
        let count = slug_counts.entry(base_slug.clone()).or_insert(0);
        *count += 1;
        let slug = if *count == 1 {
            base_slug
        } else {
            format!("{base_slug}-{count}")
        };

        users.push(User {
            email: email.to_string(),
            name: name.to_string(),
            slug,
        });
    }

    users
}

/// Convert a display name to a URL-safe slug.
///
/// Unicode characters are transliterated to ASCII (e.g. "Rene" -> "rene") before
/// lowercasing. Non-alphanumeric characters are replaced with hyphens, consecutive hyphens are
/// collapsed, and leading/trailing hyphens are trimmed.
fn slugify(name: &str) -> String {
    let ascii = deunicode::deunicode(name);
    let mut slug = String::with_capacity(ascii.len());
    for c in ascii.chars() {
        if c.is_ascii_alphanumeric() {
            slug.push(c.to_ascii_lowercase());
        } else {
            slug.push('-');
        }
    }

    // Collapse consecutive hyphens
    let mut collapsed = String::with_capacity(slug.len());
    let mut prev_hyphen = false;
    for c in slug.chars() {
        if c == '-' {
            if !prev_hyphen {
                collapsed.push('-');
            }
            prev_hyphen = true;
        } else {
            collapsed.push(c);
            prev_hyphen = false;
        }
    }

    collapsed.trim_matches('-').to_string()
}

#[cfg(test)]
mod tests {
    use chrono::TimeZone;

    use super::*;

    fn make_event(
        email: &str,
        name: &str,
        timestamp: chrono::DateTime<chrono::Utc>,
    ) -> AchievementLogEvent {
        AchievementLogEvent {
            timestamp,
            event: crate::achievement::AchievementEventKind::Grant,
            achievement_id: "test".to_string(),
            commit: gix::ObjectId::from_bytes_or_panic(&[0xAA; 20]),
            user_name: name.to_string(),
            user_email: email.to_string(),
        }
    }

    fn ts(secs: i64) -> chrono::DateTime<chrono::Utc> {
        chrono::Utc.timestamp_opt(secs, 0).unwrap()
    }

    #[test]
    fn slugify_simple_name() {
        assert_eq!(slugify("Alice Smith"), "alice-smith");
    }

    #[test]
    fn slugify_special_characters() {
        assert_eq!(slugify("O'Brien-Jones"), "o-brien-jones");
    }

    #[test]
    fn slugify_unicode_transliteration() {
        assert_eq!(slugify("René Müller"), "rene-muller");
        assert_eq!(slugify("Øyvind"), "oyvind");
        assert_eq!(slugify("Paweł Nować"), "pawel-nowac");
    }

    #[test]
    fn slugify_collapses_consecutive_hyphens() {
        assert_eq!(slugify("a   b"), "a-b");
        assert_eq!(slugify("a---b"), "a-b");
    }

    #[test]
    fn slugify_trims_leading_trailing_hyphens() {
        assert_eq!(slugify(" alice "), "alice");
        assert_eq!(slugify("--alice--"), "alice");
    }

    #[test]
    fn derive_users_picks_latest_name() {
        let events = HashMap::from([(
            "repo".to_string(),
            vec![
                make_event("alice@example.com", "alice", ts(100)),
                make_event("alice@example.com", "Alice Smith", ts(200)),
            ],
        )]);

        let users = derive_users(&events);
        assert_eq!(users.len(), 1);
        assert_eq!(users[0].name, "Alice Smith");
        assert_eq!(users[0].slug, "alice-smith");
    }

    #[test]
    fn derive_users_slug_collision() {
        // Two different emails that slugify to the same thing
        let events = HashMap::from([(
            "repo".to_string(),
            vec![
                make_event("alice1@example.com", "Alice Smith", ts(100)),
                make_event("alice2@example.com", "Alice Smith", ts(200)),
            ],
        )]);

        let users = derive_users(&events);
        assert_eq!(users.len(), 2);
        // First-seen gets bare slug
        assert_eq!(users[0].slug, "alice-smith");
        assert_eq!(users[0].email, "alice1@example.com");
        // Second gets numeric suffix
        assert_eq!(users[1].slug, "alice-smith-2");
        assert_eq!(users[1].email, "alice2@example.com");
    }

    #[test]
    fn derive_users_across_repos() {
        let events = HashMap::from([
            (
                "repo-a".to_string(),
                vec![make_event("alice@example.com", "Alice", ts(100))],
            ),
            (
                "repo-b".to_string(),
                vec![make_event("alice@example.com", "Alice Smith", ts(200))],
            ),
        ]);

        let users = derive_users(&events);
        assert_eq!(users.len(), 1);
        // Latest name wins across repos
        assert_eq!(users[0].name, "Alice Smith");
    }
}
