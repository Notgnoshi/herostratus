pub use crate::achievement::{Achievement, AchievementDescriptor, Rule};

pub struct AlwaysFail {
    desc: [AchievementDescriptor; 1],
}

impl Default for AlwaysFail {
    fn default() -> Self {
        Self {
            desc: [AchievementDescriptor {
                enabled: true,
                id: 1,
                human_id: "always-fail",
                name: "Always Fail",
                description: "This rule always fails to grant an achievement",
            }],
        }
    }
}
impl Rule for AlwaysFail {
    fn get_descriptors(&self) -> &[AchievementDescriptor] {
        &self.desc
    }
    fn get_descriptors_mut(&mut self) -> &mut [AchievementDescriptor] {
        &mut self.desc
    }
    fn process(&mut self, _commit: &gix::Commit, _repo: &gix::Repository) -> Option<Achievement> {
        None
    }
}

pub struct ParticipationTrophy {
    desc: [AchievementDescriptor; 1],
}
impl Default for ParticipationTrophy {
    fn default() -> Self {
        Self {
            desc: [AchievementDescriptor {
                enabled: true,
                id: 2,
                human_id: "participation-trophy",
                name: "Always succeed",
                description: "This rule always grants an achievement",
            }],
        }
    }
}
impl Rule for ParticipationTrophy {
    fn get_descriptors(&self) -> &[AchievementDescriptor] {
        &self.desc
    }
    fn get_descriptors_mut(&mut self) -> &mut [AchievementDescriptor] {
        &mut self.desc
    }
    fn process(&mut self, commit: &gix::Commit, _repo: &gix::Repository) -> Option<Achievement> {
        tracing::debug!("Granting {:?} a participation trophy", commit.id());
        Some(Achievement {
            name: "",
            commit: commit.id,
        })
    }
}

pub struct ParticipationTrophy2 {
    desc: [AchievementDescriptor; 1],
}
impl Default for ParticipationTrophy2 {
    fn default() -> Self {
        Self {
            desc: [AchievementDescriptor {
                enabled: true,
                id: 3,
                human_id: "participation-trophy-2",
                name: "Always succeed at finalize",
                description: "This rule always grants an achievement at finalize",
            }],
        }
    }
}
impl Rule for ParticipationTrophy2 {
    fn get_descriptors(&self) -> &[AchievementDescriptor] {
        &self.desc
    }
    fn get_descriptors_mut(&mut self) -> &mut [AchievementDescriptor] {
        &mut self.desc
    }

    fn process(&mut self, _commit: &gix::Commit, _repo: &gix::Repository) -> Option<Achievement> {
        None
    }

    fn finalize(&mut self, _repo: &gix::Repository) -> Vec<Achievement> {
        tracing::debug!("Finalizing ParticipationTrophy2");
        vec![Achievement {
            name: "",
            commit: gix::ObjectId::null(gix::index::hash::Kind::Sha1),
        }]
    }
}
