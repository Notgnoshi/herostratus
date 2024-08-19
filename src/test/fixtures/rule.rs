use crate::achievement::{Achievement, Rule};

pub struct AlwaysFail;
impl Rule for AlwaysFail {
    fn name(&self) -> &'static str {
        "AlwaysFail"
    }
    fn process(&mut self, _commit: &git2::Commit, _repo: &git2::Repository) -> Option<Achievement> {
        None
    }
}

pub struct ParticipationTrophy;
impl Rule for ParticipationTrophy {
    fn name(&self) -> &'static str {
        "Participation Trophy"
    }
    fn process(&mut self, commit: &git2::Commit, repo: &git2::Repository) -> Option<Achievement> {
        tracing::debug!("Granting {:?} a participation trophy", commit.id());
        Some(self.grant(commit, repo))
    }
}

pub struct ParticipationTrophy2;
impl Rule for ParticipationTrophy2 {
    fn name(&self) -> &'static str {
        "Participation Trophy 2"
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
