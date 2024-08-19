use std::path::{Path, PathBuf};
use std::process::Output;

use assert_cmd::Command;
use tempfile::{tempdir, TempDir};
use tracing::Level;
use tracing_subscriber::fmt::writer::TestWriter;
use tracing_subscriber::EnvFilter;

#[ctor::ctor]
fn setup_test_logging() {
    let filter = EnvFilter::builder()
        .with_default_directive(Level::DEBUG.into())
        .with_env_var("HEROSTRATUS_LOG")
        .from_env_lossy();
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(TestWriter::new())
        .init();
}

// Cache the path to the binary as suggested by https://github.com/assert-rs/assert_cmd/issues/6 to
// avoid expensive repeated lookups.
lazy_static::lazy_static! {
    static ref HEROSTRATUS: PathBuf = assert_cmd::cargo::cargo_bin("herostratus");
}

/// Get a [`Command`] for the herostratus binary and the [`TempDir`] data dir used in the test
pub fn herostratus(data_dir: Option<&Path>) -> (Command, Option<TempDir>) {
    let (tempdir, path) = if let Some(data_dir) = data_dir {
        (None, data_dir.to_path_buf())
    } else {
        let temp = tempdir().unwrap();
        let data_dir = temp.path().to_path_buf();
        (Some(temp), data_dir)
    };

    let mut cmd = Command::new(&*HEROSTRATUS);
    cmd.arg("--log-level=DEBUG").arg("--data-dir").arg(path);

    (cmd, tempdir)
}

fn capture_output(output: &Output) {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    // Test output capture relies on magic in the print! and println! macros
    print!("{stdout}");
    print!("{stderr}");
}

pub trait CommandExt {
    /// Same as [Command::output], except with hooks to print stdout and stderr for failed tests
    fn captured_output(&mut self) -> std::io::Result<Output>;
}

impl CommandExt for Command {
    fn captured_output(&mut self) -> std::io::Result<Output> {
        let output = self.output()?;
        capture_output(&output);
        Ok(output)
    }
}

// TODO: Split herostratus::test::fixtures out into its own library so I don't have to copy-pasta
// the fixtures to use them in the integration tests.
#[allow(unused)]
pub mod fixtures {
    use git2::{Repository, Signature, Time};
    use tempfile::{tempdir, TempDir};

    pub struct TempRepository {
        pub tempdir: TempDir,
        pub repo: Repository,
    }

    pub fn add_empty_commit(repo: &Repository, message: &str) -> eyre::Result<()> {
        let mut index = repo.index()?;
        let head = repo.find_reference("HEAD")?;
        let parent = head.peel_to_commit().ok();
        let parents = if let Some(ref parent) = parent {
            vec![parent]
        } else {
            vec![]
        };

        let oid = index.write_tree()?;
        let tree = repo.find_tree(oid)?;

        let time = Time::new(1711656630, -500);
        let signature = Signature::new("Herostratus", "Herostratus@example.com", &time)?;

        let oid = repo.commit(
            Some("HEAD"),
            &signature,
            &signature,
            message,
            &tree,
            &parents,
        )?;
        tracing::debug!("Created commit {oid:?}");

        Ok(())
    }

    pub fn simplest() -> eyre::Result<TempRepository> {
        with_empty_commits(&["Initial commit"])
    }

    pub fn with_empty_commits(messages: &[&str]) -> eyre::Result<TempRepository> {
        let tempdir = tempdir()?;
        tracing::debug!("Creating repo fixture in '{}'", tempdir.path().display());

        let repo = Repository::init(tempdir.path())?;

        for message in messages {
            add_empty_commit(&repo, message)?;
        }

        Ok(TempRepository { tempdir, repo })
    }
}
