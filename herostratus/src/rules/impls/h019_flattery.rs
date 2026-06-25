use std::collections::{HashMap, HashSet};
use std::mem::Discriminant;

use crate::achievement::{AchievementKind, Grant, Meta};
use crate::observer::{CommitContext, Observation};
use crate::rules::rule::Rule;
use crate::rules::rule_plugin::RuleFactory;

const META: Meta = Meta {
    id: 19,
    human_id: "flattery",
    name: "Imitation Is the Sincerest Form of Flattery",
    description: "Copy a previous commit's subject line",
    kind: AchievementKind::PerUser { recurrent: true },
};

const THRESHOLDS: &[(usize, &str)] = &[
    (1, "Imitation Is the Sincerest Form of Flattery"),
    (5, "Copy-Paste Connoisseur"),
    (10, "Ctrl+C, Ctrl+Commit"),
    (20, "Serial Plagiarist"),
];

fn normalize(subject: &str) -> String {
    subject
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

/// 64-bit FNV-1a. Needs to be stable across releases and platforms, so cached hashes stay valid!
fn hash_subject(normalized: &str) -> u64 {
    const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;
    let mut hash = FNV_OFFSET;
    for byte in normalized.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

/// One subject occurrence buffered during this run.
struct Occurrence {
    ctx: CommitContext,
    hash: u64,
}

#[derive(Default, Clone, serde::Deserialize, serde::Serialize)]
pub struct FlatteryCache {
    /// Hashes of every normalized subject ever seen.
    seen: HashSet<u64>,
    /// Cumulative per-author copy count, for milestone tracking.
    user_counts: HashMap<String, usize>,
}

/// Grant an achievement to commits that reuse a subject line seen in an older commit.
///
/// The commit walk is newest-first, so "is this a copy?" cannot be decided when a commit is first
/// seen. So we buffer this run's subjects and resolve copies in [Self::finalize].
#[derive(Default)]
pub struct Flattery {
    cache: FlatteryCache,
    buffer: Vec<Occurrence>,
}
inventory::submit!(RuleFactory::default::<Flattery>());

impl Rule for Flattery {
    type Cache = FlatteryCache;

    fn meta(&self) -> &Meta {
        &META
    }

    fn consumes(&self) -> &'static [Discriminant<Observation>] {
        &[Observation::SUBJECT]
    }

    fn process(&mut self, ctx: &CommitContext, obs: &Observation) -> eyre::Result<Option<Grant>> {
        let Observation::Subject { subject } = obs else {
            return Ok(None);
        };
        let hash = hash_subject(&normalize(subject));
        self.buffer.push(Occurrence {
            ctx: ctx.clone(),
            hash,
        });
        Ok(None)
    }

    fn finalize(&mut self) -> eyre::Result<Vec<Grant>> {
        // Group this run's occurrences by subject hash.
        let mut groups: HashMap<u64, Vec<Occurrence>> = HashMap::new();
        for occ in self.buffer.drain(..) {
            groups.entry(occ.hash).or_default().push(occ);
        }

        // Find copies of subjects found during this run
        let mut copies: Vec<Occurrence> = Vec::new();
        for (hash, mut occs) in groups {
            occs.sort_by_key(|o| (o.ctx.commit_timestamp, o.ctx.oid));
            let had_prior = self.cache.seen.contains(&hash);
            self.cache.seen.insert(hash);
            if had_prior {
                copies.extend(occs);
            } else {
                copies.extend(occs.into_iter().skip(1));
            }
        }

        // Replay copies in chronological order so per-user milestones accrue correctly.
        copies.sort_by_key(|o| (o.ctx.commit_timestamp, o.ctx.oid));
        let mut grants = Vec::new();
        for occ in copies {
            let count = self
                .cache
                .user_counts
                .entry(occ.ctx.author_email.clone())
                .or_insert(0);
            *count += 1;
            if let Some((_, title)) = THRESHOLDS.iter().find(|(t, _)| *t == *count) {
                grants.push(META.grant(&occ.ctx).with_name((*title).to_string()));
            }
        }
        Ok(grants)
    }

    fn init_cache(&mut self, cache: Self::Cache) {
        self.cache = cache;
    }

    fn fini_cache(&self) -> Self::Cache {
        self.cache.clone()
    }
}

#[cfg(test)]
mod tests {
    use chrono::{DateTime, Utc};

    use super::*;
    use crate::observer::CommitContext;

    fn ctx(name: &str, ts: i64) -> CommitContext {
        CommitContext {
            oid: gix::ObjectId::null(gix::hash::Kind::Sha1),
            author_name: name.to_string(),
            author_email: format!("{}@example.com", name.to_lowercase()),
            commit_timestamp: DateTime::<Utc>::from_timestamp(ts, 0).unwrap(),
        }
    }

    fn subj(s: &str) -> Observation {
        Observation::Subject {
            subject: s.to_string(),
        }
    }

    #[test]
    fn normalize_trims_collapses_and_lowercases() {
        assert_eq!(normalize("  WIP "), "wip");
        assert_eq!(normalize("Merge   main"), "merge main");
        assert_eq!(normalize("wip"), normalize("WIP"));
    }

    #[test]
    fn equal_normalized_subjects_hash_equal() {
        assert_eq!(
            hash_subject(&normalize("WIP")),
            hash_subject(&normalize("wip "))
        );
    }

    #[test]
    fn copier_earns_original_does_not() {
        let mut rule = Flattery::default();
        rule.process(&ctx("Alice", 1000), &subj("wip")).unwrap();
        rule.process(&ctx("Bob", 2000), &subj("wip")).unwrap();
        let grants = rule.finalize().unwrap();
        assert_eq!(grants.len(), 1);
        assert_eq!(grants[0].user_name, "Bob");
        assert_eq!(
            grants[0].name_override.as_deref(),
            Some("Imitation Is the Sincerest Form of Flattery")
        );
    }

    #[test]
    fn copying_yourself_counts() {
        let mut rule = Flattery::default();
        rule.process(&ctx("Alice", 1000), &subj("wip")).unwrap();
        rule.process(&ctx("Alice", 2000), &subj("wip")).unwrap();
        let grants = rule.finalize().unwrap();
        assert_eq!(grants.len(), 1);
        assert_eq!(grants[0].user_name, "Alice");
    }

    #[test]
    fn normalization_collides_case_and_whitespace() {
        let mut rule = Flattery::default();
        rule.process(&ctx("Alice", 1000), &subj("WIP")).unwrap();
        rule.process(&ctx("Bob", 2000), &subj("  wip ")).unwrap();
        let grants = rule.finalize().unwrap();
        assert_eq!(grants.len(), 1);
        assert_eq!(grants[0].user_name, "Bob");
    }

    #[test]
    fn unique_subject_grants_nothing() {
        let mut rule = Flattery::default();
        rule.process(&ctx("Alice", 1000), &subj("a unique subject"))
            .unwrap();
        let grants = rule.finalize().unwrap();
        assert!(grants.is_empty());
    }

    #[test]
    fn cache_detects_copy_in_later_run() {
        let mut run1 = Flattery::default();
        run1.process(&ctx("Alice", 1000), &subj("wip")).unwrap();
        let grants1 = run1.finalize().unwrap();
        assert!(grants1.is_empty(), "unique subject grants nothing in run 1");
        let cache = run1.fini_cache();
        assert!(cache.seen.contains(&hash_subject(&normalize("wip"))));

        let mut run2 = Flattery::default();
        run2.init_cache(cache);
        run2.process(&ctx("Bob", 2000), &subj("wip")).unwrap();
        let grants2 = run2.finalize().unwrap();
        assert_eq!(grants2.len(), 1);
        assert_eq!(grants2[0].user_name, "Bob");
    }

    #[test]
    fn milestones_escalate_per_user() {
        let mut rule = Flattery::default();
        // Five distinct subjects, each authored first by Alice (original) then copied by Bob.
        for i in 0..5 {
            let s = format!("subject {i}");
            rule.process(&ctx("Alice", 1000 + i), &subj(&s)).unwrap();
            rule.process(&ctx("Bob", 2000 + i), &subj(&s)).unwrap();
        }
        let grants = rule.finalize().unwrap();
        // Bob earns at copy #1 and copy #5; Alice (always the original) earns nothing.
        assert_eq!(grants.len(), 2);
        assert!(grants.iter().all(|g| g.user_name == "Bob"));
        assert_eq!(
            grants[0].name_override.as_deref(),
            Some("Imitation Is the Sincerest Form of Flattery")
        );
        assert_eq!(
            grants[1].name_override.as_deref(),
            Some("Copy-Paste Connoisseur")
        );
    }

    #[test]
    fn counts_are_per_user() {
        let mut rule = Flattery::default();
        rule.process(&ctx("Alice", 1000), &subj("wip")).unwrap(); // original
        rule.process(&ctx("Bob", 2000), &subj("wip")).unwrap(); // Bob copy #1
        rule.process(&ctx("Carol", 3000), &subj("wip")).unwrap(); // Carol copy #1
        let grants = rule.finalize().unwrap();
        assert_eq!(grants.len(), 2);
        let mut names: Vec<_> = grants.iter().map(|g| g.user_name.as_str()).collect();
        names.sort();
        assert_eq!(names, ["Bob", "Carol"]);
    }
}
