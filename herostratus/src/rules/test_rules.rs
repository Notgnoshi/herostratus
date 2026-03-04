use std::mem::Discriminant;

use crate::achievement::{AchievementKind, Grant, Meta};
use crate::observer::{CommitContext, Observation};
use crate::rules::rule::Rule;

/// A rule that grants on every Observation::Dummy, no cache. For basic dispatch tests.
pub struct GrantOnDummy {
    meta: Meta,
}

impl GrantOnDummy {
    pub fn new(id: usize) -> Self {
        Self {
            meta: Meta {
                id,
                human_id: "grant-on-dummy",
                name: "Grant On Dummy",
                description: "Grants on every Dummy observation",
                kind: AchievementKind::PerUser { recurrent: false },
            },
        }
    }
}

impl Rule for GrantOnDummy {
    type Cache = ();

    fn meta(&self) -> &Meta {
        &self.meta
    }

    fn consumes(&self) -> &'static [Discriminant<Observation>] {
        &[Observation::DUMMY]
    }

    fn process(&mut self, ctx: &CommitContext, obs: &Observation) -> eyre::Result<Option<Grant>> {
        match obs {
            Observation::Dummy => Ok(Some(self.meta.grant(ctx))),
            _ => Ok(None),
        }
    }
}

/// A rule that counts Dummy observations and grants in finalize. For testing caching, finalize,
/// and stateful behavior.
pub struct CountingRule {
    meta: Meta,
    count: usize,
    last_ctx: Option<CommitContext>,
}

impl CountingRule {
    pub fn new(id: usize) -> Self {
        Self {
            meta: Meta {
                id,
                human_id: "counting-rule",
                name: "Counting Rule",
                description: "Counts Dummy observations and grants in finalize",
                kind: AchievementKind::PerUser { recurrent: false },
            },
            count: 0,
            last_ctx: None,
        }
    }
}

impl Rule for CountingRule {
    type Cache = usize;

    fn meta(&self) -> &Meta {
        &self.meta
    }

    fn consumes(&self) -> &'static [Discriminant<Observation>] {
        &[Observation::DUMMY]
    }

    fn process(&mut self, ctx: &CommitContext, obs: &Observation) -> eyre::Result<Option<Grant>> {
        if matches!(obs, Observation::Dummy) {
            self.count += 1;
            self.last_ctx = Some(ctx.clone());
        }
        Ok(None)
    }

    fn finalize(&mut self) -> eyre::Result<Option<Grant>> {
        if self.count > 0 {
            let ctx = self.last_ctx.as_ref().expect("counted but no context");
            Ok(Some(self.meta.grant(ctx)))
        } else {
            Ok(None)
        }
    }

    fn init_cache(&mut self, cache: Self::Cache) {
        self.count = cache;
    }

    fn fini_cache(&self) -> Self::Cache {
        self.count
    }
}
