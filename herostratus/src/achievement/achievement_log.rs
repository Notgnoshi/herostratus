use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};

use super::grant::Grant;
use super::meta::{AchievementKind, Meta};

/// Whether an achievement was granted or revoked.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EventKind {
    Grant,
    Revoke,
}

/// A timestamped record of a grant or revocation in the achievement log.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AchievementEvent {
    pub timestamp: DateTime<Utc>,
    pub event: EventKind,
    pub achievement_id: String,
    #[serde(with = "object_id_serde")]
    pub commit: gix::ObjectId,
    pub user_name: String,
    pub user_email: String,
}

impl AchievementEvent {
    fn from_meta_and_grant(meta: &Meta, grant: Grant) -> Self {
        Self {
            timestamp: grant.timestamp,
            event: EventKind::Grant,
            achievement_id: meta.human_id.to_string(),
            commit: grant.commit,
            user_name: grant.user_name,
            user_email: grant.user_email,
        }
    }
}

/// The result of resolving a grant through the achievement log.
///
/// Contains the grant event that was recorded, and optionally a revocation of the previous holder
/// (for [Global { revocable: true }](AchievementKind::Global) achievements).
pub struct Resolution {
    pub revoke: Option<AchievementEvent>,
    pub grant: AchievementEvent,
}

/// Persistent log of achievement grants and revocations.
///
/// Records events as timestamped rows in a CSV file and enforces [AchievementKind] semantics:
/// deduplication for [PerUser](AchievementKind::PerUser), uniqueness/revocation for
/// [Global](AchievementKind::Global).
pub struct AchievementLog {
    path: Option<PathBuf>,
    events: Vec<AchievementEvent>,
}

impl AchievementLog {
    /// Load an achievement log from a CSV file, or create an empty log.
    ///
    /// If `path` is `None` or the file does not exist, returns an empty log.
    #[tracing::instrument(target = "perf", skip_all)]
    pub fn load(path: Option<&Path>) -> eyre::Result<Self> {
        let Some(path) = path else {
            return Ok(Self {
                path: None,
                events: Vec::new(),
            });
        };

        let events = if path.exists() {
            let mut reader = csv::Reader::from_path(path)?;
            let events: Result<Vec<AchievementEvent>, _> = reader.deserialize().collect();
            let events = events?;
            tracing::debug!(
                "Loaded achievement log ({} events) from {path:?}",
                events.len()
            );
            events
        } else {
            tracing::debug!("Initializing new achievement log from {path:?}");
            Vec::new()
        };

        Ok(Self {
            path: Some(path.to_path_buf()),
            events,
        })
    }

    /// Write the full log to CSV. No-op if path is `None`.
    #[tracing::instrument(target = "perf", skip_all)]
    pub fn save(&self) -> eyre::Result<()> {
        let Some(path) = &self.path else {
            return Ok(());
        };
        if let Some(parent) = path.parent()
            && !parent.exists()
        {
            std::fs::create_dir_all(parent)?;
        }

        let mut writer = csv::Writer::from_path(path)?;
        for event in &self.events {
            writer.serialize(event)?;
        }
        writer.flush()?;
        tracing::debug!(
            "Wrote achievement log ({} events) to {path:?}",
            self.events.len()
        );
        Ok(())
    }

    /// Append a grant event to the log.
    fn record_grant(&mut self, event: AchievementEvent) {
        debug_assert_eq!(event.event, EventKind::Grant);
        self.events.push(event);
    }

    /// Append a revocation event for the given achievement and user.
    ///
    /// Copies the commit/user fields from the grant being revoked. The `timestamp` is the
    /// timestamp of the new grant that is superseding the old one, so the revocation appears at
    /// the correct point in the timeline.
    fn record_revocation(
        &mut self,
        achievement_id: &str,
        user_email: &str,
        timestamp: DateTime<Utc>,
    ) {
        // Find the grant being revoked to copy its fields
        let grant = self
            .events
            .iter()
            .rev()
            .find(|e| {
                e.event == EventKind::Grant
                    && e.achievement_id == achievement_id
                    && e.user_email == user_email
            })
            .expect("record_revocation called without a matching grant");

        let revoke = AchievementEvent {
            timestamp,
            event: EventKind::Revoke,
            achievement_id: achievement_id.to_string(),
            commit: grant.commit,
            user_name: grant.user_name.clone(),
            user_email: grant.user_email.clone(),
        };
        self.events.push(revoke);
    }

    /// True if there is an active (non-revoked) grant for this achievement and user.
    fn is_granted_to(&self, achievement_id: &str, user_email: &str) -> bool {
        let mut granted = false;
        for event in &self.events {
            if event.achievement_id != achievement_id || event.user_email != user_email {
                continue;
            }
            match event.event {
                EventKind::Grant => granted = true,
                EventKind::Revoke => granted = false,
            }
        }
        granted
    }

    /// The most recent non-revoked grant event for this achievement (any user).
    fn current_holder(&self, achievement_id: &str) -> Option<&AchievementEvent> {
        // Find the most recent grant that hasn't been revoked.
        // For Global achievements, there should be at most one active holder.
        let mut holder: Option<&AchievementEvent> = None;
        for event in &self.events {
            if event.achievement_id != achievement_id {
                continue;
            }
            match event.event {
                EventKind::Grant => holder = Some(event),
                EventKind::Revoke if holder.is_some_and(|h| h.user_email == event.user_email) => {
                    holder = None;
                }
                EventKind::Revoke => {}
            }
        }
        holder
    }

    /// All grant events that have no subsequent revocation for the same achievement+user.
    pub fn active_grants(&self) -> impl Iterator<Item = &AchievementEvent> {
        // Collect active grants by scanning all events
        let mut active: Vec<&AchievementEvent> = Vec::new();
        for event in &self.events {
            match event.event {
                EventKind::Grant => active.push(event),
                EventKind::Revoke => {
                    active.retain(|g| {
                        !(g.achievement_id == event.achievement_id
                            && g.user_email == event.user_email)
                    });
                }
            }
        }
        active.into_iter()
    }

    /// Resolve a grant through the achievement log, enforcing [AchievementKind] semantics.
    ///
    /// Returns `Some(Resolution)` if the grant should be emitted, `None` if it should be
    /// suppressed (e.g., duplicate per-user grant, or global achievement already held).
    pub fn resolve(&mut self, meta: &Meta, grant: Grant) -> Option<Resolution> {
        match meta.kind {
            AchievementKind::PerUser { recurrent: false } => {
                if self.is_granted_to(meta.human_id, &grant.user_email) {
                    return None;
                }
                let event = AchievementEvent::from_meta_and_grant(meta, grant);
                self.record_grant(event.clone());
                Some(Resolution {
                    revoke: None,
                    grant: event,
                })
            }
            AchievementKind::PerUser { recurrent: true } => {
                let event = AchievementEvent::from_meta_and_grant(meta, grant);
                self.record_grant(event.clone());
                Some(Resolution {
                    revoke: None,
                    grant: event,
                })
            }
            AchievementKind::Global { revocable: false } => {
                if self.current_holder(meta.human_id).is_some() {
                    return None;
                }
                let event = AchievementEvent::from_meta_and_grant(meta, grant);
                self.record_grant(event.clone());
                Some(Resolution {
                    revoke: None,
                    grant: event,
                })
            }
            AchievementKind::Global { revocable: true } => {
                if let Some(holder) = self.current_holder(meta.human_id) {
                    if holder.user_email == grant.user_email {
                        // Same user already holds it -- skip
                        return None;
                    }
                    // Different user -- revoke the previous holder, grant to new
                    let prev_email = holder.user_email.clone();
                    self.record_revocation(meta.human_id, &prev_email, grant.timestamp);
                    let revoke_event = self.events.last().cloned().unwrap();

                    let grant_event = AchievementEvent::from_meta_and_grant(meta, grant);
                    self.record_grant(grant_event.clone());
                    Some(Resolution {
                        revoke: Some(revoke_event),
                        grant: grant_event,
                    })
                } else {
                    // No current holder -- grant
                    let event = AchievementEvent::from_meta_and_grant(meta, grant);
                    self.record_grant(event.clone());
                    Some(Resolution {
                        revoke: None,
                        grant: event,
                    })
                }
            }
        }
    }
}

/// Serde helpers for round-tripping gix::ObjectId as a hex string.
mod object_id_serde {
    use std::str::FromStr;

    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(oid: &gix::ObjectId, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&oid.to_string())
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<gix::ObjectId, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        gix::ObjectId::from_str(&s).map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::achievement::AchievementKind;

    fn test_oid(byte: u8) -> gix::ObjectId {
        gix::ObjectId::from_bytes_or_panic(&[byte; 20])
    }

    fn test_meta(human_id: &'static str, kind: AchievementKind) -> Meta {
        Meta {
            id: 1,
            human_id,
            name: "Test",
            description: "test achievement",
            kind,
        }
    }

    fn test_grant(oid: gix::ObjectId, name: &str, email: &str) -> Grant {
        Grant {
            commit: oid,
            user_name: name.to_string(),
            user_email: email.to_string(),
            timestamp: chrono::DateTime::UNIX_EPOCH,
            name_override: None,
            description_override: None,
        }
    }

    fn test_event(achievement_id: &str, email: &str, kind: EventKind) -> AchievementEvent {
        AchievementEvent {
            timestamp: Utc::now(),
            event: kind,
            achievement_id: achievement_id.to_string(),
            commit: test_oid(0xAA),
            user_name: "Test".to_string(),
            user_email: email.to_string(),
        }
    }

    #[test]
    fn csv_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("log.csv");

        let events = vec![
            test_event("fixup", "alice@example.com", EventKind::Grant),
            test_event("fixup", "bob@example.com", EventKind::Grant),
            test_event("fixup", "alice@example.com", EventKind::Revoke),
        ];

        // Save
        let log = AchievementLog {
            path: Some(path.clone()),
            events,
        };
        log.save().unwrap();

        // Load
        let loaded = AchievementLog::load(Some(&path)).unwrap();
        assert_eq!(loaded.events.len(), 3);
        assert_eq!(loaded.events[0].event, EventKind::Grant);
        assert_eq!(loaded.events[0].achievement_id, "fixup");
        assert_eq!(loaded.events[0].user_email, "alice@example.com");
        assert_eq!(loaded.events[1].user_email, "bob@example.com");
        assert_eq!(loaded.events[2].event, EventKind::Revoke);

        // Verify ObjectId round-tripped correctly
        assert_eq!(loaded.events[0].commit, test_oid(0xAA));
    }

    #[test]
    fn csv_round_trip_creates_parent_dirs() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested/dirs/log.csv");

        let log = AchievementLog {
            path: Some(path.clone()),
            events: vec![test_event("fixup", "a@b.com", EventKind::Grant)],
        };
        log.save().unwrap();

        let loaded = AchievementLog::load(Some(&path)).unwrap();
        assert_eq!(loaded.events.len(), 1);
    }

    #[test]
    fn load_missing_file_returns_empty() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nonexistent.csv");
        let log = AchievementLog::load(Some(&path)).unwrap();
        assert!(log.events.is_empty());
    }

    #[test]
    fn load_none_path_returns_empty() {
        let log = AchievementLog::load(None).unwrap();
        assert!(log.events.is_empty());
        assert!(log.path.is_none());
    }

    #[test]
    fn save_none_path_is_noop() {
        let log = AchievementLog {
            path: None,
            events: vec![test_event("fixup", "a@b.com", EventKind::Grant)],
        };
        log.save().unwrap();
        // No file written, no error
    }

    #[test]
    fn is_granted_to_grant_then_check() {
        let mut log = AchievementLog::load(None).unwrap();
        log.record_grant(test_event("fixup", "alice@example.com", EventKind::Grant));
        assert!(log.is_granted_to("fixup", "alice@example.com"));
        assert!(!log.is_granted_to("fixup", "bob@example.com"));
        assert!(!log.is_granted_to("other", "alice@example.com"));
    }

    #[test]
    fn is_granted_to_grant_then_revoke() {
        let mut log = AchievementLog::load(None).unwrap();
        log.record_grant(test_event("fixup", "alice@example.com", EventKind::Grant));
        assert!(log.is_granted_to("fixup", "alice@example.com"));

        log.record_revocation("fixup", "alice@example.com", Utc::now());
        assert!(!log.is_granted_to("fixup", "alice@example.com"));
    }

    #[test]
    fn current_holder_lifecycle() {
        let mut log = AchievementLog::load(None).unwrap();

        // No holder initially
        assert!(log.current_holder("fixup").is_none());

        // Grant to alice
        log.record_grant(test_event("fixup", "alice@example.com", EventKind::Grant));
        let holder = log.current_holder("fixup").unwrap();
        assert_eq!(holder.user_email, "alice@example.com");

        // Revoke alice
        log.record_revocation("fixup", "alice@example.com", Utc::now());
        assert!(log.current_holder("fixup").is_none());

        // Grant to bob
        log.record_grant(test_event("fixup", "bob@example.com", EventKind::Grant));
        let holder = log.current_holder("fixup").unwrap();
        assert_eq!(holder.user_email, "bob@example.com");
    }

    #[test]
    fn active_grants_filters_revoked() {
        let mut log = AchievementLog::load(None).unwrap();

        log.record_grant(test_event("a", "alice@example.com", EventKind::Grant));
        log.record_grant(test_event("b", "bob@example.com", EventKind::Grant));
        log.record_grant(test_event("a", "carol@example.com", EventKind::Grant));

        // Revoke alice's "a"
        log.record_revocation("a", "alice@example.com", Utc::now());

        let active: Vec<_> = log.active_grants().collect();
        assert_eq!(active.len(), 2);
        assert!(
            active
                .iter()
                .any(|e| e.achievement_id == "b" && e.user_email == "bob@example.com")
        );
        assert!(
            active
                .iter()
                .any(|e| e.achievement_id == "a" && e.user_email == "carol@example.com")
        );
    }

    #[test]
    fn resolve_per_user_non_recurrent() {
        let meta = test_meta("fixup", AchievementKind::PerUser { recurrent: false });
        let mut log = AchievementLog::load(None).unwrap();

        // First grant succeeds
        let grant = test_grant(test_oid(1), "Alice", "alice@example.com");
        let res = log.resolve(&meta, grant);
        assert!(res.is_some());
        let res = res.unwrap();
        assert!(res.revoke.is_none());
        assert_eq!(res.grant.user_email, "alice@example.com");

        // Second grant to same user is suppressed
        let grant = test_grant(test_oid(2), "Alice", "alice@example.com");
        let res = log.resolve(&meta, grant);
        assert!(res.is_none());

        // Different user can still get it
        let grant = test_grant(test_oid(3), "Bob", "bob@example.com");
        let res = log.resolve(&meta, grant);
        assert!(res.is_some());
    }

    #[test]
    fn resolve_per_user_recurrent() {
        let meta = test_meta("sailor", AchievementKind::PerUser { recurrent: true });
        let mut log = AchievementLog::load(None).unwrap();

        // Multiple grants to same user all succeed
        for i in 0..3 {
            let grant = test_grant(test_oid(i), "Alice", "alice@example.com");
            let res = log.resolve(&meta, grant);
            assert!(res.is_some(), "grant {i} should succeed");
            assert!(res.unwrap().revoke.is_none());
        }
    }

    #[test]
    fn resolve_global_non_revocable() {
        let meta = test_meta("first", AchievementKind::Global { revocable: false });
        let mut log = AchievementLog::load(None).unwrap();

        // First user gets it
        let grant = test_grant(test_oid(1), "Alice", "alice@example.com");
        let res = log.resolve(&meta, grant);
        assert!(res.is_some());
        assert!(res.unwrap().revoke.is_none());

        // Second user is blocked
        let grant = test_grant(test_oid(2), "Bob", "bob@example.com");
        let res = log.resolve(&meta, grant);
        assert!(res.is_none());

        // Same first user is also blocked (already holds it)
        let grant = test_grant(test_oid(3), "Alice", "alice@example.com");
        let res = log.resolve(&meta, grant);
        assert!(res.is_none());
    }

    #[test]
    fn resolve_global_revocable() {
        let meta = test_meta("best", AchievementKind::Global { revocable: true });
        let mut log = AchievementLog::load(None).unwrap();

        // First user gets it (no revocation)
        let grant = test_grant(test_oid(1), "Alice", "alice@example.com");
        let res = log.resolve(&meta, grant).unwrap();
        assert!(res.revoke.is_none());
        assert_eq!(res.grant.user_email, "alice@example.com");

        // Same user is skipped (already holds it)
        let grant = test_grant(test_oid(2), "Alice", "alice@example.com");
        let res = log.resolve(&meta, grant);
        assert!(res.is_none());

        // Different user triggers revocation + new grant
        let grant = test_grant(test_oid(3), "Bob", "bob@example.com");
        let res = log.resolve(&meta, grant).unwrap();
        assert!(res.revoke.is_some());
        let revoke = res.revoke.unwrap();
        assert_eq!(revoke.event, EventKind::Revoke);
        assert_eq!(revoke.user_email, "alice@example.com");
        assert_eq!(res.grant.user_email, "bob@example.com");
    }
}
