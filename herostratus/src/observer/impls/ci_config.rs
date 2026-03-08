use std::mem::Discriminant;

use crate::observer::observation::Observation;
use crate::observer::observer::{DiffAction, Observer};
use crate::observer::observer_factory::ObserverFactory;

/// Emits [Observation::CiConfig] when a commit adds a CI configuration file.
///
/// Recognized CI systems:
/// - GitHub Actions (`.github/workflows/*.yml`, `.github/workflows/*.yaml`)
/// - GitLab CI (`.gitlab-ci.yml`)
/// - Jenkins (`Jenkinsfile`)
/// - Travis CI (`.travis.yml`)
/// - CircleCI (`.circleci/config.yml`)
/// - Azure Pipelines (`azure-pipelines.yml`)
/// - Bitbucket Pipelines (`bitbucket-pipelines.yml`)
/// - Drone CI (`.drone.yml`)
/// - AppVeyor (`.appveyor.yml`, `appveyor.yml`)
/// - Buildkite (`.buildkite/pipeline.yml`, `.buildkite/pipeline.yaml`)
/// - Woodpecker CI (`.woodpecker.yml`, `.woodpecker/*.yml`, `.woodpecker/*.yaml`)
/// - Forgejo Actions (`.forgejo/workflows/*.yml`, `.forgejo/workflows/*.yaml`)
/// - Gitea Actions (`.gitea/workflows/*.yml`, `.gitea/workflows/*.yaml`)
#[derive(Default)]
pub struct CiConfigObserver {
    found_ci_config: bool,
}

inventory::submit!(ObserverFactory::new::<CiConfigObserver>());

impl Observer for CiConfigObserver {
    fn emits(&self) -> Discriminant<Observation> {
        Observation::CI_CONFIG
    }

    fn is_interested_in_diff(&self) -> bool {
        true
    }

    fn on_commit(
        &mut self,
        _commit: &gix::Commit,
        _repo: &gix::Repository,
    ) -> eyre::Result<Option<Observation>> {
        Ok(None)
    }

    fn on_diff_start(&mut self) -> eyre::Result<()> {
        self.found_ci_config = false;
        Ok(())
    }

    #[tracing::instrument(
        target = "perf",
        level = "debug",
        name = "CiConfig::on_diff_change",
        skip_all
    )]
    fn on_diff_change(
        &mut self,
        change: &gix::object::tree::diff::ChangeDetached,
        _repo: &gix::Repository,
    ) -> eyre::Result<DiffAction> {
        if self.found_ci_config {
            return Ok(DiffAction::Cancel);
        }

        let location = match change {
            gix::object::tree::diff::ChangeDetached::Addition { location, .. } => location,
            gix::object::tree::diff::ChangeDetached::Rewrite { location, .. } => location,
            _ => return Ok(DiffAction::Continue),
        };

        if is_ci_config(location.as_ref()) {
            self.found_ci_config = true;
            return Ok(DiffAction::Cancel);
        }

        Ok(DiffAction::Continue)
    }

    fn on_diff_end(&mut self) -> eyre::Result<Option<Observation>> {
        if self.found_ci_config {
            return Ok(Some(Observation::CiConfig));
        }
        Ok(None)
    }
}

/// Well-known CI configuration file paths that can be matched exactly.
const EXACT_PATHS: &[&[u8]] = &[
    b".gitlab-ci.yml",
    b"Jenkinsfile",
    b".travis.yml",
    b".circleci/config.yml",
    b"azure-pipelines.yml",
    b"bitbucket-pipelines.yml",
    b".drone.yml",
    b".appveyor.yml",
    b"appveyor.yml",
    b".woodpecker.yml",
    b".buildkite/pipeline.yml",
    b".buildkite/pipeline.yaml",
];

/// Directory prefixes where any `.yml` or `.yaml` file is a CI configuration.
const CI_DIRECTORIES: &[&[u8]] = &[
    b".github/workflows/",
    b".forgejo/workflows/",
    b".gitea/workflows/",
    b".woodpecker/",
];

fn is_ci_config(path: &[u8]) -> bool {
    if EXACT_PATHS.contains(&path) {
        return true;
    }

    if has_yaml_extension(path) && CI_DIRECTORIES.iter().any(|d| path.starts_with(d)) {
        return true;
    }

    false
}

fn has_yaml_extension(path: &[u8]) -> bool {
    path.ends_with(b".yml") || path.ends_with(b".yaml")
}

#[cfg(test)]
mod tests {
    use herostratus_tests::fixtures::repository;

    use super::*;
    use crate::observer::impls::test_helpers::observe_all;

    #[test]
    fn modification_of_ci_file_not_detected() {
        let repo = repository::Builder::new()
            .commit("add CI")
            .file(".github/workflows/ci.yml", b"name: CI")
            .commit("update CI")
            .file(".github/workflows/ci.yml", b"name: CI v2")
            .build()
            .unwrap();
        let observations = observe_all(&repo, CiConfigObserver::default());
        // Only the first commit (addition) triggers, not the modification
        assert_eq!(observations, [Observation::CiConfig]);
    }

    #[test]
    fn emits_ci_config_observation() {
        let repo = repository::Builder::new()
            .commit("add multiple CI configs")
            .file(".github/workflows/ci.yml", b"name: CI")
            .file(".gitlab-ci.yml", b"stages: [build]")
            .build()
            .unwrap();
        let observations = observe_all(&repo, CiConfigObserver::default());
        assert_eq!(observations, [Observation::CiConfig]);
    }

    #[test]
    fn is_ci_config_exact_matches() {
        assert!(is_ci_config(b".gitlab-ci.yml"));
        assert!(is_ci_config(b"Jenkinsfile"));
        assert!(is_ci_config(b".travis.yml"));
        assert!(is_ci_config(b".circleci/config.yml"));
        assert!(is_ci_config(b"azure-pipelines.yml"));
        assert!(is_ci_config(b"bitbucket-pipelines.yml"));
        assert!(is_ci_config(b".drone.yml"));
        assert!(is_ci_config(b".appveyor.yml"));
        assert!(is_ci_config(b"appveyor.yml"));
        assert!(is_ci_config(b".woodpecker.yml"));
        assert!(is_ci_config(b".buildkite/pipeline.yml"));
        assert!(is_ci_config(b".buildkite/pipeline.yaml"));
    }

    #[test]
    fn is_ci_config_directory_matches() {
        assert!(is_ci_config(b".github/workflows/ci.yml"));
        assert!(is_ci_config(b".github/workflows/release.yaml"));
        assert!(is_ci_config(b".forgejo/workflows/test.yml"));
        assert!(is_ci_config(b".gitea/workflows/deploy.yaml"));
        assert!(is_ci_config(b".woodpecker/build.yml"));
    }

    #[test]
    fn is_ci_config_rejects_non_ci() {
        assert!(!is_ci_config(b"README.md"));
        assert!(!is_ci_config(b"src/main.rs"));
        assert!(!is_ci_config(b".github/CODEOWNERS"));
        assert!(!is_ci_config(b".github/workflows/README.md"));
        assert!(!is_ci_config(b"jenkins/build.sh"));
    }
}
