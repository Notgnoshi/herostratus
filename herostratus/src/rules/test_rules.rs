use crate::achievement::{Achievement, AchievementDescriptor};
use crate::rules::Rule;

const ALWAYS_FAIL_DESCRIPTORS: [AchievementDescriptor; 1] = [AchievementDescriptor {
    id: 1,
    human_id: "always-fail",
    name: "Always Fail",
    description: "This rule always fails to grant an achievement",
}];

#[derive(Default)]
pub struct AlwaysFail;

impl Rule for AlwaysFail {
    type Cache = ();

    fn descriptors(&self) -> &[AchievementDescriptor] {
        &ALWAYS_FAIL_DESCRIPTORS
    }
    fn process(&mut self, _commit: &gix::Commit, _repo: &gix::Repository) -> Vec<Achievement> {
        Vec::new()
    }
}

const PARTICIPATION_TROPHY_DESCRIPTORS: [AchievementDescriptor; 1] = [AchievementDescriptor {
    id: 2,
    human_id: "participation-trophy",
    name: "Always succeed",
    description: "This rule always grants an achievement",
}];

#[derive(Default)]
pub struct ParticipationTrophy;

impl Rule for ParticipationTrophy {
    type Cache = ();

    fn descriptors(&self) -> &[AchievementDescriptor] {
        &PARTICIPATION_TROPHY_DESCRIPTORS
    }
    fn process(&mut self, commit: &gix::Commit, _repo: &gix::Repository) -> Vec<Achievement> {
        tracing::debug!("Granting {:?} a participation trophy", commit.id());
        vec![PARTICIPATION_TROPHY_DESCRIPTORS[0].grant(commit.id)]
    }
}

const PARTICIPATION_TROPHY2_DESCRIPTORS: [AchievementDescriptor; 1] = [AchievementDescriptor {
    id: 3,
    human_id: "participation-trophy-2",
    name: "Always succeed at finalize",
    description: "This rule always grants an achievement at finalize",
}];

#[derive(Default)]
pub struct ParticipationTrophy2 {
    last_commit: Option<gix::ObjectId>,
}

impl Rule for ParticipationTrophy2 {
    type Cache = ();

    fn descriptors(&self) -> &[AchievementDescriptor] {
        &PARTICIPATION_TROPHY2_DESCRIPTORS
    }

    fn process(&mut self, commit: &gix::Commit, _repo: &gix::Repository) -> Vec<Achievement> {
        self.last_commit = Some(commit.id);
        Vec::new()
    }

    fn finalize(&mut self, _repo: &gix::Repository) -> Vec<Achievement> {
        tracing::debug!("Finalizing ParticipationTrophy2");
        let oid = self.last_commit.expect("process() was never called");
        vec![PARTICIPATION_TROPHY2_DESCRIPTORS[0].grant(oid)]
    }
}

pub struct FlexibleRule {
    pub descriptors: Vec<AchievementDescriptor>,
}

impl Rule for FlexibleRule {
    type Cache = ();

    fn descriptors(&self) -> &[AchievementDescriptor] {
        &self.descriptors
    }

    fn process(&mut self, commit: &gix::Commit, _repo: &gix::Repository) -> Vec<Achievement> {
        self.descriptors
            .iter()
            .map(|d| d.grant(commit.id))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::RulePlugin;

    #[test]
    fn test_erased_rule_name() {
        // You still get the concrete type name even after the rule has been type-erased
        let rule: Box<dyn RulePlugin> = Box::new(AlwaysFail);
        assert_eq!(rule.name(), "AlwaysFail");
    }
}
