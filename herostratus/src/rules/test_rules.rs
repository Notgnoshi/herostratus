pub use crate::achievement::{Achievement, Rule};

pub struct AlwaysFail;
impl Rule for AlwaysFail {
    fn id(&self) -> usize {
        1
    }
    fn human_id(&self) -> &'static str {
        "always-fail"
    }
    fn name(&self) -> &'static str {
        ""
    }
    fn description(&self) -> &'static str {
        ""
    }
    fn process(&mut self, _commit: &git2::Commit, _repo: &git2::Repository) -> Option<Achievement> {
        None
    }
}

pub struct ParticipationTrophy;
impl Rule for ParticipationTrophy {
    fn id(&self) -> usize {
        2
    }
    fn human_id(&self) -> &'static str {
        "participation-trophy"
    }
    fn name(&self) -> &'static str {
        ""
    }
    fn description(&self) -> &'static str {
        ""
    }
    fn process(&mut self, commit: &git2::Commit, repo: &git2::Repository) -> Option<Achievement> {
        tracing::debug!("Granting {:?} a participation trophy", commit.id());
        Some(self.grant(commit, repo))
    }
}

pub struct ParticipationTrophy2;
impl Rule for ParticipationTrophy2 {
    fn id(&self) -> usize {
        3
    }
    fn human_id(&self) -> &'static str {
        "participation-trophy-2"
    }
    fn name(&self) -> &'static str {
        ""
    }
    fn description(&self) -> &'static str {
        ""
    }
    fn process(&mut self, _commit: &git2::Commit, _repo: &git2::Repository) -> Option<Achievement> {
        None
    }

    fn finalize(&mut self, _repo: &git2::Repository) -> Vec<Achievement> {
        tracing::debug!("Finalizing ParticipationTrophy2");
        vec![Achievement {
            commit: git2::Oid::zero(),
            name: self.name(),
        }]
    }
}
